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

## Addenda

Results, amendments and the gate line go here; nothing above is changed after a run.

## Amendment 1 (2026-11-16): registration, before any code of F-701 and before any run of its checks

REGISTERED 2026-11-16, written before any code of F-701 and before any run of its checks. Written by the implementing agent on
branch `f701-work` (from 2f72214d: F-698 merged with F-699 on top of F-697, F-700 and F-268), worktree `w49i-work/f701`, scratch
`w49i-work/f701-scratch`. Before this text no library or test code of F-701 existed in the branch. This amendment replaces the
section "Registration (to be fixed before any code)" above; where the two differ, this text holds. The maintainer answered the open
questions (section "Maintainer decisions"; Q6 by B2, NC2 by B3, every other question by its recommendation); every further choice
below is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and is marked "(standing
instruction)" where it is not one of the answered questions.

### What was read

`AGENT_RULES.md`; the brief; F-701's roadmap title, acceptance and note (binding, quoted below); this ADR (stub and maintainer
decisions); ADR 0620 (F-697), ADR 0621 (F-698: surrogate predictions are never evidence here), ADR 0622 (F-699: the
`fidelity_summary` entry, metrics `<m>.n_pairs` and `<m>.oos_rmse`, `settings.correction.low = {id, version}`; the strict privacy
split), ADR 0623 (F-268: `FanRomModel`, parameters `stagger_common_deg` [deg], `chord_rel` [1], `tip_gap_delta` [m]; an unknown
parameter is refused), ADR 0632 (F-700: `entry_ref`, `entry_hash`, `resolve`, `parents`, `Fidelity::Measured` written only to
private ledgers), `docs/design-loop.md`, `docs/fan-project-breakdown.md` (H6), `loop-drafts/decisions.md`,
`loop-drafts/h6-long-horizon/h6_proto.py` (the stub's prototype). Source: `design/ledger.rs` (all public items),
`design/fan_rom_eval.rs`, `design/uq.rs` (`beta_inv`, `binom_cdf`, `clopper_pearson`, the `parallel_map` pattern),
`design/sampling.rs` (`latin_hypercube`), `design/study.rs` (`parallel_map`), `design/fidelity.rs` (summary layout),
`rcfd_core::hash::fnv64` (FNV-1a 64), `rcfd_core::rng::Rng`, `rcfd_reactor::mixfrac::reg_inc_beta`.

The binding acceptance (clauses numbered as in the roadmap):

> Every limit not stated here is registered in ADR 0624 before the run, and no check uses FAN-01, a CFD run, a person or a language model. An opt-in library module (no case-file or project key, no MCP tool, no default changed) writes a hypothesis to the design ledger (ADR 0121) before its runs (claim, factor and levels, response, direction, smallest effect that matters, held metrics, domain, paired design with n and seed, evaluator id, version and tier, level spent), runs exactly the registered pairs with a ledger::Model, and writes the result (confirmed, refuted, inconclusive or void) with references to its evidence entries. (1) Order: a result is refused with a located error when an evidence entry precedes the registration line, when the number of pairs differs from the registered one, when the registration was altered, or when the evidence evaluator version differs: 4 of 4. (2) A scripted investigator that reads 60 exploratory points and registers the 12 best-looking claims per world, on 400 seeded synthetic worlds (12 parameters, 4 metrics, planted true, boundary and zero effects, known noise, 64 pairs per test), makes a false confirmation in at most 37 of the 400 worlds and confirms at least 95 percent of the large planted effects it registers; the levels spent in one ledger sum to at most 0.05 and the count continues across a reopened ledger; a single test at the boundary effect has a confirmation rate within 4 standard errors of its level; the registered negative controls fail as stated (a scripted agent that tests on the exploratory data without correction false-confirms in at least 300 of 400 worlds, one that stops at the first favourable look in at least 40 of 400, and the protocol with a constant level 0.05 exceeds 37). (3) Fidelity: on a two-tier synthetic model with planted low-tier artefacts, 0 artefact claims are returned by the lessons query at smallest tier cfd, and a re-test at the high tier refutes them with both results kept; on the reduced-order fan evaluator (rcfd_post::fan_rom) a parameter the model ignores gives paired differences of exactly 0 and the verdict refuted, and the registered monotone effect is confirmed with scope reduced_order and the model error marked unknown. (4) Equal flow: on the closed form q = n (1 + a s), tone = n^2 (1 + a s)^2 (1 + b s) with q held by n, the mean change of tone is within 1e-9 relative of the closed form for b = 0 and b = -0.1, the false claim at b = 0 is refuted, and it is confirmed when the hold is removed (negative control). (5) A result line written by hand through the ledger append is never reported: 0 of 5 returned, 5 of 5 flagged. (6) After an evaluator version change 12 of 12 results are flagged stale and 0 are returned by default. (7) A run killed at each registered point resumes with 0 duplicate and 0 missing evidence entries and a bit-identical result; the ledger is byte-identical for 1 and 8 workers; baseline_hashes, the existing design tests and both MCP tools-list goldens are unchanged.

Interpretations (standing instruction):

* Clause (1): "a result is refused" means `verify` reports the hypothesis as broken with a located message (the entry index and
  the rule), `lessons` does not return it, and `run` refuses to continue on that ledger with the same message. "The number of pairs
  differs" covers an extra pair and a missing pair under a result line. The stub's T2 (evidence spanning two versions) is the same
  rule as (1)(d) and is checked there; `run` itself refuses a model whose id, version or tier differs from the registration.
* Clause (2): "the 12 best-looking claims" are the 12 (parameter, metric) cells of largest absolute Spearman correlation over the
  60 exploratory entries (the reading aid `explore`), registered strongest first, each in its apparent direction. A false
  confirmation is a `confirmed` verdict for a cell whose true change in the registered direction is at most `min_effect` (zero,
  small, boundary and wrong-sign cells). "Its level" of the single boundary test is the level the library assigns to the first
  hypothesis of a ledger, `a_1`.
* Clause (3): "0 artefact claims returned at smallest tier cfd" is checked before the re-test (no row at all) and after it (no
  `confirmed` artefact row; the refuted `cfd` rows are returned, as they should be).
* Clause (5): checked twice: hand-written `confirmed` results for 5 hypotheses that were run (genuine results exist) and for 5 that
  were never run.

### Disclosed: smoke runs and arithmetic before this registration

All on seeds that no check below uses.

1. **Synthetic worlds (numpy prototype `f701-scratch/proto.py`, about 110 lines written from this text, not rcfd code; t quantiles
   by the Cornish-Fisher expansion of the stub's prototype; seeds 9000 to 9399; logs `proto8.log`, `proto4.log`).** W1 as in the
   stub (4 large cells, 4 boundary, 4 small) and strengthened (8 boundary cells); Spearman selection, K = 12, 64 pairs:

   | Quantity | 4 boundary cells | 8 boundary cells | Limit registered (8 cells) |
   |---|---|---|---|
   | S1 worlds with a false confirmation, series levels | 0 | 1 | at most 37 |
   | NC2 the same with level 0.05 for every hypothesis | 45 | 70 | at least 38 (exceeds 37) |
   | NC1 naive agent (exploratory r significant at 0.05) | 399 | 400 | at least 300 |
   | S2 large cells confirmed of registered | 1593 / 1594 | 1584 / 1593 (0.994) | at least 0.95 |
   | NC3 peeking agent on a boundary effect, looks 8..64 at 0.05 | 80 | 86 | at least 40 |
   | S4 4000 single boundary tests at `a_1` | 107 | 107 | 79 to 165 |

   With 4 boundary cells NC2 clears 37 by 8 (one standard error of a count near 45 is about 6.6); with 8 by 33 (about 4.3
   standard errors of a count near 70). So W1 is registered with 8 boundary cells (decision B3: the test is strengthened; the limit
   of S1 stays 37 = 20 + 4 sqrt(400 0.05 0.95), rounded down).
2. **Arithmetic, S4.** `a_1 = 0.05 * 6 / pi^2 = 0.030396`; under normal noise the paired t rule at the boundary confirms with
   probability exactly `a_1` (the one-sided t test has its size). 4000 repeats: expected 121.6, standard error `sqrt(4000 a_1 (1 -
   a_1)) = 10.86`, 4 standard errors 43.4: the registered band is 79 to 165 inclusive.
3. **Arithmetic, S1 and S3.** The levels of hypotheses 1 to 12 sum to `0.05 (6 / pi^2) sum_{i<=12} 1/i^2 = 0.0476 < 0.05`; by
   the union bound the probability of any false confirmation in a world is at most 0.05, so the count over 400 independent worlds
   is stochastically at most Binomial(400, 0.05) (mean 20, sd 4.36); 37 is the mean plus 4 standard deviations, rounded down.
4. **The reduced-order fan (temporary test `zz_f701_smoke.rs`, never committed, moved to `f701-scratch/`; log `smoke_fan.log`).**
   Set-up C of ADR 0622 (7 blades, r_hub 0.030, r_tip 0.058, tip gap 1 mm, free vortex to 45 Pa at 0.045 m^3/s, 3000 rpm, system
   through the design point, n_scan 100, `RomModel::default()`). LHS of 32 points (seeds 7_019_001 and 7_019_002) in stagger
   offset x chord_rel, tip_gap_delta at 0 and at 1e-3 m: every point ok in both arms; a repeat of the low arm is bit-identical;
   `eta_s` at 1 mm more gap minus `eta_s` is negative at every point: box [-3, 3] deg x [-0.2, 0.2]: mean -7.42e-3, sd 2.62e-3,
   largest -2.99e-3; box [-6, 6] x [-0.3, 0.3]: mean -7.32e-3, largest -6.8e-4. About 0.045 s per evaluation with 4 workers. So
   the monotone factor is `tip_gap_delta` 0 to 1e-3 m on `eta_s`, direction `decreases`, over the box [-3, 3] deg x [-0.2, 0.2],
   with `min_effect` 1e-3 (a third of the smallest change seen) and a declared resolution 1e-6. `tone1_pa` is about 1e-9 Pa in this
   set-up (a steady-loading proxy); it is the response of the ignored-parameter check F3.

### Design (fixed now; names are the implementation's)

New module `crates/rcfd-api/src/design/hypothesis.rs`, test file `crates/rcfd-api/tests/f701_hypothesis.rs`; one module line and
one doc line in `design/mod.rs`. No other source file is edited. Docs: `docs/design-loop.md` (a section with the protocol guidance
of Q12 (a), and a code-map row), `docs/fan-project-breakdown.md` (H6 row). No existing path calls the module, so every default
keeps its bits; the ledger format stays `rcfd-ledger-1` with new `kind` values only.

* **Record (D2).** `Hypothesis { claim_text, factor: Factor { name, unit, low, high }, response: Response { name, unit,
  resolution }, direction: Increases | Decreases | NoEffect, predicted_size: Option<(lo, hi)> (signed mean change high minus low,
  response unit), min_effect, held: Vec<Held { metric, unit, target: Value(t) | AsLowArm { at }, tolerance, by, by_unit, bracket:
  (lo, hi) }>, domain: Vec<Bound { name, unit, lo, hi }>, n_pairs, seed, evaluator: Scope { id, version, tier, deterministic,
  objective: Option<Identity> }, statistic: PairedT | Sign (Q8 (c), default PairedT), max_failed_share (default 0.2), origin:
  Exploratory { entries: Vec<EntryRef> } | Prior { citation }, supersedes: Option<id>, replicates: Option<id> }`. `to_json` gives
  the canonical JSON (keys sorted, numbers shortest round-trip, so `from_json` gives the same bits); the record hash is `fnv1a64:`
  and the 16 hex digits of `rcfd_core::hash::fnv64` of that text (Q17 (a)). The claim text is stored verbatim and never
  interpreted (Q16 (a)).
* **Registration entry.** `register(ledger, &Hypothesis) -> Result<Registered, String>` reads the ledger, validates, and appends one
  entry: kind `hypothesis`, evaluator `rcfd.hypothesis` version `1`, study `hypothesis`, variant = id, tier = the registered tier,
  verdict ok, no params or metrics, `settings = {record, hash, id, family: {slot, level, batch}, derived_seed}`, `provenance =
  {driver: hypothesis, id}`, `seed` = the derived seed, `parents` = the origin entries. The id is `H-<n>`, n the number of
  `hypothesis` entries of the driver already in the file plus one. The family slot `i` is one plus the number of slot-opening
  registrations before it (every single registration and the first member of a batch open a slot; withdrawn and void ones count);
  the level is `a_i = 0.05 * 6 / (pi^2 i^2)` (D6, B2; the family is the ledger file, Q7 (a)). The design seed is `fnv64` of the
  text `<record hash>:<seed>` (D4).
* **Validation**, refused with a located message (the field named) and nothing appended: empty claim text; a name or unit empty;
  factor `low >= high` or not finite; `min_effect` not finite or `<= 0`; resolution not finite or `< 0`; `min_effect < 10
  resolution` (k_res = 10); `n_pairs < 16`; a domain bound not finite or `lo >= hi`, a duplicate name, the factor in the domain;
  more than one held metric (not covered, see the limits); a held metric equal to the response; `by` equal to the factor or in the
  domain; a tolerance not finite or `<= 0`; a bracket not finite or `lo >= hi`; `AsLowArm.at` outside the bracket;
  `max_failed_share` outside `[0, 1)`; an origin reference that does not resolve (`ledger::resolve`, index and hash); `supersedes`
  or `replicates` naming no hypothesis of the ledger; a `supersedes` chain that already holds 2 re-tests (Q10 (c)); a predicted
  size not finite or `lo > hi`; an evaluator id or version empty.
* **Batch (B2, Holm as an option).** `register_batch(ledger, &[Hypothesis])` appends k registrations in one write; they share one
  slot and its level `a_i`; `settings.family.batch = {first, size, member}`. A member's verdict is decided by Holm's step-down
  procedure (Holm 1979) over the batch at `a_i`: p-values sorted ascending (ties: member order), the r-th is confirmed when its
  bound at level `a_i / (k - r + 1)` meets the confirm rule, and the procedure stops at the first that does not; refuted is judged
  at `a_i / k`. A batch's results are written only when every member's evidence is complete.
* **Withdrawal.** `withdraw(ledger, id, reason)` appends kind `hypothesis_withdrawn`; refused after the first evidence entry. It
  keeps its slot.
* **Design (D5).** `design_points(record, derived_seed)` = `latin_hypercube(n, domain.len(), derived_seed)` scaled to the domain
  bounds. Pair `p`, arm low: the factor at `low`, the domain point `p`; arm high: the factor at `high`. Parameter order in an
  evidence entry: factor, domain in registered order, then `by`. With a held metric each arm solves `by` by bisection in the
  bracket: evaluate both ends (an end within tolerance is accepted), require a sign change of `metric - target`, then at most 200
  halvings until `|metric - target| <= tolerance` or the midpoint equals an end; the evaluation at the accepted point is the
  evidence entry; an unbracketed or exhausted solve leaves the last evaluation as the evidence entry (it does not match). Target
  `Value(t)`: both arms solve; `AsLowArm { at }`: the low arm uses `by = at`, the high arm solves to the low arm's value.
* **Run.** `run(ledger, id, model, &RunOptions { workers (default 1), chunk (pairs per append, default 8) }) -> Result<RunReport,
  String>`. Refused (located) when the registration is broken (verify), withdrawn, or the model's id, version or tier differs from
  the registration. Evidence entries: kind `evaluation`, the model's identity, study `hypothesis`, variant `<id>#p<pair>.<arm>`,
  seed the derived seed, parents the registration, provenance `{driver: hypothesis, hypothesis, hash, pair, arm, iterations}`.
  Existing evidence of the hypothesis is kept; only missing (pair, arm) are evaluated, by `parallel_map` over the pairs of a chunk,
  appended in pair and arm order (so the bytes depend neither on the workers nor on the chunk). A panicking evaluation is a failed
  arm. Then the ledger is read back, the hypothesis verified and, when its slot is complete, one `hypothesis_result` per member
  without a genuine result is appended. A second call after completion appends nothing and returns the result.
* **Test (D5, Q5 (b), Q8 (c)).** `d_p = s (y_high - y_low)` over the ok pairs, `s = -1` for `decreases`, else +1. PairedT: mean
  `m`, `sd` with divisor `n - 1`, `SE = sd / sqrt(n)`, `t = t_{1 - a, n - 1}` (`student_t_quantile`, from `beta_inv(2 a, nu / 2,
  1/2)`), `L = m - t SE`, `U = m + t SE`. Increases or decreases: confirmed `L >= min_effect`; refuted `U < min_effect` (flag
  `opposite` when `U < 0`); else inconclusive. No effect (two one-sided tests, Schuirmann 1987): confirmed `-min_effect < L` and
  `U < min_effect`; refuted `L >= min_effect` or `U <= -min_effect`; else inconclusive. p-value of the confirm test `1 - F_t((m -
  min_effect) / SE)` (no effect: the larger of the two one-sided p-values); with `SE = 0` the bounds equal `m` and p is 0 or 1 by
  the rule. Sign: `k+` = pairs with `d > min_effect`, `k-` = pairs with `d < min_effect`; confirmed when `P(Bin(n, 1/2) >= k+) <=
  a`, refuted when `P(Bin(n, 1/2) >= k-) <= a` (opposite when the same holds for `d < 0`); no effect by the two one-sided sign
  tests. Void: fewer than 2 ok pairs; a failed share (pairs with a failed arm over n) above `max_failed_share`; any ok arm whose
  held metric is not within tolerance of its target (recomputed from the stored metric, not from the provenance). Also reported:
  the share of pairs with `d > 0` and its Clopper-Pearson 0.95 interval, the smallest and largest `d` with their pairs, whether
  every `d` is exactly 0, failures per arm, overlap of `s [L, U]` with `predicted_size`, and `spread: domain` (deterministic) or
  `noise and domain`.
* **Result entry.** Kind `hypothesis_result`, evaluator `rcfd.hypothesis` 1, tier the registered one, variant the id, verdict ok,
  metrics `mean, sd, se, t, lower, upper, p_value, level, n_ok, n_failed_low, n_failed_high, sign_share, d_min, d_max` (response
  unit or `1`), `settings = {verdict, opposite, void_reason, scope, family, record_hash, statistic, ...}`, parents = the
  registration then every evidence entry in pair and arm order (index and hash), provenance `{driver, hypothesis, hash}`.
* **Verify (D3, D7, Q14 (b)).** `verify(entries, id, rerun: Option<&dyn Model>) -> Verification`. Rules, each broken one a located
  problem: the registration is a driver entry, its id unique, its stored hash equal to the hash of its stored record, its slot and
  level equal to the recount; every evidence entry (provenance hypothesis = id) has an index greater than the registration's, the
  registered hash, the registered evaluator id, version and tier, a pair in `0..n` and an arm, no (pair, arm) twice, parameters
  equal to the design bit for bit (factor level, domain point; `by` inside the bracket, `= at` for an `AsLowArm` low arm); a
  result line is genuine when its metrics, settings and parents equal the recomputation exactly; every other `hypothesis_result`
  line of the id is flagged (stored verdict, parents or numbers differ); a genuine result needs exactly 2 n evidence entries. With
  `rerun` given (same id and version) and tier analytic or reduced_order, every evidence entry is re-evaluated with
  `ledger::rerun_check` (rtol 0, atol 0) and a mismatch is a problem; otherwise the lesson says `evidence not re-run`.
* **Lessons (D8).** `lessons(entries, &LessonQuery, rerun: &[&dyn Model]) -> Lessons { rows, flagged }`. One row per hypothesis
  that verifies and has a genuine result: the generated statement (built from the record, never from the claim text), the claim
  text labelled as the author's words, verdict and flag `opposite`, scope, model error, family slot and level, the registration,
  result and evidence indices, `stale`, `replicates`, `replicated_by`, `evidence_rerun`, the statistics. Tier analytic is never
  returned. Query: factor, response, verdict, `min_tier` (order analytic < reduced_order < cfd < measured, Q4 (a) with F-700's
  value), `include_stale` (default false), `current` (evaluator id and version pairs the caller holds now; a row is stale when its
  scope's (id, version) is not among them; read time only, Q9 (a)), `declared_errors` (evaluator id, version, metric, value; Q13
  (b)), `trust_stored` (default false; a negative-control switch documented as such: rows from the stored verdict of the last
  result line, no recomputation). Model error: the `<response>.oos_rmse` of the last `fidelity_summary` whose
  `settings.correction.low` is the scope's (id, version) and whose `<response>.n_pairs >= 1`; else a declared value; else
  `unknown`. Flag `below_model_error` when `|m| <` the value, `above_model_error` otherwise. The statement names the tier,
  evaluator and version, the factor levels, the mean change with its bounds, the sign count, the held metrics, the verdict with its
  slot, the model-error state and whether a replicate at a higher tier exists ("Not tested at <tier>"); when every `d` is exactly
  0 it says that the model does not respond to the factor over the domain.
* **Explore (D1).** `explore(entries, &Filter) -> Exploration { exploratory: true, cells }`: Spearman rank correlation (average
  ranks for ties) of every parameter with every metric over the matching `ok` entries that hold both; cells sorted by `|rho|`
  descending (ties: parameter then metric order). Never evidence.
* **Statistics exposed for checks and the negative controls:** `series_level(i)`, `student_t_quantile(p, nu)`, `student_t_cdf(t,
  nu)`, `paired_test(d, level, min_effect, direction, statistic) -> (Verdict, Stats)`.

### Set-ups (fixed now)

Every ledger uses the fixed clock `1_700_000_000_000`. Synthetic noise is a deterministic function of the point: `z = sqrt(-2 ln
(1 - u1)) cos(2 pi u2)` with `u1, u2` the first two `uniform()` draws of `Rng::new(fnv64(bytes) ^ world_seed ^ ((metric + 1) *
0x9E37_79B9_7F4A_7C15))`, `bytes` the little-endian bit patterns of the parameter values in order (so a repeat gives the same
bits, `rerun_check` works, and distinct points give independent draws).

* **W1 planted linear world.** Parameters `x0..x11` [1] on [0, 1], metrics `y0..y3` [1], `y_j = sum_k B_kj x_k + z_j` (sd 1).
  `Rng::new(world_seed)` shuffles the 48 cells; the first 4 get slope 3 (change 1.5 between 0.25 and 0.75), the next 8 slope 1
  (change 0.5 = `min_effect`, boundary), the next 4 slope 0.5 (change 0.25); each sign by `below(2)` in that order after the
  shuffle; the other 32 cells are 0. Exploration: 60 points, `latin_hypercube(60, 12, world_seed ^ 0xE)`, appended as `evaluation`
  entries (study `explore`). Protocol agent: `explore`, the 12 strongest cells, registered strongest first (factor `x_k` 0.25 to
  0.75, response `y_j` resolution 0, `min_effect` 0.5, domain the other 11 parameters on [0, 1], 64 pairs, seed `world_seed +
  cell`, PairedT, origin the 60 exploration entries), each run right after its registration (workers 1). Evaluator `synthetic.w1`
  version `1`, tier analytic (reduced_order where a check needs `lessons`, stated). World seeds `701_000 + w`, w = 0 to 399.
* **W1b single boundary cell.** `x0, x1` [1] on [0, 1], `y = x0 + z` (sd 1); factor `x0` 0.25 to 0.75 increases, `min_effect`
  0.5, domain `x1`, 64 pairs; the first hypothesis of a fresh ledger (slot 1). S4: seeds `702_000 + r`, r = 0 to 3999. NC3:
  seeds `703_000 + r`, r = 0 to 399.
* **W2 two tiers.** `x0..x5` [1] on [0, 1], metric `y` [1], noise sd 0.25 (world seed 701_601 low, 701_602 high). Slopes low / high:
  `x0` 3 / 0, `x1` 3 / 0, `x2` 3 / -3, `x3` -3 / 3 (artefacts, 4), `x4` 3 / 3 (genuine), `x5` 0 / 0. Low `synthetic.w2.low` 1 tier
  reduced_order, high `synthetic.w2.high` 1 tier cfd. Claims (origin prior): `x0..x4` on `y` in the low model's direction, 0.25 to
  0.75, `min_effect` 0.5, 64 pairs, seeds 701_610 + k; re-tests at the high tier with `replicates` and seeds 701_620 + k.
* **W3 equal flow.** Parameters `s` [1], `n` [1/s], `c` [1] (ignored by the model); `q = n (1 + 0.5 s)` [m3/s], `tone = n^2 (1 +
  0.5 s)^2 (1 + b s)` [Pa]; factor `s` 0 to 1; domain `c` on [0, 1]; 32 pairs; `min_effect` 0.01, resolution 0. Held `q` [m3/s]
  target `Value(1.0)` tolerance 1e-13 by `n` bracket [0.1, 10]. Case A `b = 0`, claim "increases" (seed 701_501); case B `b =
  -0.1`, claim "decreases" (701_502); E2b: case B with `AsLowArm { at: 0.8 }` (701_503); NC4: case A with no held metric and `n`
  in the domain on [0.5, 1.5] (701_504); E3: case B with bracket [0.1, 0.5] (701_505), then one more registration in the same
  ledger. Evaluator `synthetic.w3` 1, tier analytic.
* **W4 the reduced-order fan.** Set-up C of smoke 4 through a test-local wrapper model with id `rcfd.fan_rom+dummy` and version
  `FAN_ROM_MODEL_VERSION`, tier reduced_order, that drops the parameter `sweep_deg` [deg] and passes the rest to `FanRomModel`.
  F3: factor `sweep_deg` 0 to 20, response `tone1_pa` [Pa] resolution 1e-15, `decreases`, `min_effect` 1e-12, domain
  stagger_common_deg [-3, 3], chord_rel [-0.2, 0.2], tip_gap_delta [0, 5e-4], 32 pairs, seed 701_401. F4: factor `tip_gap_delta`
  0 to 1e-3, response `eta_s` resolution 1e-6, `decreases`, `min_effect` 1e-3, domain stagger_common_deg [-3, 3], chord_rel
  [-0.2, 0.2], sweep_deg [0, 20], 32 pairs, seed 701_402. Both in one ledger (slots 1 and 2), workers 4, `verify` with the model
  for the re-run.

### Checks (limits fixed now)

All in `crates/rcfd-api/tests/f701_hypothesis.rs`, release profile.

| Id | Clause | What | Limit |
|---|---|---|---|
| K1 | — | `student_t_quantile` against the closed forms nu = 1 (`tan(pi (p - 1/2))`) and nu = 2 (`(2p - 1) / sqrt(2 p (1 - p))`) at p = 0.9, 0.95, 0.99, 0.9999, 1 - 1e-6; `student_t_cdf(quantile)` at nu = 63 | relative <= 1e-9; cdf within 1e-12 of p |
| K2 | — | `paired_test` on fixed vectors of 16 values: m, sd, SE, L, U, p against the formulas evaluated in the test; verdicts for an inconclusive vector (m > min_effect, L < min_effect), the same shifted to confirmed, to refuted with opposite, and the no-effect cases; the sign-test p-values against the binomial sum written in the test | relative <= 1e-12; verdicts exact |
| L1 | (2) | `series_level(i)`, i = 1..12, and the levels stored by 12 registrations | equal to `0.05 * 6 / (PI^2 i^2)` bit for bit; sum <= 0.05 |
| G1 | — | a registration read back through a new `Ledger` handle: `Hypothesis::from_json` equals the original; stored hash equals the recomputed one and the one computed in the test | equal |
| G2 | (1) | on copies of a completed W1 ledger: (a) one evidence line moved before the registration, (b1) a 65th pair appended before the result, (b2) the last pair's two lines removed, result kept, (c) the registration's `min_effect` edited in the file text, (d) one evidence line's `evaluator_version` edited | each refused by `verify` and by `run` with a message naming the entry index and the rule: 4 of 4 clauses (5 of 5 cases) |
| G3 | (5) | W1 world at tier reduced_order: (a) 5 hypotheses run, then a hand-written `confirmed` result line for each (`Ledger::append`); (b) 5 hypotheses registered and never run, the same lines | each: 0 of 5 forged verdicts returned by `lessons`; 5 of 5 forged lines flagged |
| NC5 | (5) | G3 with `trust_stored = true` | the 5 forged `confirmed` verdicts returned (the control must fail G3) |
| G4 | (7) | W1 hypothesis of 64 pairs (128 evidence entries): the completed file cut after the registration and k evidence lines, k = 0, 1, 63, 127, 128, and k = 63 plus half of the next line (torn); `run` again | 0 duplicate and 0 missing (pair, arm); the result line bit-identical to the uninterrupted run; the whole file byte-identical for the untorn cuts |
| G5 | — | evidence written by an impostor model with the registered id and version (values + 0.3) and its result | `verify` with the real model: a problem per evidence entry; `lessons` with the real model: 0 rows; without: 1 row with `evidence_rerun = None` |
| V1 | — | each refusal of the validation list (17 cases) | refused, the message names the field; the file unchanged |
| S1 | (2) | W1, 400 worlds, protocol agent: worlds with a false confirmation | at most 37 |
| S2 | (2) | W1: large cells registered and confirmed | at least 0.95 of those registered |
| S3 | (2) | W1: the sum of the stored levels of each world's ledger; a ledger with 3 registrations, a new handle, a 4th registration | <= 0.05 for every world; the 4th gets slot 4 and `a_4` |
| S4 | (2) | W1b, 4000 fresh ledgers, one test each at `a_1`: confirmations | 79 to 165 |
| S5 | (2) | W1: registered exactly-zero cells confirmed | 0 |
| NC1 | (2) | W1 naive agent on the same 12 cells: Pearson r over the 60 exploratory points significant at 0.05 one-sided in the apparent direction (`t = r sqrt(58 / (1 - r^2)) >= t_{0.95, 58}`) | false confirmation in at least 300 of 400 worlds |
| NC2 | (2) | W1: the protocol agent's evidence judged with `paired_test` at level 0.05 for every hypothesis | false confirmation in at least 38 of 400 worlds (exceeds 37) |
| NC3 | (2) | W1b, 400 runs, the peeking agent: `paired_test` at 0.05 on the first k pairs of the evidence, k = 8..64, stop at the first `confirmed` | at least 40 of 400 |
| F1 | (3) | W2 low-tier results: the 4 artefact claims confirmed with scope reduced_order and `model_error_unknown`; `lessons` with `min_tier` cfd | 0 rows before the re-test; after it 0 `confirmed` artefact rows |
| F1b | (3) | W2 with a declared model error 2.0 for `y` of the low model | the 4 artefact rows flagged `below_model_error`, source declared |
| F1c | (3) | W2 after `fidelity::run` (low = W2 low, high = W2 high through `ModelSource`, 6 parameters on [0, 1], candidates `Lhs { 60, 701_630 }`, metric `y` floor 1e-3, 1 round of 6, objective `y` Max) in the same ledger | model error of the low rows = the last summary's `y.oos_rmse` bit for bit, source naming its index |
| F2 | (3) | W2 re-tests at tier cfd with `replicates` | 4 of 4 artefacts refuted (x2, x3 with `opposite`), x4 confirmed; `lessons` returns both rows of each of the 4 claims, unmerged, linked by `replicates` / `replicated_by` |
| F3 | (3) | W4 `sweep_deg` | every `d` exactly 0; refuted; the statement says the model does not respond |
| F4 | (3) | W4 `tip_gap_delta` on `eta_s` | confirmed, scope reduced_order, model error unknown; the statement says "Not tested at cfd"; the re-run of all 64 evidence entries passes |
| F5 | (3) | W4 registration with `min_effect` 5e-6 (< 10 x 1e-6) | refused, the message names `min_effect` and the resolution |
| E1 | (4) | W3 case A: mean change of tone; verdict | `|mean| <= 1e-9` (q0^2 = 1); refuted |
| E2 | (4) | W3 case B: mean change against `-0.1 q0^2 (1 - 0)`; verdict | relative <= 1e-9; confirmed |
| E2b | (4) | W3 `AsLowArm { at: 0.8 }`: mean change against `-0.1 * 0.8^2` | relative <= 1e-9; confirmed |
| NC4 | (4) | W3 case A without the hold | confirmed (the control must fail E1) |
| E3 | (4) | W3 bracket [0.1, 0.5] | void, the reason names a pair, an arm and `q`; the next registration gets the next slot |
| T1 | (6) | W1 world at tier reduced_order (12 results), `current` with version `2` | 12 of 12 stale; 0 returned by default; 12 returned with `include_stale` |
| HB1 | — | batch of 4 (members with designed effects: change 2.0, a change whose p lies in `(a/4, a/3]`, the boundary 0.5, 0.4; deterministic `d = mu + (x1 - 0.5)`, x1 the first domain parameter, `min_effect` 0.5, 64 pairs) | one slot for 4 members, the next single registration the next slot; verdicts equal Holm computed in the test from the stored p-values; the second member confirmed by Holm (the set-up requires its p in `(a/4, a/3]`, asserted as a set-up condition); no result written before the last member's evidence |
| X1 | — | `explore` on a 20-entry fixture against Spearman computed in the test, with a tie | within 1e-12; ordering; `exploratory = true` |
| R1 | (7) | one W1 world (12 hypotheses) and W3 case B, workers 1 and 8 | ledger bytes equal |
| R2 | (7) | existing: `cargo test -p rcfd-api --release --lib design::`, `--test f697_fan_objective`, `--test f699_fidelity`, `--test fan_rom_bands_f268`, `--test f249_default_bits`, `--test f260_default_bits`, `--test f297_default_bits`, `--test baseline_hashes`; rcfd-mcp `default_f1`, `tool_list_f275`, `project_list_f270`; `git diff 2f72214d -- crates/rcfd-mcp crates/rcfd-api/tests/baseline_hashes.rs` | pass; empty diff; no existing test file edited |
| R3 | — | `cargo fmt -p rcfd-api --check`, `cargo clippy -p rcfd-api --all-targets -- -D warnings` | clean |

S1, S2, S5, NC1 and NC2 run in one heavy test over the 400 worlds, S4 in another (4000 ledgers); both are `#[ignore]`d heavy
records run once; fast counterparts run the same code path on worlds 0 to 7 and on 100 repeats (seeds as registered) with only
structural assertions (S3, every registered cell has a result, no skipped ledger line). NC3 runs in a fast test if it takes under
about two minutes, else with the heavy records.

Negative controls: NC1 to NC5 above (each must fail as stated, or the check it guards proves nothing). The stub's NC6 (the protocol
agent counting the exploratory entries as evidence) is the naive agent of NC1 with selection, and the guard against it is G2 (a)
(evidence before the registration is refused); it is not registered separately.

Mutations (scratch copy, one at a time, each must fail a registered fast check):

| Id | Mutation | Expected to fail |
|---|---|---|
| M1 | sign of `d` flipped | E2, F4 |
| M2 | `sd` with divisor n | K2 |
| M3 | two-sided quantile `t_{1 - a/2}` in the one-sided bounds | K2 |
| M4 | the slot not advanced by a void or withdrawn hypothesis (only hypotheses with a non-void result counted) | E3 |
| M5 | slot restarted per handle (always 1) | L1, S3 |
| M6 | order rule removed (evidence before the registration accepted) | G2 (a) |
| M7 | staleness compared on id only | T1 |
| M8 | held tolerance ignored (an unmatched arm counts as matched) | E3 |
| M9 | `min_effect` compared with `m` in place of `L` | K2 |
| M10 | stored verdict trusted by default | G3 |
| M11 | pair-count rule removed in `verify` | G2 (b) |
| M12 | registration hash not checked | G2 (c) |
| M13 | re-run ignored in `verify` | G5 |
| M14 | Holm replaced by Bonferroni (`a / k` for every member) | HB1 |
| M15 | evidence parameters not compared with the design | recorded which check, if any |

### Decision rule

* **done:** every check above met, NC1 to NC5 failing as stated, and every mutation M1 to M14 failing a check (M15 is recorded, not
  judged: a forged parameter also breaks the evidence hash references, so no check is expected to isolate it).
* **partial:** every guard check (G1 to G5, V1, S1, S3, S4, S5, F1, F2, F3, E1 to E3, T1, R1) met, and S2, F4, HB1 or X1 missed (an
  ignored `*_known_miss` record with the limit intact), or a negative control does not fail, or a mutation M1 to M14 survives.
* **todo** (the lead does not take the code): a guard check missed.

### Limits (not covered)

* Library only (D1): no service method, no MCP tool, no case-file or project key (Q1 (c), Q3 (a); F-702). The assistant cannot
  call it from a session yet, so H6 becomes `partial`, not done.
* One factor at two levels per hypothesis; one held metric at most (the record holds a list; solving several coupled holds is not
  built); no interactions, dose-response shapes or claims about optima.
* The resolution floor uses the declared resolution only; the stub's second floor (the change of the response caused by the
  held-metric tolerance) is not computed.
* `verify` re-runs all evidence (Q14 (b)) or none; the seeded sample for expensive tiers (Q14 (c)) is not built.
* No online false-discovery-rate rule (Q6 (c)); no automatic re-test; no `hypothesis_stale` marker (Q9 (a)).
* Every check uses synthetic evaluators or the reduced-order fan. A confirmation on the reduced-order tier is a statement about
  that model (assumed loss constants), never about a fan. The checks show that the record and its guards work for scripted agents
  that are greedy in one fixed way, not that an assistant uses them well (risk 1).
* The hash is FNV-1a 64: it detects accidental edits; a deliberate rewrite of the ledger defeats every guard (risk 8).

## Addendum 1: results (2026-11-16)

Code: commit adc35501 (registration dc351026 before it). New `crates/rcfd-api/src/design/hypothesis.rs` (the design of
amendment 1), `crates/rcfd-api/tests/f701_hypothesis.rs` (21 tests: 19 fast, the two heavy W1 and W1b tests ignored), one module
line and doc lines in `design/mod.rs`; docs `docs/design-loop.md` (one section with the protocol guidance of Q12, one code-map row,
one "Not delivered" bullet for the MCP tools of F-702) and `docs/fan-project-breakdown.md` (H6 row: "partial (F-701, ADR 0624:
the record, registered paired tests and the lessons query in the library; no MCP tool until F-702, and every confirmation so far
is about the reduced-order model)"). No other source file, no shared solver file, no golden, no hash table, no case key, no MCP
tool.

Runs: the registered run `cargo test -p rcfd-api --release --test f701_hypothesis -- --test-threads=2` (log
`f701-scratch/run1.log`): 19 passed. The two heavy tests once each (`--ignored`, `heavy.log`: W1 177.9 s, W1b 76 s). After
`cargo fmt` and one clippy change (`map_or(true, ..)` to `is_none_or`, same logic) the fast tests ran once more to confirm
(`run2_after_fmt.log`, 19 passed). R2 in `r2.log`. Mutations on a `git archive` copy of adc35501 with its own target directory
(`mutate.py`, `mutate_summary.log`; the unmutated control 19 of 19). Release profile throughout.

| Check | Limit | Measured | Met |
|---|---|---|---|
| K1 | rel <= 1e-9; cdf within 1e-12 | largest rel 1.33e-11; cdf error 0 | yes |
| K2 | rel <= 1e-12; verdicts exact | rel 0; verdicts exact; sign-test p 0.4018 (9 of 16) equal to the binomial sum | yes |
| L1 | bit-equal to `0.05 * 6 / (PI^2 i^2)`; sum <= 0.05 | 12 of 12 bit-equal; sum 0.047570 | yes |
| G1 | equal | record equal; hash `fnv1a64:4b58385209778c10` stored = recomputed = test's | yes |
| G2 | 5 of 5 refused by `verify` and `run`, index and rule named | 5 of 5, for example "entry 60: evidence of H-1 precedes its registration (entry 61)", "entry 189: ... pair 64, outside the registered 64 pairs", "entry 187: result of H-1 with evidence for 126 of the registered 128 ...", "entry 60: the registration of H-1 was altered ...", "entry 61: evidence evaluator synthetic.w1 version '2' ..." | yes |
| G3 | 0 of 5 returned, 5 of 5 flagged, (a) and (b) | (a) 0 and 5; (b) 0 and 5 | yes |
| NC5 | 5 forged verdicts returned | 5 of 5 | yes (fails G3 as it must) |
| G4 | 0 duplicate, 0 missing, result bit-identical, file identical (untorn) | cuts 0, 1, 63, 127, 128: 0, 0, identical, identical; torn at 63: result identical, 1 skipped line | yes |
| G5 | a problem per entry; 0 rows with re-run; 1 row with `None` without | 128 of 128; 0 rows; 1 row, `evidence_rerun = None` | yes |
| V1 | refused, field named, file unchanged | 22 of 22 (deviation: 22 cases, the registration said 17) | yes |
| S1 | <= 37 of 400 | 1 | yes |
| S2 | >= 0.95 | 1592 of 1595 = 0.998 | yes |
| S3 | <= 0.05 per world; 4th registration slot 4 | largest sum 0.047570; slot 4 with `a_4` after a new handle | yes |
| S4 | 79 to 165 of 4000 | 118 (at `a_1` 0.030396; z -0.33) | yes |
| S5 | 0 | 0 | yes |
| NC1 | >= 300 of 400 | 400 | yes (fails as it must) |
| NC2 | >= 38 of 400 | 61 | yes (fails as it must) |
| NC3 | >= 40 of 400 | 75 | yes (fails as it must) |
| F1 | artefacts confirmed reduced_order, model error unknown; 0 rows at `min_tier` cfd before; 0 confirmed artefact rows after | 5 low rows, as stated; 0 rows; after the re-test 5 rows, 0 confirmed artefact | yes |
| F1b | 4 artefact rows `below_model_error`, declared | 4 of 4 | yes |
| F1c | model error = last summary's `y.oos_rmse`, bit for bit, index named | 5 of 5 rows take 7.0060 from entry 1372 | yes |
| F2 | 4 of 4 refuted (x2, x3 opposite), x4 confirmed at cfd; rows kept and linked | as registered; 4 of 4 linked both ways | yes |
| F3 | every d 0; refuted; "does not respond" | as registered | yes |
| F4 | confirmed, reduced_order, model error unknown, "Not tested at cfd", re-run passes | mean change -7.3941e-3, bounds (-8.4664e-3, -6.3218e-3) at level 7.599e-3; re-run `Some(true)`; statement as registered | yes |
| F5 | refused, `min_effect` and resolution named | "min_effect: 0.000005 is below 10 times the declared resolution 0.000001 of 'eta_s'" | yes |
| E1 | abs(mean) <= 1e-9; refuted | -3.553e-15; refuted | yes |
| E2 | rel <= 1e-9; confirmed | 8.38e-14; confirmed | yes |
| E2b | rel <= 1e-9; confirmed | 4.16e-13; confirmed | yes |
| NC4 | confirmed | mean 1.3553, confirmed | yes (fails E1 as it must) |
| E3 | void, pair, arm and `q` named; next slot | "pair 0 arm low: held metric 'q' = 0.5 not within 1e-13 of the target 1"; next H-2 at slot 2 | yes |
| T1 | 12 stale, 0 by default, 12 with `include_stale` | 12, 0, 12 | yes |
| HB1 | one slot; verdicts = Holm by hand; member 2 confirmed by Holm; no early result | set-up `mu_b` 0.587869 (p 0.009379 in (a/4 0.00760, a/3 0.01013]); p [1.39e-47, 0.009379, 0.5035, 0.9963]; [T, T, F, F] = by hand; 0 early results; next registration slot 2 | yes |
| X1 | within 1e-12; ordering; exploratory | 4 cells, error 0, sorted, `exploratory = true` | yes |
| R1 | bytes equal, workers 1 and 8 | W1 world 2347886 bytes, W3 B 56361 bytes, equal | yes |
| R2 | pass; empty diff | `design::` 76 passed (3 ignored), f697 22, fan_rom_bands_f268 4, f249 4, f260 1, f297 3, baseline_hashes 3, rcfd-mcp default_f1 2, tool_list_f275 7, project_list_f270 1; diff empty. f699_fidelity: 15 passed, 1 failed, `p4_table_errors_name_row_and_column_never_a_value` (P4 leaked "168"): the temporary path `rcfd_f699_p4_41688` holds the cell text "168", the process-id dependence of ADR 0622 question 3; no file of F-699 is changed (diff empty). A diagnostic run of that one test (`r2_p4_diag.log`, pid 42436) passed | no, as written (see the decision) |
| R3 | fmt and clippy clean | `cargo fmt -p rcfd-api --check` clean; `cargo clippy -p rcfd-api --all-targets -- -D warnings` clean | yes |

Mutations (each a one-place change, the tests run once; caught = at least one test fails):

| Id | Mutation | Expected | Failing tests | Caught |
|---|---|---|---|---|
| M1 | sign of `d` flipped | E2, F4 | E1-E3/NC4, F1/F2, HB1, NC3, F3-F5 | yes |
| M2 | `sd` with divisor n | K2 | K2 | yes |
| M3 | two-sided quantile | K2 | K2, HB1 | yes |
| M4 | slot not advanced by a void or withdrawn hypothesis | E3 | E1-E3, HB1, L1/S3 | yes |
| M5 | slot restarted per handle | L1, S3 | L1/S3 and 8 others | yes |
| M6 | order rule removed | G2 (a) | G2 | yes |
| M7 | staleness on id only | T1 | T1 | yes |
| M8 | held tolerance ignored | E3 | E1-E3 | yes |
| M9 | `min_effect` against `m` | K2 | K2 | yes |
| M10 | stored verdict trusted by default | G3 | G3/NC5, G5 | yes |
| M11 | pair-count rule removed | G2 (b) | G2 | yes |
| M12 | registration hash not checked | G2 (c) | G2 | yes |
| M13 | re-run ignored in `verify` | G5 | G5, F3-F5 | yes |
| M14 | Holm replaced by Bonferroni | HB1 | HB1 | yes |
| M15 | evidence parameters not compared with the design | recorded | none (19 of 19 pass) | recorded |

M15 survives, as amendment 1 expected: no check isolates a forged evidence parameter.

Deviations: (1) one development run before the registered run, of a copy of the test file with other seeds (prefixes 751, 752
and 753 in place of the registered ones; not committed): 19 passed. The seed-free checks K1, K2 and X1 (and the structure of V1)
therefore ran twice with the same deterministic inputs; disclosed. (2) V1 has 22 cases, not the 17 written (more cases, same
limit). (3) The fast tests ran a second time after fmt and clippy (same result). (4) The diagnostic run of the one f699 test
above. Not run: the whole workspace (the lead's gate), the heavy tests more than once, any rcfd-mcp test beyond the three listed.

Decision by the registered rule: **partial**. Every check from K to R1 and R3 is met, NC1 to NC5 fail as they must, and M1 to
M14 are all caught. R2 as written ("pass") is not met: one f699 test failed for a reason that depends on the process id and
lies in a file this item does not touch. The rule does not name R2 among the guard checks. "Done" requires every check, so the
rule gives at most partial; it does not give todo, because every guard check is met.

### Remaining questions (each with a recommendation)

1. **R2 and the f699 P4 fragility.** Options: (a) keep partial; (b) count R2 as met, because the failing test is in a file with
   an empty diff and passed under another process id. Recommendation: (a). H6 stays partial anyway until F-702 adds the MCP
   tools. The fix (P4 searches its messages with the file path removed) is already the follow-up proposed in ADR 0622.
2. **Wording of the statement.** "pairs with d > 0" in a statement means pairs in the claimed direction (the sign is flipped
   for `Direction::Decrease`). Options: (a) keep; (b) print the sign of the claimed direction. Recommendation: (b), a text change
   with the MCP tools of F-702.
3. **Seeded re-run sample for expensive tiers (Q14 (c)).** `verify` re-runs all or none. Options: (a) keep; (b) a registered
   sample of k evidence pairs with a derived seed. Recommendation: (b) before the first re-run at cfd, as a follow-up.
4. **Several held metrics and the second resolution floor.** One held metric is solved; the floor from the held tolerance is not
   computed. Options: (a) keep; (b) a follow-up with coupled holds and the second floor. Recommendation: (b) when a fan
   hypothesis needs two holds (flow and speed together).
5. **M15.** Options: (a) keep recorded; (b) a check that edits one evidence parameter and re-hashes the reference chain, so the
   parameter comparison is the guard that decides. Recommendation: (b) inside the follow-up of question 3.

## For the lead: roadmap entries

> Proposed status: partial

> Part H6 of docs/fan-project-breakdown.md. ADR 0624, 2026-11-16. PARTIAL (every F-701 check met, M1 to M14 caught; R2 missed only by the process-id dependence of an f699 test, file unchanged). Library only, opt-in (no case key, no MCP tool, no default changed): design::hypothesis records a hypothesis (claim, factor at two levels, response with resolution, direction, predicted size, min_effect at least 10 times the resolution, held metric with bisection, domain, pairs at least 16, seed, evaluator scope, statistic) in the design ledger before any evidence, gives it the next level of the family series 0.05 * 6 / (pi^2 i^2) (Holm inside a registered batch), runs paired evidence with resume after a cut, and judges a one-sided t (or sign) test against min_effect. Verdicts are never trusted from the file: verify and lessons recompute them from the evidence, check order, pair count, registration hash and evaluator version, and optionally re-run the evidence; lessons scope every row to its tier, model error (unknown, declared or taken from a fidelity summary) and staleness, and keep replications unmerged. Checks: t quantiles against closed forms within 1.3e-11; 400 synthetic worlds with 1 false confirmation (limit 37) and 0.998 of large effects found, while the naive agent confirms falsely in 400, the unadjusted one in 61 and the peeking one in 75; 4000 single tests at a_1 give 118 (79 to 165); all 5 forged results ignored (returned with trust on); a cut run resumes byte-identical; an impostor evaluator is caught by re-run; low-tier artefacts are scoped and refuted at the high tier; on the reduced-order fan, +1 mm tip gap lowers eta_s by 7.39e-3 (confirmed, "Not tested at cfd") and sweep does not respond; ledgers byte-identical for 1 and 8 workers. M15 (evidence parameters not compared) is recorded, not caught. Every confirmation is about the evaluator that produced it, never about a real fan. MCP exposure is F-702.

> Follow-up proposed: Hypothesis record, seeded re-run sample and evidence-parameter guard. Deps F-701. Acceptance: verify with a registered sample of k evidence pairs (derived seed, k recorded in the result) catches an impostor evaluator on at least the registered share of synthetic worlds; a check that edits one evidence parameter and rewrites the reference hashes is refused only by the parameter comparison (the mutation "parameters not compared" fails it); every ADR 0624 check keeps its result.

> Follow-up proposed: Coupled held metrics and the held-tolerance resolution floor for hypotheses. Deps F-701. Acceptance: two held metrics (flow and speed) solved together on a synthetic evaluator with a known joint solution within the registered tolerance, the floor from the held tolerance computed against a closed form, a registration below that floor refused with both values named, and every ADR 0624 check keeps its result.
