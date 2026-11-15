# ADR 0624: F-701 proposal: the long-horizon loop (H6): a durable hypothesis record, tests registered before their runs, and verdicts scoped to the evaluator and its fidelity

Status: **DRAFT STUB 2026-11-15. Not registered, not accepted.** The maintainer answered the open questions on 2026-11-15 (section \"Maintainer decisions\"); the limits are still proposals until the implementing agent registers them. No code of F-701 exists and no check has run. Nothing in
this text is a result of rcfd. Every number marked "proposed" is a starting value and may be changed until the registration
is written; after that the house rule applies (limits fixed before the run, a miss becomes an ignored `*_known_miss` test
with the limit intact). The open questions at the end are for the maintainer and are not taken here.

Disclosed: a rough numpy prototype (`h6_proto.py`, next to this draft, outside the repository; about 80 lines written from
the textbook formulas, Student t quantiles by a Cornish-Fisher expansion, not rcfd code). Its numbers are quoted under
"Estimates behind the limits". It evaluates no rcfd function.

Roadmap item F-701 (proposed deps F-268, F-697, F-699), part H6 of `docs/fan-project-breakdown.md`: "Long-horizon
improvement: Claude reads the ledger, forms hypotheses (for example 'more sweep lowers tonal noise at equal flow'), tests
them, and records what it learned". Number 0624 given by the lead. Read only; nothing in the repository was edited, built
or run.

Parents and neighbours:

* ADR 0121 (design ledger, `ledger::Model`, the tolerance driver) and ADR 0240 (Sobol indices): the pattern of a library
  driver over a `Model` that logs to the ledger.
* ADR 0623 (draft, F-268 remaining scope): the ledger sink for studies, the `FanRomModel` adapter, and the proposed ledger
  fields P1 to P8 (entry reference, parent links, metric uncertainty, objective identity).
* ADR 0620 (draft, F-697, objective), ADR 0621 (draft, F-698, surrogate), ADR 0622 (draft, F-699, fidelity management).
  All three are drafts; every reference to them is an assumption to be checked when they are registered.
* ADR 0119 (reduced-order fan, `rcfd_post::fan_rom`, `design::fan_rom_eval::FanRomEvaluator`).

House rules that apply: clean-room (published literature and first principles only, see References); defaults bit-identical
(everything here is opt-in; `crates/rcfd-api/tests/baseline_hashes.rs` and the golden files are not edited); no new
dependency, no unsafe code; no case-file or project key and no MCP tool-list change without the maintainer's decision (Q1,
Q2, Q3, Q4).

## Context

What the plan asks. H6 is the last step of the fan plan: after the ledger (H4), the objective (H1), the surrogate (H3) and
the fidelity loop (H7) exist, the assistant is to learn across sessions. It reads what was evaluated, states a claim, tests
it, and writes down the outcome so that a later session starts from it.

What exists (checked in the source):

* `design::ledger` (ADR 0121): an append-only JSON-lines file. `Entry.kind` is a free string (`evaluation`, `uq_summary`,
  `sobol_summary`). An entry's index is its line position, and nothing is ever rewritten, so the order of lines is a
  durable record of what was written first. `rerun_check` refuses another evaluator id or version.
* `ledger::Fidelity` has three values (`analytic`, `reduced_order`, `cfd`). The same three are hard-coded as a JSON-schema
  enum in three places of `crates/rcfd-mcp/src/lib.rs` and therefore sit in the tools-list goldens
  (`crates/rcfd-mcp/tests/golden/tools_list_default.json`, `tools_list_project_f270.json`).
* The MCP tool `design_ledger_append` takes a free `kind`, free `provenance` and any metric values. So an assistant can
  today append any line it likes, including a line that looks like a verdict. Any design that trusts a stored verdict is
  open to this.
* `design::uq` has `norm_cdf`, `binom_cdf`, `clopper_pearson`, `quantile_interval`; `rcfd_reactor::mixfrac` has
  `reg_inc_beta`, which gives the Student t distribution without new code of substance. `rcfd_core::hash::fnv64` is the
  only content hash in reach (64 bit, not cryptographic).
* `design::sampling` (seeded Latin hypercube), `design::study::parallel_map`.
* The assistant tools of `docs/mcp-tools.md` and `docs/design-loop.md`: ten `design_*` study tools and four
  `design_ledger_*` tools. None knows a hypothesis. `uq::run` and `sensitivity::run` are library only (F-339 proposes
  their tools).
* No hypothesis, experiment or lesson record exists anywhere in `crates/` or `docs/`.

Why a record and not free notes. An assistant session ends and its context is lost. What it "learned" survives only as
text someone kept. Free text cannot be checked: it does not say which runs support it, with which evaluator version, at
which fidelity, or how many other claims were tried and dropped on the way. The project already has a discipline for this
in its own verification work (limits written before the run, misses recorded and never tuned away). H6 applies the same
discipline to design claims.

Why the assistant is the main risk. A system that proposes claims from data and then tests them on the same data confirms
noise (Simmons, Nelson and Simonsohn 2011; Gelman and Loken 2013; Kerr 1998). An optimiser run on a cheap model finds the
places where the model is wrong in the favourable direction (Smith and Winkler 2006). A long-running loop tests many claims,
and some pass by chance. None of this needs bad intent, and a language model is fluent at explaining a chance result after
the fact. The design below therefore puts every guard in the library, where it holds whoever the caller is, and does not
rely on the caller's judgement.

Why the first target is the reduced-order model. Fan CFD does not converge today (`docs/milestone-fan.md`; ADR 0622 gives
the references), and no measured fan exists. Every claim the loop can test now is a claim about `rcfd_post::fan_rom`, whose
loss constants are marked ASSUMED or UNVERIFIED in the code. A verdict must therefore carry its evaluator and tier with it
everywhere it is shown, or the ledger fills with statements about a model that read like statements about fans.

## Decision proposal

### D1. Where and what (opt-in, library only)

New module `crates/rcfd-api/src/design/hypothesis.rs` with `hypothesis_tests.rs`. No existing path calls it. No service
method, no MCP tool, no case-file or project key in this item (Q1 to Q3). Functions (names to be fixed at registration):

* `register(ledger, hypothesis) -> Result<HypothesisRef, String>`: validates and appends one `hypothesis` entry.
* `run(ledger, ref, model) -> Result<TestResult, String>`: evaluates the registered design with a `ledger::Model`, appends
  the evidence entries and one `hypothesis_result` entry. Resumable: a second call finishes the missing evaluations only.
* `verify(ledger, ref) -> Verification`: recomputes the verdict from the evidence entries and reports every rule broken.
* `lessons(ledger, query, current) -> Vec<Lesson>`: what is currently known, with scope and staleness (D8).
* `explore(ledger, filter) -> ...`: optional reading aids for forming hypotheses (rank correlation of each parameter with
  each metric over existing entries). Marked exploratory in its output; never evidence (D4).

### D2. The hypothesis record

One ledger entry of `kind = "hypothesis"`, written before any run. Fields (in `provenance`, or as P2/P7-style fields if ADR
0623 adds them):

| Field | Content |
|---|---|
| `id` | `H-<n>`, `n` the count of `hypothesis` entries in this ledger plus one (durable; a new session cannot restart it) |
| `claim_text` | the claim in plain words, stored verbatim, never interpreted by the code |
| `factor` | parameter name, unit, low and high level |
| `response` | metric name, unit |
| `direction` | `increases`, `decreases` or `no_effect` |
| `predicted_size` | optional interval of the mean change of the response from low to high (response unit) |
| `min_effect` | smallest change that matters (response unit, strictly positive); see D5 |
| `held` | list of `{metric, target or "as_low_arm", tolerance, by: parameter, bracket}`: metrics kept equal between the two arms by solving one compensating parameter each ("at equal flow": `q` held by speed) |
| `domain` | bounds of the other parameters over which the claim is made, with units |
| `design` | `paired`, number of pairs `n`, sampler (Latin hypercube), seed |
| `evaluator` | id, version, tier; objective id, version and hash if an F-697 objective is the response |
| `test` | statistic, family level, the level spent on this hypothesis (D6), largest share of failed pairs allowed |
| `origin` | `{kind: exploratory, entries: [references]}`: what the claim was formed from; or `prior` (literature, first principles) with a citation |
| `supersedes`, `replicates` | optional `H-` ids (a larger re-test of an inconclusive one; the same claim at a higher tier or a new evaluator version) |
| `hash` | hash of the canonical JSON of all fields above |

Validation refuses, with located errors: a missing or non-positive `min_effect`; a factor outside the domain; a held metric
equal to the response; `n` below the registered smallest (proposed 16); a `direction` of `no_effect` without `min_effect`
(a claim of no effect is an equivalence claim and needs a size, Schuirmann 1987).

### D3. Registered before run, checked mechanically

The ledger is append-only and the index is the line position. That gives an order that needs no clock:

1. Every evidence entry of a hypothesis must have an index greater than the index of its `hypothesis` entry and must carry
   `provenance {driver: hypothesis, hypothesis: id, hash, pair, arm}`.
2. The `hypothesis_result` entry carries the registration index and hash. `verify` recomputes the hash from the stored
   registration and refuses a mismatch.
3. A registration is never edited. Before its first evidence entry it may be withdrawn by an appended
   `hypothesis_withdrawn` entry with a reason (it still counts in D6). After the first evidence entry nothing about it can
   change: not `n`, not the level, not the domain, not `min_effect`.
4. `run` produces exactly the registered pairs. A result computed from fewer or more pairs than registered is refused
   (no stopping when it looks good, no adding runs until it passes; Armitage, McPherson and Rowe 1969).
5. A miss is recorded like a hit. `refuted` and `inconclusive` verdicts are ordinary result entries, listed by `lessons`
   with the same standing as `confirmed` (the file-drawer problem, Rosenthal 1979).
6. A second attempt at the same claim is a new hypothesis with `supersedes` or `replicates`, a new seed and its own level
   from D6. The old result stays.

### D4. Exploratory and confirmatory data are kept apart

* Entries that existed before the registration may be read to form the claim (`origin`). They are never evidence for it.
  A claim suggested by a data set and then tested on the same data set is biased toward confirmation (de Groot 1956;
  Wagenmakers et al. 2012).
* Evidence comes only from the registered design: fresh evaluations at seeded Latin-hypercube points of the domain. Points
  chosen by an optimiser, by a surrogate's acquisition rule or by an F-699 promotion are selected because they looked good
  and are never evidence (the optimiser's curse, Smith and Winkler 2006).
* Surrogate predictions (F-698) and corrected predictions (F-699) are never evidence. Only `evaluation` entries of the
  registered evaluator with verdict `ok` count.
* The registered seed is derived from the registration hash and a caller seed, so the caller cannot pick the sample after
  seeing which seeds give a favourable one.

### D5. The test

Design: paired comparison (Box, Hunter and Hunter 2005, the paired design). For pair `i = 1..n`, a background point `x_i`
of the domain; the factor at its low and at its high level with everything else equal; each held metric brought to its
target in both arms by solving its compensating parameter (bisection in the registered bracket, to the registered
tolerance). `d_i = s (y_high,i - y_low,i)`, with `s = +1` for `increases` and `-1` for `decreases`.

Estimand: the mean of `d` over the domain. Statistic: mean `m`, standard deviation `sd` (n - 1), `SE = sd / sqrt(n)`,
one-sided Student t bounds `L = m - t SE`, `U = m + t SE` with `t = t_{1 - a, n - 1}` (Student 1908) at the level `a`
spent on this hypothesis (D6).

Verdicts (proposed, Q5):

| Verdict | Rule |
|---|---|
| `confirmed` | `L >= min_effect` |
| `refuted` | `U < min_effect`; sub-flag `opposite` when `U < 0` |
| `inconclusive` | neither |
| `void` | more failed pairs than the registered share (proposed 0.2), a held metric not matched, or the evaluator id or version changed between the first and the last evidence entry |

For `no_effect`: `confirmed` when the two-sided interval lies inside `(-min_effect, +min_effect)` (two one-sided tests,
Schuirmann 1987), `refuted` when it lies outside, otherwise `inconclusive`.

Also reported, without bearing on the verdict: the share of pairs with the predicted sign and its Clopper-Pearson
interval; whether the interval `[L, U]` overlaps `predicted_size`; the smallest and largest `d_i` with their pair index
(a mean can hide a region of the domain where the effect reverses).

What "noise" means, by evaluator:

* **Deterministic evaluator** (the reduced-order fan): a repeat gives the same bits, so `sd` is the variation of the effect
  over the domain, not noise. The result says so (`noise: none, spread: domain`). When every `d_i` is identical, `sd = 0`
  and the bounds equal the mean; no division by zero.
* **Resolution floor.** A deterministic model still has a numerical resolution (the operating-point bisection tolerance of
  the fan evaluator, a held-metric tolerance). `register` refuses a `min_effect` below `k_res` times the resolution the
  model declares for the response (proposed `k_res = 10`), and below the change of the response caused by the held-metric
  tolerance.
* **Noisy evaluator** (a tolerance-averaged objective of F-697, a CFD value with iteration error, a measurement): pairing
  removes what the two arms share; the remaining noise is in `sd`. If entries carry a stated uncertainty (ADR 0623 P5) the
  result also reports the ratio of `sd` to the stated value; a ratio far below 1 means the stated uncertainty or the
  pairing is wrong, and is flagged.

Fidelity scope. Every result carries `scope = {evaluator, version, tier}` and a model-error flag:

* `below_model_error` when `|m|` is smaller than the known error of this tier for this metric. The known error is the
  running out-of-sample RMSE of `high - pred` in the latest `fidelity_summary` of ADR 0622 D6 for this evaluator version,
  or a value the caller declares (Q13).
* `model_error_unknown` when neither exists. This is the state of every reduced-order result today.
* A verdict at tier `analytic` is verification only and is never returned by `lessons`.
* A claim confirmed at `reduced_order` is confirmed about that model. It becomes a claim at `cfd` (or `measured`, if ADR
  0622 Q2 adds it) only through a new registered test at that tier with `replicates` set. `lessons` shows both results
  side by side when they disagree and never merges them.

### D6. Many hypotheses: a durable error budget

A loop that tests claims for months will confirm some by chance unless the level per test shrinks. The proposed rule (Q6):
hypothesis number `i` of the family is tested at

`a_i = a_family * 6 / (pi^2 i^2)`, with `a_family = 0.05` (proposed).

Since `sum 1/i^2 = pi^2 / 6`, the levels of any number of hypotheses sum to at most `a_family`, and by the union bound the
probability of at least one false confirmation in the whole family is at most `a_family`, for any dependence between tests
and for any rule the caller uses to choose the next claim. It needs no knowledge of how many hypotheses will follow.

* `i` is the count of `hypothesis` entries of the family in the ledger, withdrawn and void ones included. It is read from
  the file, so a new session cannot reset it.
* The family is the ledger file (Q7).
* The price is power: at `i = 30` the level is 3.4e-5. For a deterministic model this costs little (pairs are cheap and
  `sd` is domain spread). For CFD it is expensive, and that is the honest price of asking many questions of little data.
  Less strict published alternatives are listed in Q6.

### D7. A stored verdict is never trusted

Because `design_ledger_append` can write any line (Context), `lessons` and `verify` do not read the verdict of a
`hypothesis_result` entry. They recompute it from the registration and the evidence entries, and report a result entry
whose stored verdict differs, whose evidence is missing, out of order (D3), of another evaluator, or not written by the
driver. Evidence entries themselves could be forged with invented metrics; for a deterministic evaluator `verify` can
re-evaluate a registered share of them with `rerun_check` (proposed: all of them for the reduced-order tier, since it costs
milliseconds; Q14). For an evaluator that cannot be re-run cheaply this guard does not exist, and the lesson says
`evidence not re-run`.

### D8. What "learned" means: the lessons query

`lessons(ledger, query, current)` returns one row per hypothesis that passes `verify`, with:

* the generated statement, built from the record and not from `claim_text`. Example of the form: "reduced_order (fan_rom
  0.3): raising `sweep` from 10 to 30 deg changed `tone1_pa` by -0.8 Pa on average (one-sided bounds -1.1, -0.5; 61 of 64
  pairs lower) with `q` held within 0.5 percent, over the registered domain. Confirmed as hypothesis 7 of the family.
  Model error of this tier unknown. Not tested at cfd." (Numbers invented for the form only.)
* `claim_text` as written, labelled as the author's words;
* verdict, scope, model-error flag, family index and level, references to the registration, result and evidence entries;
* `stale`: true when `current` (the evaluator ids and versions, and objective hashes, that the caller holds now) differs
  from the scope. A stale lesson is not returned unless asked for, and is then labelled. Staleness is computed at read
  time, so nothing is rewritten; a re-test is a new hypothesis with `replicates` (Q9).
* filters: factor, response, verdict, smallest tier, include stale.

`refuted`, `inconclusive` and `void` rows are returned by default. Knowing that a claim failed, or could not be decided
with 64 pairs, is part of what was learned.

### D9. Determinism and defaults

Evaluations go through `parallel_map` and are appended in pair order from the calling thread (as `uq::run`), so the file
does not depend on the worker count. Opt-in only: no existing function changes behaviour, no existing ledger entry changes,
and the format string stays `rcfd-ledger-1` (new `kind` values only; existing readers tolerate them).

## Registration (to be fixed before any code)

The registration replaces this section. It must fix every item below after a disclosed smoke with seeds other than the
registered ones and before the registered runs. No check uses FAN-01, a mesh, a flow solve, a person or a language model.

### The scripted investigator

The acceptance needs an agent that behaves the way the risk says an assistant behaves, and that is a fixed program. Three
scripted agents, all deterministic functions of a seed:

* **Protocol agent.** Reads an exploratory ledger. For every (parameter, metric) cell computes the correlation over the
  exploratory entries, takes the `K` cells with the largest absolute value, and registers one hypothesis per cell in the
  apparent direction, strongest first. Runs each through `register` and `run`. It is deliberately greedy: it picks the
  claims that look best in the data it has seen.
* **Naive agent (negative control).** Same reading. Declares a cell confirmed when its correlation in the exploratory data
  is significant at 0.05 one-sided in the apparent direction. No registration, no fresh data, no correction.
* **Peeking agent (negative control).** Runs a paired test of a true null and looks after every pair from 8 on, stopping
  at the first look with a one-sided p below 0.05.

### Synthetic worlds

* **W1 planted linear world.** `d = 12` parameters on the unit box, 4 metrics, normal noise of sd 1 on every metric,
  seeded. Of the 48 cells, 12 are planted: 4 large (change 1.5 between factor levels 0.25 and 0.75), 4 at the boundary
  (change exactly `min_effect = 0.5`) and 4 small (0.25). The other 36 are exactly zero. Exploration: 60 points. `K = 12`
  hypotheses per world, 64 pairs each. A **false confirmation** is a `confirmed` verdict for a cell whose true change is
  at most `min_effect` (zero, small and boundary cells). `R = 400` worlds with registered seeds.
* **W2 two-tier world with a planted artefact.** A low model (declared `reduced_order`) and a high model (declared `cfd`)
  that agree except on registered cells where the low model has an effect and the high model has none or the opposite one.
* **W3 equal-flow closed form.** `q = n (1 + a s)`, `tone = n^2 (1 + a s)^2 (1 + b s)`, parameters `s` (the factor) and
  `n` (the compensating parameter). At equal `q = q_0` the tone is `q_0^2 (1 + b s)` exactly; at fixed `n` it rises with
  `s` for `a > 0`. Case A: `b = 0` (no effect at equal flow). Case B: `b = -0.1`.
* **W4 the reduced-order fan.** `FanRomEvaluator` through the `FanRomModel` adapter of ADR 0623 D2 (or a test-local
  adapter if that is not merged), on a registered `RomFan` family, plus one dummy parameter the evaluator ignores, plus one
  factor with a known monotone effect on a registered metric (candidate: tip gap on `eta_s`; to be confirmed by the
  disclosed smoke, not assumed).

### Checks (limits proposed; each gets a test name, seeds and a final limit at registration)

| Id | Check | Proposed limit |
|---|---|---|
| G1 | Record round trip: a registered hypothesis read back from a new `Ledger` handle equals the original; hash stable | bit-identical |
| G2 | Order: a result is refused when (a) one evidence entry precedes the registration, (b) the number of pairs differs from the registered one, (c) the registration line was altered after the fact, (d) the evidence evaluator version differs | 4 of 4 refused, located errors |
| G3 | Stored verdict not trusted: a `hypothesis_result` line with verdict `confirmed` appended by hand for each of 5 registered hypotheses | 0 of 5 returned by `lessons`; 5 of 5 reported by `verify` |
| G4 | Resume: `run` killed after each registered kill point (proposed 0, 1, 63, 127 of 128 evaluations) and called again | 0 duplicate and 0 missing evidence entries; result entry bit-identical to an uninterrupted run |
| S1 | W1, protocol agent: worlds with at least one false confirmation | at most 37 of 400 (0.05 plus 4 binomial standard errors) |
| S2 | W1, protocol agent: large planted effects that were registered and are confirmed | at least 95 percent |
| S3 | W1: level spent, summed over the hypotheses of one ledger | at most 0.05 for every world; `i` continues across a closed and reopened ledger |
| S4 | W1, single test at the boundary (true change exactly `min_effect`), 4000 seeded repeats at level 0.05: rate of `confirmed` | within 4 standard errors of 0.05 (the test has its stated size) |
| S5 | W1, exactly zero cells: verdict `confirmed` | 0 |
| F1 | W2: every artefact claim tested at the low tier has scope `reduced_order` and flag `model_error_unknown` or `below_model_error`; `lessons` with smallest tier `cfd` | 0 artefact claims returned |
| F2 | W2: the same claims re-tested at the high tier with `replicates` | refuted; `lessons` returns both results, unmerged, for 100 percent of them |
| F3 | W4, dummy parameter: every `d_i` | exactly 0; verdict `refuted`; no division by zero |
| F4 | W4, the registered monotone factor: verdict and scope | `confirmed`, scope `reduced_order`, flag `model_error_unknown` |
| F5 | W4: `register` with `min_effect` below `k_res` times the declared resolution | refused, located error |
| E1 | W3 case A, `q` held by `n`: mean change of tone; verdict of "raising s raises tone" | within 1e-9 relative of 0 on the scale `q_0^2`; `refuted` |
| E2 | W3 case B: mean change against `-0.1 q_0^2 (s_high - s_low)` | within 1e-9 relative; `confirmed` for `decreases` |
| E3 | W3: a held metric that cannot be matched inside the bracket | `void`, located reason, counted in the family |
| T1 | Staleness: 12 results, then `current` with the evaluator version changed | 12 of 12 flagged stale; 0 returned by default |
| T2 | An evidence set that spans two evaluator versions | `void` |
| R1 | Same seeds, 1 and 8 workers, fixed clock: ledger | byte-identical |
| R2 | Existing default-bits tests, `baseline_hashes.rs`, both tools-list goldens | unchanged, files not edited |

Negative controls (each must fail as stated, or the check it guards proves nothing):

* NC1 for S1: the naive agent false-confirms in at least 300 of 400 worlds.
* NC2 for S1 and S3: the protocol agent with the level fixed at 0.05 for every hypothesis exceeds the S1 limit.
* NC3 for G2 (b): the peeking agent with the pair-count rule switched off false-confirms a true null in at least 40 of 400
  worlds.
* NC4 for E1: with `held` removed, the false claim of case A is `confirmed`.
* NC5 for G3: with recomputation switched off, the 5 hand-written verdicts are returned.
* NC6 for D4: the protocol agent allowed to count the exploratory entries as evidence exceeds the S1 limit.

Planned mutations (scratch copy, not committed): sign of `d`; `sd` with `n` in place of `n - 1`; two-sided level used for
a one-sided test; `a_i` with `i` not advanced by a void or withdrawn hypothesis; `i` restarted per session; order test `>=`
in place of `>`; staleness compared on id only; held-metric tolerance ignored; `min_effect` compared with `m` in place of
`L`.

### Estimates behind the limits (numpy prototype, disclosed)

`h6_proto.py`, 400 worlds, seeds 1000 to 1399, W1 as above. These are prototype numbers, not rcfd results.

| Quantity | Prototype | Proposed limit |
|---|---|---|
| S1 worlds with a false confirmation, series of D6 | 2 of 400 | at most 37 |
| The same with one Bonferroni level 0.05 / 12 | 6 of 400 | (alternative of Q6) |
| NC2, level 0.05 for every hypothesis | 46 of 400 | must exceed 37 |
| NC1 naive agent | 397 of 400 | at least 300 |
| S2 large effects confirmed | 1598 of 1600 | at least 95 percent |
| NC3 peeking agent on a true null | 90 of 400 | at least 40 |
| Exactly zero cells refuted (not a check) | 498 of 1863 | none |

Two things the prototype shows that the registration must deal with:

* NC2 clears the S1 limit by a thin margin (46 against 37). Before registration W1 is strengthened (proposed: 8 boundary
  cells in place of 4, or `K = 24`) until the uncorrected agent fails clearly in the smoke. The limit of S1 is not moved to
  make room.
* With the series of D6 only about a quarter of the true-zero claims are `refuted` at 64 pairs; the rest are
  `inconclusive`. That is correct behaviour and it will look unproductive. Q6 and Q10 are about this.

### Decision rule (proposed)

F-701 is done when G, S, F, E, T and R pass and every negative control fails as stated. A miss in S2 alone (power) with
everything else passing gives `partial` with the limit kept as a known miss. A miss in S1, S4, S5, F1 or G3 means the
guard does not work, and the item is not done.

## Known risks

1. **The acceptance tests the library, not the assistant.** The scripted agents are greedy in one fixed way. A language
   model can fool itself in ways no script anticipates: by choosing `min_effect` after an informal look, by choosing the
   domain where an effect is known to hold, by wording `claim_text` more broadly than the tested claim. The record makes
   these visible (origin, domain, the generated statement next to the author's words) and does not prevent them.
2. **Evidence outside the ledger.** The guards see only what is in the file. Evaluations made elsewhere (a library call
   that does not log, another ledger file) before a registration are invisible, and D4 cannot refuse them. A second ledger
   file also starts a new family with a fresh budget (Q7).
3. **Confirmed about the model, read as confirmed about the fan.** The largest risk in practice. With only the
   reduced-order tier available, every confirmation is a property of a blade-element model with assumed loss constants.
   Scope and flag are attached to every row, but a reader who copies only the sentence loses them. The tonal-noise metric
   in particular (`tone1_pa`, a steady-loading estimate) is a proxy; "more sweep lowers tonal noise" cannot be tested
   meaningfully on a strip model that may not respond to sweep at all. F3 covers the case of a parameter the model
   ignores: the answer is `refuted` with an effect of exactly zero, and the statement must read as "the model does not
   respond", not as "sweep does not help".
4. **Mean over a domain hides structure.** An effect that is positive in half the domain and negative in the other half
   has a mean near zero and is `refuted`. The sign share and the extreme pairs are reported to show it; finding the
   sub-domain is a new hypothesis, formed from these (now exploratory) data, with its own level.
5. **The t test assumes roughly normal means.** With 64 pairs and a bounded smooth response this is mild. A response with
   rare large jumps (a stall boundary inside the domain) breaks it. S4 checks the size on normal noise only. Q8 offers a
   sign test as the alternative.
6. **Failed pairs are not missing at random.** If the high arm fails more often (it stalls), the surviving pairs are a
   biased sample. The `void` threshold bounds this crudely; the result reports failures per arm.
7. **The budget makes late hypotheses weak.** After some tens of hypotheses the level is very small. For CFD this may make
   the loop unable to confirm anything within a realistic budget. This is a property of the question, but it will create
   pressure to start a fresh ledger, which is the forking path the budget exists to close.
8. **Hash strength.** `fnv64` detects accidental edits, not a deliberate one. A deliberate rewrite of the ledger file
   defeats every guard here; the ledger has no signature and none is proposed.
9. **Staleness is coarse.** A version string that is not bumped when the model changes hides the change. A version bump
   that does not touch the tested behaviour marks everything stale. `rerun_check` on the evidence would tell the two
   apart for a deterministic model and is proposed as an optional step of `lessons`, not as the rule.
10. **Dependence on parallel drafts.** ADR 0620 to 0623 are drafts. The held-metric mechanism assumes an evaluator that
    exposes the operating-point flow as a metric (true of `FanRomEvaluator`); the model-error flag assumes the
    `fidelity_summary` of ADR 0622 D6; the adapter assumes ADR 0623 D2.
11. **Without tools the assistant cannot use it.** Library only means H6 is verified and not yet usable from a session;
    see Q1.

## Limits of this item

* One factor per hypothesis, two levels. Interactions, dose-response shapes and claims about optima are not covered.
* No automatic generation of hypotheses beyond the reading aid of D1 and the scripted agents of the tests.
* No CFD evidence, no measured evidence, no claim about a real fan.
* No MCP tool, no project key, no plugin text (Q1, Q3, Q12).

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

Every other open question of this ADR takes the recommendation written with it. Here: Q1 is D1; Q4 is D2 (the order over tiers includes `measured`, from F-700); Q6 is B2; Q15 keeps the deps F-268, F-697, F-699 (built last, D6); NC2 falls under B3.

## OPEN QUESTIONS for the maintainer

Each has options and a recommendation. None is taken in this draft.

1. **Q1 Exposure to the assistant (changes the tools-list goldens).** (a) Library only in F-701; H6 stays `partial` in the
   breakdown until tools exist. (b) F-701 also adds service methods and MCP tools (proposed names
   `design_hypothesis_register`, `design_hypothesis_run`, `design_hypothesis_lessons`); both tools-list goldens change, and
   `run` needs a named evaluator, the same problem as F-339. (c) (a) now and a follow-up item for the tools, scheduled with
   or after F-339 so that one named-evaluator mechanism serves both. **Recommendation: (c).** Without (b) or (c) the item
   does not let Claude do what H6 says.
2. **Q2 Where the records live.** (a) In the design ledger, as new `kind` values (`hypothesis`, `hypothesis_result`,
   `hypothesis_withdrawn`); one append-only order covers registrations and evidence, which D3 relies on. (b) A separate
   file next to the ledger; the order between the two files then needs a clock or cross-references. (c) A project-level
   store with its own key. **Recommendation: (a).**
3. **Q3 Case-file or project key for the ledger path.** (a) None; the path is an argument of every call. (b) A project key
   naming the project's ledger, so every session of a project shares one family; changes the project schema and the
   `project_shapes.json` golden. **Recommendation: (a)** in this item; (b) belongs with the tools of Q1 and would close
   risk 2 for projects.
4. **Q4 Fidelity values and their order.** The scope and `lessons` need an order of tiers. (a) Order `analytic <
   reduced_order < cfd` defined in the library; no new value; `measured` is placed above `cfd` only if ADR 0622 Q2 adds
   it. (b) Add a value now. Any new value in the MCP enums changes the goldens. **Recommendation: (a).**
5. **Q5 What `confirmed` means.** (a) Direction only: lower bound above 0. (b) Lower bound at or above a mandatory
   `min_effect`, as D5. (c) Both recorded, (b) decides. **Recommendation: (b)**: a direction-only rule confirms effects too
   small to matter, and with a deterministic model it confirms anything that is not exactly zero.
6. **Q6 The multiplicity rule and the family level.** (a) The series `0.05 * 6 / (pi^2 i^2)` of D6: controls the chance of
   any false confirmation over an unbounded sequence; strict. (b) Holm (1979) within a batch registered together, with the
   series across batches. (c) An online false-discovery-rate rule (alpha-investing, Foster and Stine 2008; LORD, Javanmard
   and Montanari 2018): more power, allows a stated share of false confirmations, harder to explain and to verify.
   **Recommendation: (a)** first, with (b) as an option of `register` for a batch; (c) only as a follow-up with its own
   registration. Please confirm 0.05 or give another level.
7. **Q7 What a family is.** (a) One ledger file. (b) One project (needs Q3 b). (c) One (response metric) within a ledger,
   so noise claims and efficiency claims have separate budgets. **Recommendation: (a)**; (c) multiplies the total error
   by the number of metrics.
8. **Q8 Test statistic.** (a) Paired t on the mean, as D5. (b) Exact sign test on the share of pairs with the predicted
   sign (no normality assumption, weaker, a different estimand). (c) Registered per hypothesis, default (a).
   **Recommendation: (c).**
9. **Q9 Stale results.** (a) Flag at read time only; re-testing is the caller's choice. (b) Also append a
   `hypothesis_stale` marker when a version change is first seen. (c) Re-test automatically; this spends runs and budget
   without a decision. **Recommendation: (a).**
10. **Q10 Inconclusive results.** (a) A re-test is a new hypothesis with a larger `n` and the next level of the series.
    (b) Allow one registered extension of `n` inside the same hypothesis at a split level (a two-look sequential design).
    (c) Cap the number of re-tests of one claim (proposed 2). **Recommendation: (a) with (c).**
11. **Q11 Existing entries as evidence.** (a) Never, as D4. (b) Allowed when the registration lists the entries and the
    code can show none was read through `explore` or returned by a query in this ledger's history; the code cannot show
    that for reads made outside it. **Recommendation: (a).** It costs re-evaluations, which are cheap on the reduced-order
    tier and expensive on CFD; the cost is the reason to ask.
12. **Q12 Guidance text for the assistant.** A short protocol page (when to register, how to choose `min_effect` and the
    domain, how to quote a lesson with its scope) in `docs/`, and possibly a skill in the Claude Code plugin. (a) A docs
    page in this item, plugin text with the tools of Q1. (b) Nothing until the tools exist. **Recommendation: (a).**
13. **Q13 Source of the model-error value.** (a) The out-of-sample RMSE of the latest matching `fidelity_summary` (ADR
    0622 D6) when present, otherwise `model_error_unknown`. (b) A caller-declared value is also accepted and labelled as
    declared. **Recommendation: (b)**, so the flag does not depend on F-699 being merged; then F-699 can be "related"
    rather than a dependency (Q15).
14. **Q14 How far `verify` goes.** (a) Recompute the verdict from stored evidence only. (b) Also re-evaluate all evidence
    with `rerun_check` when the evaluator is deterministic and of tier `reduced_order` or `analytic`. (c) Re-evaluate a
    seeded sample. **Recommendation: (b)**, with (c) as the option for expensive tiers.
15. **Q15 Dependencies and the H6 row.** (a) deps F-268, F-697, F-699 (the order of the plan: H6 after H3 and H7). (b)
    deps F-268 only, F-697 to F-699 related: the checks need only the ledger and a `Model`. (c) Also F-698.
    **Recommendation: (a)** if the loop is to be scheduled last as the plan says, (b) if it may be built earlier; the
    acceptance holds either way. The H6 row of the breakdown would go from `missing` to `partial` under Q1 (a) or (c).
16. **Q16 The author's words.** (a) `claim_text` stored verbatim and shown labelled next to the generated statement. (b)
    No free text; only the generated statement. **Recommendation: (a)**; the difference between the two is itself useful
    to a reviewer.
17. **Q17 Hash.** (a) `rcfd_core::hash::fnv64`, stated as a guard against accidents only. (b) A SHA-256 written in the
    tree (no dependency), if one does not already exist. **Recommendation: (a)**; a stronger hash without a signature
    gives no real protection against a deliberate rewrite.

## References

All cited from memory by this draft; volume, page, equation and section numbers are UNVERIFIED and must be checked at
registration and recorded in `docs/reference-data.md`.

* Student (W. S. Gosset) (1908), The probable error of a mean, Biometrika 6, 1-25.
* G. E. P. Box, J. S. Hunter, W. G. Hunter (2005), Statistics for Experimenters, 2nd ed., Wiley (paired comparison,
  blocking, randomisation).
* C. J. Clopper, E. S. Pearson (1934), The use of confidence or fiducial limits illustrated in the case of the binomial,
  Biometrika 26, 404-413.
* D. J. Schuirmann (1987), A comparison of the two one-sided tests procedure and the power approach for assessing the
  equivalence of average bioavailability, J. Pharmacokinet. Biopharm. 15, 657-680.
* S. Holm (1979), A simple sequentially rejective multiple test procedure, Scand. J. Statist. 6, 65-70.
* Y. Benjamini, Y. Hochberg (1995), Controlling the false discovery rate, J. R. Statist. Soc. B 57, 289-300.
* D. P. Foster, R. A. Stine (2008), Alpha-investing: a procedure for sequential control of expected false discoveries,
  J. R. Statist. Soc. B 70, 429-444.
* A. Javanmard, A. Montanari (2018), Online rules for control of false discovery rate and false discovery exceedance,
  Ann. Statist. 46, 526-554.
* P. Armitage, C. K. McPherson, B. C. Rowe (1969), Repeated significance tests on accumulating data, J. R. Statist. Soc.
  A 132, 235-244.
* J. P. Simmons, L. D. Nelson, U. Simonsohn (2011), False-positive psychology, Psychol. Sci. 22, 1359-1366.
* A. Gelman, E. Loken (2013), The garden of forking paths (manuscript, Columbia University).
* N. L. Kerr (1998), HARKing: hypothesizing after the results are known, Pers. Soc. Psychol. Rev. 2, 196-217.
* A. D. de Groot (1956, English translation 2014), The meaning of "significance" for different types of research, Acta
  Psychologica 148, 188-194.
* E.-J. Wagenmakers, R. Wetzels, D. Borsboom, H. L. J. van der Maas, R. A. Kievit (2012), An agenda for purely
  confirmatory research, Perspect. Psychol. Sci. 7, 632-638.
* B. A. Nosek, C. R. Ebersole, A. C. DeHaven, D. T. Mellor (2018), The preregistration revolution, Proc. Natl. Acad. Sci.
  115, 2600-2606.
* R. Rosenthal (1979), The file drawer problem and tolerance for null results, Psychol. Bull. 86, 638-641.
* J. E. Smith, R. L. Winkler (2006), The optimizer's curse: skepticism and postdecision surprise in decision analysis,
  Management Science 52, 311-322.
* M. C. Kennedy, A. O'Hagan (2000), Predicting the output from a complex computer code when fast approximations are
  available, Biometrika 87, 1-13 (model discrepancy, through ADR 0622).
* The series `sum 1/i^2 = pi^2 / 6` (Euler) and the union bound (Boole): first principles.
* Project documents: ADR 0119, 0121, 0240; drafts ADR 0620 to 0623; `docs/fan-project-breakdown.md` (H6),
  `docs/design-loop.md`, `docs/mcp-tools.md`, `docs/milestone-fan.md`.

## For the lead: roadmap entry

See `roadmap-entry.json` next to this file (F-701, phase 9, deps F-268, F-697, F-699, status todo). If Q1 (c) is taken, a
follow-up item for the MCP tools is needed (number from the lead; it edits both tools-list goldens and needs the user
before merge, like F-339). If Q15 (b) is taken, the deps shrink to F-268.
