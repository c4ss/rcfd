# ADR 0662: F-727 registration: a seeded re-run sample of the hypothesis evidence and the evidence-parameter guard

Status: **REGISTERED 2026-11-16, written before any code of F-727 and before any run of its checks.** Written by the
implementing agent on branch `f727-work` (from main 9f06e866, which holds F-701 / ADR 0624), worktree `w49i-work/f727`, scratch
`w49i-work/f727-scratch`. Not yet accepted by the maintainer. Every choice below that the maintainer would normally be asked
about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and is marked "(standing
instruction)".

Roadmap item F-727 (deps F-701), the follow-up proposed by ADR 0624 (addendum 1, remaining questions 3 and 5). The binding
acceptance:

> verify with a registered sample of k evidence pairs (derived seed, k recorded in the result) catches an impostor evaluator on at least the registered share of synthetic worlds; a check that edits one evidence parameter and rewrites the reference hashes is refused only by the parameter comparison (the mutation "parameters not compared" fails it); every ADR 0624 check keeps its result.

Every limit not stated there is registered here. House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed
after a run (a miss becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; existing tests
not edited; defaults bit-identical; no golden, hash table or `baseline_hashes.rs` edit; no new dependency, no unsafe code;
clean-room. F-716 edits `design/bayes.rs` and `design/fan_objective.rs` on another branch; this item edits neither. The library
change is confined to `crates/rcfd-api/src/design/hypothesis.rs`; the checks go in a new
`crates/rcfd-api/tests/f727_rerun_sample.rs`. `f701_hypothesis.rs` and its ignored records are not edited.

## What was read

`AGENT_RULES.md`; the brief; ADR 0624 (D1 to D9, amendment 1 with its design, set-ups, checks and mutations, addendum 1 with
the results, remaining questions 3 and 5 and the two proposed follow-ups; the R2 miss: the f699 P4 test depends on the process id
through a temporary path, ADR 0622 question 3); `design/hypothesis.rs` (record JSON, `registration_entry`, `check_member` with the
parameter comparison, `verify_slot` with the all-or-none re-run, `result_entry`, `run`, `lessons`); `design/ledger.rs`
(`entry_hash`, `entry_ref`, `resolve`, `rerun_check(e, model, rtol, atol)`, `Fidelity`); `rcfd_core::rng::Rng` (`below`,
`shuffle`); `crates/rcfd-api/tests/f701_hypothesis.rs` (helpers, G1, G2, G5).

## Disclosed: runs and arithmetic before this registration

1. **Arithmetic of k (Python, `f727-scratch/karith.py`, exact binomial coefficients).** With n = 64 pairs and b corrupted pairs,
   the probability that a uniform sample of k distinct pairs misses every corrupted one is `C(n - b, k) / C(n, k)` (below).
   b = 16: k = 10 gives 0.04318, 11 gives 0.03039, 12 gives 0.02121, 13 gives 0.01469, 14 gives 0.01008. Over 400 independent
   worlds the number of missed worlds is Binomial(400, P_miss); P(more than 20 missed) is 0.209 (k = 10), 0.0119 (11), 1.7e-4
   (12), 7.9e-7 (13). b = 6: the smallest k with P_miss <= 0.02 is 30 (0.01794); b = 16 and miss 0.05: 10. With k = 1 and b = 16
   the catch probability is 16/64 = 0.25: 100 of 400 expected, sd 8.66.
2. **Smoke run on main's code (temporary test `zz_f727_smoke.rs` in the worktree before any library change, never committed,
   moved to `f727-scratch/`; log `smoke.log`).** It writes the no-sample ledger of check C2 (set-up W5, tier reduced_order, 16
   pairs, seed 727_900, fixed clock) with the code of 9f06e866 and prints its byte length, the FNV-1a 64 of its bytes and the
   entry hashes; those numbers are the literals of C2 below. No other check's code ran.

## Design (fixed now; names are the implementation's)

### D1. The registered sample in the record

`Hypothesis` gets `rerun_sample: Option<RerunSample>`, `RerunSample { pairs: usize, rtol: f64, atol: f64 }` (`Hypothesis::new`
gives `None`). `to_json` writes the key `"rerun_sample": {"pairs", "rtol", "atol"}` **only when it is `Some`**; a record without a
sample keeps its canonical text, its hash, its derived seed, its design and every entry byte of ADR 0624. `from_json` reads an
absent (or null) key as `None`, so every ledger written by ADR 0624's code is read as before (old records keep being read). A
reader with ADR 0624's code ignores the key of a new record (it does not know it) and re-runs all or none as before; its hash
checks still hold because the hash is of the stored text. This is stated in `docs/design-loop.md`.

Validation (refused, located, nothing appended): `pairs` in `1..=n_pairs`; `rtol`, `atol` finite and `>= 0`; tier `measured`
refused (a measurement cannot be re-run); an evaluator declared non-deterministic with `rtol = atol = 0` refused (a repeat need not
give the same bits). Each message names `rerun_sample`.

### D2. The sample: derived seed and the draw

* Seed: `rerun_seed(hash, seed) = fnv64("<record hash>:rerun:<seed>")`, a different string from the design seed
  `fnv64("<record hash>:<seed>")` of ADR 0624, so the sample is fixed at registration, reproducible by anyone holding the record,
  and not the design's stream.
* Draw: `rerun_pairs(n, k, seed)`: `idx = [0, 1, ..., n - 1]`, `r = Rng::new(seed)`; for `i` in `0..k`: `j = i + r.below(n - i)`,
  swap `idx[i]`, `idx[j]`; the sample is `idx[0..k]` sorted ascending. This is the first k steps of the Fisher-Yates shuffle
  (Durstenfeld 1964; Knuth, TAOCP vol. 2, algorithm 3.4.2 P, cited from memory): every k-subset is equally likely when `below` is
  uniform (its modulo bias is below `n / 2^64`).

### D3. Sample size: the hypergeometric bound (first principles)

If an impostor's evidence differs from the trusted evaluator on b of the n pairs, a re-run of a uniform k-subset finds none of
them with probability

`P_miss(k) = C(n - b, k) / C(n, k) = prod_{i=0}^{k-1} (n - b - i) / (n - i) <= (1 - b/n)^k`

(each factor is at most `(n - b)/n`; so the binomial bound `k >= ln(miss) / ln(1 - b/n)` is sufficient, the exact product gives a
smaller k). `rerun_sample_size(n, b, miss)` returns the smallest k with `P_miss(k) <= miss` by the product (None when `b = 0`,
`b > n`, `n = 0` or `miss` outside `(0, 1)`), and `rerun_miss_probability(n, b, k)` the product.

The registered share (standing instruction): **an impostor that differs on at least a quarter of the pairs (b = 16 of n = 64) is
caught in at least 0.95 of the synthetic worlds, i.e. in at least 380 of 400.** k is the smallest sample for which a correct
implementation misses the limit with probability at most 1e-4: misses ~ Binomial(400, P_miss(k)), P(misses > 20) is 1.7e-4 at
k = 12 and 7.9e-7 at k = 13 (disclosed item 1), so **k = 13** (P_miss 0.01469, the per-world catch probability 0.98531). The same
k is the smallest with P_miss <= 0.02, which is the miss given to `rerun_sample_size` in the checks.

### D4. Verify with a sample

When the record has a sample, `Verification` gets `rerun_sample: Some(RerunPlan { k, seed, pairs, rtol, atol })` whether or not a
model is given (so the caller knows what to re-run). With a model of the registered id and version and the registered tier any of
analytic, reduced_order or cfd, the evidence entries of both arms of each sampled pair (those present) are re-evaluated with
`ledger::rerun_check(entry, model, rtol, atol)`; a mismatch is a located problem as in ADR 0624 ("entry k: evidence of H-i not
reproduced by a re-run ..."), and `evidence_rerun = Some(all reproduced)`. The unsampled pairs are not re-run, whatever the tier.
Without a sample nothing changes: ADR 0624's rule (all evidence at analytic and reduced_order, none at cfd). `lessons` goes through
`verify` and so drops a hypothesis whose sample does not reproduce.

### D5. k recorded in the result

The `hypothesis_result` entry of a record with a sample gets `settings.rerun_sample = {"k", "seed", "pairs", "rtol", "atol"}`
(absent without a sample, so ADR 0624's result entries keep their bytes). It is part of the recomputation, so a forged or edited
sample in a result line makes it non-genuine like any other edit.

### D6. The evidence-parameter guard (ADR 0624 question 5)

No library change: the comparison of every evidence parameter with the registered design (factor level, domain point bit for bit;
the compensating parameter inside its bracket and equal to `at` for an `AsLowArm` low arm) exists in `check_member`. ADR 0624's
M15 survived because an edited parameter also changes the entry's hash, so the result line's parent reference stops matching and
the result is flagged by the recomputation. Check B1 edits one parameter **and** rewrites the result's parent hash, so the
reference guard is passed (every parent resolves) and only the parameter comparison is left to refuse it.

## Set-ups (fixed now)

Every ledger uses the fixed clock `1_700_000_000_000`. `noise(world, metric, params)` is ADR 0624's deterministic normal of a
point (copied into the new test file).

* **W5.** Parameters `x0` [1] (factor, 0.25 to 0.75) and `x1` [1] (domain [0, 1]); metric `y = x0 + noise(world, 0, params)`
  [1]; evaluator `synthetic.w5` version `1`, `deterministic: true`, tier `cfd` (A checks) or `reduced_order` (B1, C2); claim
  "raising x0 raises y by at least 0.5", `Increases`, `min_effect` 0.5, resolution 0, 64 pairs (16 in B1 and C2).
* **Impostor.** Same id, version and tier. It returns `y + 0.3` for the **high arm** of the corrupted pairs and the real `y`
  otherwise; the corrupted pairs are recognised by the bits of the domain point `x1` of the registered design (the test computes
  `design_points` after the registration). Corrupted set: the first 16 of `0..64` shuffled by `Rng::new(world ^ 0xC0FF_EE)`.
* **A worlds.** w = 0 to 399, world seed `727_000 + w`, record seed `727_000 + w`. Four fresh ledgers per world: impostor with
  k = 13 (A1), the real model with k = 13 (A2, A3), impostor without a sample (NC6), impostor with k = 1 (NC7). `rtol = atol = 0`.
  `verify(entries, "H-1", Some(&real))`. "Caught" means at least one problem containing "not reproduced by a re-run".
* **W3b (B1 (c)).** Parameters `s` [1] factor 0 to 1, `c` [1] domain [0, 1] (ignored), `n` [1/s] compensating; `q = n (1 + 0.5 s)`
  [m3/s], `y = n^2 (1 + 0.5 s)^2 (1 - 0.1 s)` [Pa]; held `q` `AsLowArm { at: 0.8 }` tolerance 1e-13 by `n` bracket [0.1, 10];
  `Decreases`, `min_effect` 0.01; 16 pairs; seed 727_503; evaluator `synthetic.w3b` 1, deterministic, tier reduced_order.

## Checks (limits fixed now)

All in `crates/rcfd-api/tests/f727_rerun_sample.rs`, release profile.

| Id | Clause | What | Limit |
|---|---|---|---|
| K0 | (1) | `rerun_sample_size(64, 16, 0.02)`, `(64, 6, 0.02)`, `(64, 16, 0.05)`; `rerun_miss_probability(64, 16, 13)` against `C(48, 13) / C(64, 13)` from factorial-free products written in the test; refusals `b = 0`, `miss = 1` | 13, 30, 10 exactly; relative <= 1e-12; `None` |
| K1 | (1) | `rerun_pairs(n, k, seed)` against the draw of D2 written in the test, seeds 0 to 99, (n, k) in (64, 1), (64, 13), (64, 64), (16, 5); k distinct, sorted, in `0..n`; (64, 64) is `0..64` | equal for 400 of 400 |
| A0 | (1) | W5 tier cfd, world 727_990, k = 13: (i) impostor corrupting the high arm of **every** pair: exactly 13 re-run problems, each on a high-arm entry of a sampled pair; `evidence_rerun = Some(false)`; `lessons` with the real model 0 rows; (ii) the real model: 0 problems, `Some(true)`, the result genuine, `lessons` 1 row; (iii) no model: 0 problems, `evidence_rerun = None`, `rerun_sample` reported; (iv) the result entry's `settings.rerun_sample` has `k = 13`, the seed `rerun_seed(hash, seed)` and the pairs of `Verification::rerun_sample` | as stated, exactly |
| A1 | (1) | A worlds, impostor, k = 13: worlds caught | **at least 380 of 400** (the registered share 0.95) |
| A1b | (1) | A worlds: number of re-run problems equal to the size of (sample ∩ corrupted) | 400 of 400 |
| A2 | (1) | A worlds, the real model, k = 13: worlds with a problem; `evidence_rerun = Some(true)` | 0 of 400; 400 of 400 |
| A3 | (1) | A worlds: the result entries (A1 and A2 ledgers) record k = 13 and pairs equal to `rerun_pairs(64, 13, rerun_seed(..))`; the count of each pair over the 400 real-model samples | 800 of 800; every count in [50, 113] (Binomial(400, 13/64): mean 81.25, sd 8.05, +- 4 sd) |
| NC6 | (1) | A worlds, impostor, no sample, tier cfd | caught in 0 of 400, `evidence_rerun = None` in 400 (the sample is what catches it; the control must fail A1) |
| NC7 | (1) | A worlds, impostor, k = 1 | caught in 66 to 134 of 400 (Binomial(400, 0.25) +- 4 sd); below 380 (fails A1) |
| A4 | (1) | W5 tier cfd, world 727_991, impostor `y (1 + 1e-10)` on every arm, k = 4: `rtol = 1e-9` and `rtol = 0` | 0 problems and `Some(true)`; 8 problems and `Some(false)` |
| A5 | (1) | refusals: `pairs = 0`, `pairs = 65` of 64, `rtol = NaN`, `atol = -1`, tier measured, `deterministic = false` with zero tolerances | 6 of 6 refused, the message names `rerun_sample`, the file unchanged |
| A6 | (1) | A0 (ii) with a model of version `2` | 0 problems, `evidence_rerun = None`, `rerun_sample` reported |
| B1 | (2) | W5 reduced_order 16 pairs (seed 727_501) and W3b, each run complete, then one parameter edited in the file and the result line's parent hash of the edited entry rewritten to the edited entry's `entry_hash`: (a) `x1` of pair 5 low arm plus 1e-3; (b) `x0` of pair 9 high arm 0.75 to 0.7501; (c) W3b `n` of pair 3 low arm 0.8 to 0.81 | every parent reference of the result resolves (`ledger::resolve`); `verify` gives **exactly one problem**, naming the entry index and the parameter ((a), (b): "the design has"; (c): "outside the registered rule"); `run` refused with that message; `lessons` 0 rows; 3 of 3 |
| NCB | (2) | B1 (a) without rewriting the hash | the result line flagged (the reference guard) and the parameter problem present |
| C1 | (3) | every ADR 0624 check: `cargo test -p rcfd-api --release --test f701_hypothesis` (19 fast) and once `-- --ignored` (the two heavy records) on the new code | 19 pass; the heavy records pass with the numbers of ADR 0624 addendum 1 exactly: S1 1, S2 1592 of 1595, NC1 400, NC2 61, S4 118 |
| C2 | (3) | the C2 ledger (W5 reduced_order, 16 pairs, seed 727_900, no sample) written by the new code; ADR 0624's G1 record rebuilt; a no-sample record's `to_json` | byte length and FNV-1a 64 of the file equal to the smoke literals of main 9f06e866 (below); G1 hash `fnv1a64:4b58385209778c10` (ADR 0624 addendum 1); no key `rerun_sample`; `verify` of the file: no problem, result genuine |
| C3 | — | `cargo test -p rcfd-api --release --lib` | pass |
| R3 | — | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings` | clean |

Smoke literals of C2 (main 9f06e866): 34 entries, 26298 bytes, FNV-1a 64 of the bytes `ef3add9a434918c6`, record hash `fnv1a64:e2e79db55c67eb38`, registration entry hash `fnv1a64:9dd45ad925cd2ce8`, result entry hash `fnv1a64:46c82e4c69b03471`.

A1, A1b, A2, A3, NC6 and NC7 run in one heavy test over the 400 worlds, `#[ignore]`d, run once; a fast counterpart runs worlds 0
to 15 with the assertions that hold world by world (A1b, A2, A3's records, NC6) and prints the counts of A1 and NC7.

## Negative controls and mutations (registered now)

Negative controls: NC6 (no sample at tier cfd: the ADR 0624 rule re-runs nothing, the impostor passes), NC7 (k = 1: the catch
rate is the hypergeometric 0.25, far below the share), NCB (without the hash rewrite the reference guard catches the edit; that
is why ADR 0624's M15 survived).

Mutations (scratch copy, one at a time; each must fail a registered fast check):

| Id | Mutation | Expected to fail |
|---|---|---|
| M15 | ADR 0624's: evidence parameters not compared with the design (the comparison loop of `check_member` removed) | B1 (a), (b) |
| M16 | the compensating-parameter rule removed (bracket and `at`) | B1 (c) |
| M17 | the sample ignored in `verify` (ADR 0624's all-or-none rule even with a sample) | A0 |
| M18 | the sample is the first k pairs | K1 |
| M19 | only the low arm of a sampled pair re-run | A0 |
| M20 | `rerun_sample` not written in the result entry | A0 |
| M21 | the key `rerun_sample` written as null when there is no sample | C2 |
| M22 | the registered tolerances ignored (re-run with 0, 0) | A4 |
| M23 | the refusal of tier measured removed | A5 |

## Decision rule

* **done:** every check above met, NC6, NC7 and NCB failing as stated, every mutation M15 to M23 caught.
* **partial:** A0, A1, A2, B1, C1 and C2 met, and another check missed (an ignored `*_known_miss` record with the limit intact), or
  a control does not fail as stated, or a mutation survives.
* **todo** (the lead does not take the code): A0, A1, A2, B1, C1 or C2 missed.

## Limits (not covered)

* The sample is derived from the record hash and the seed, both in the file: an impostor that knows the protocol can compute the
  sample and be honest only on it. The sample defends against an evaluator that is wrong by mistake or by version drift, not
  against a deliberate adversary (as FNV-1a 64 in ADR 0624 risk 8). A sample secret until verification time (drawn by the
  verifier) is not built.
* The share is shown for an impostor that differs on a quarter of the pairs; one that differs on fewer pairs is caught less often
  (`rerun_miss_probability` gives the rate; b = 6 needs k = 30 for a miss of 0.02).
* The re-run compares with `rerun_check` at the registered tolerances; a stochastic evaluator needs tolerances chosen by its owner.
  No CFD evaluator is run: the cfd tier here is a synthetic model labelled cfd.
* Library only; no MCP tool (F-702). The coupled held metrics of the other ADR 0624 follow-up are not part of this item.

## References

* Hypergeometric distribution: first principles (counting k-subsets), above.
* R. Durstenfeld, "Algorithm 235: Random permutation", Comm. ACM 7 (1964) 420; D. E. Knuth, The Art of Computer Programming,
  vol. 2, section 3.4.2 (both cited from memory; the draw is derived above).
* ADR 0624 (F-701), ADR 0632 (F-700, `entry_hash`, `resolve`), ADR 0121 (the ledger).

## Addenda
