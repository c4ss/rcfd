# ADR 0670: F-749 registration: the held-tolerance floor over a sample of the design

Status: **REGISTERED 2026-11-16, written before any code of F-749 and before any run of its checks.** Written by the
implementing agent on branch `f749-work` (from 6600f7c9, which holds F-701 / ADR 0624, F-727 / ADR 0662 and F-728 / ADR 0665),
worktree `w49i-work/f749`, scratch `w49i-work/f749-scratch`. Not yet accepted by the maintainer. Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation) and is marked "(standing instruction)".

Roadmap item F-749 (deps F-728), the follow-up proposed by ADR 0665 (addendum 1, remaining question 2). The binding acceptance:

> Every limit not stated here is registered in the ADR before the run. The floor of register_with_floor taken as the largest over a registered seeded sample of design points (both arms solved at each), against a closed form on a synthetic evaluator whose sensitivity to the held metrics varies over the domain so that the midpoint floor is exceeded at some point by a registered factor; a registration accepted by the midpoint floor and below the sample floor refused with both values named; registrations made by the midpoint rule keep their bytes; every ADR 0624, ADR 0662 and ADR 0665 check keeps its result.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact); each registered run made once; existing tests not edited; defaults bit-identical; no golden, hash
table or `baseline_hashes.rs` edit; no new dependency, no unsafe code; clean-room. The library change is confined to
`crates/rcfd-api/src/design/hypothesis.rs`; the checks go in a new `crates/rcfd-api/tests/f749_floor_sample.rs`.
`f701_hypothesis.rs`, `f727_rerun_sample.rs`, `f728_coupled_held.rs` and their ignored records are not edited. ADR 0662's
record-format rule is kept: a new key is written only when used, so old records and registration entries keep their bytes and
hashes.

## What was read

`AGENT_RULES.md`; the brief; ADR 0665 (D1 to D3, the set-up WC and its closed forms, J0 to J9, addendum 1 and its remaining
question 2: "the largest floor over a registered sample of design points (two solves and the differences per point)"); ADR 0662
(D1 record-format rule, D2 the seeded Fisher-Yates draw `rerun_pairs` and `rerun_seed`); ADR 0624 (the design, `design_points`,
the derived seed); `design/hypothesis.rs` (`Hypothesis`, `to_json`, `from_json`, `validate`, `registration_entry`, `register`,
`register_batch`, `held_floor`, `arm_floor`, `HeldFloor`, `register_with_floor`, the floor rules in `check_member` and
`verify_slot`, `design_points`, `rerun_pairs`); `crates/rcfd-api/tests/f728_coupled_held.rs` (WC, closed forms, helpers).

## Disclosed: runs and arithmetic before this registration

1. **Smoke run on the base code (temporary test `zz_f749_smoke.rs` in the worktree before any library change, never committed,
   moved to `f749-scratch/`; log `smoke.log`).** It writes three ledgers by the midpoint rule of ADR 0665 (`register_with_floor`
   then `run`, workers 1) with the code of 6600f7c9 and prints their entry counts, byte lengths, the FNV-1a 64 of their bytes,
   the record hashes, the stored floors and the findings. Those are the literals of P0 below. No other check's code ran. The
   midpoint floor printed for set-up WQ is 3.5708017086659505e-6 (closed form below 3.5708017094e-6).
2. **Arithmetic (by hand, from the closed forms of ADR 0665 and set-up WQ below).** On the hold (q = 1, n = 50) the response and
   both sensitivities of WQ are those of WC multiplied by `G(c) = 1 + 3 c^2`, so the floor of a point is
   `F(c) = G(c) F_WC`, `F_WC = 2.040458119658e-6` (ADR 0665 J2). Midpoint `F(0.5) = 1.75 F_WC = 3.570801709e-6`; largest over
   the domain `F(1) = 4 F_WC = 8.161832479e-6`, a factor 4 / 1.75 = 2.2857 above the midpoint. As-low-arm targets
   (b at 0.02, a at 50): the low arm has `q / n = 0.02` at every c, so `phi_high(c) = G(c) 1.0316581197e-6`. The chance
   estimates of P2, P3 and P7 are in the estimates paragraph.

## Design (fixed now; names are the implementation's)

### D1. The sample (opt-in, in the record)

A new optional record field `floor_sample: Option<FloorSample { points }>`, written as `"floor_sample": {"points": k}` only when
given (ADR 0662 D1: a record without it keeps its text and hash). Registering it is a statement about the claim (where its held
tolerances were shown not to matter), so it belongs to the record and its hash, as the re-run sample does (standing
instruction; the alternative, a separate argument of `register_with_floor`, would leave a registration's floor rule outside the
hash that the evidence names).

* The sampled points are design points: `k` of the `n_pairs` pairs of the registered design (`design_points(h, derived_seed)`),
  the pairs drawn by `rerun_pairs(n_pairs, k, floor_seed)` (ADR 0662 D2's Fisher-Yates draw) with
  `floor_seed = FNV-1a 64 of "<record hash>:floor:<seed>"`. A sample of the design, not of the domain, because the floor bounds
  the error of each pair at its own point (ADR 0665 D2: the mean of the differences is in error by at most the largest per-pair
  bound), so the points that matter are those the evidence will be taken at; `k = n_pairs` gives the bound over the whole design.
* The domain midpoint stays in the sample, so the sample floor is never below the midpoint floor (a registration the sample rule
  accepts is one the midpoint rule accepts).
* Validation: `points` in `1..=n_pairs`; at least one hold; plain `register` and `register_batch` refuse a record with a floor
  sample ("floor_sample: `register` computes no held-tolerance floor; use `register_with_floor` (ADR 0670)").

### D2. The sample floor

At each point `x` of the sample (the midpoint first, then the sampled pairs in ascending order) the floor of ADR 0665 D2 is
computed exactly as at the midpoint: both arms solved by the solver of the evidence (the low arm at the `at` values for
as_low_arm targets), the sensitivities by differences at the solved point, `F(x) = phi_low + phi_high` (value targets) or
`phi_high` (as_low_arm). The sample floor is `F_s = max_x F(x)` (the first point reaching the maximum when two are equal). The
derivation is ADR 0665 D2 applied pair by pair; the maximum over the sampled pairs bounds the first-order error of their mean,
and the maximum over all pairs (`k = n_pairs`) bounds that of the registered mean. Not a bound over unsampled pairs.

A point whose arms cannot be solved within the brackets, or whose Jacobian is singular, refuses the registration, the message
naming the design pair (the evidence would fail there too).

### D3. API and storage

* `held_floor(h, model)`: with no `floor_sample` exactly ADR 0665's function (same arithmetic, same messages). With one, the
  value is `F_s`, `probe` and `arms` those of the largest point, and `sample: Some(SampledFloor { points, seed, pairs, midpoint,
  values, largest })`, `values` the floor of each sampled pair in `pairs` order, `largest` the pair index of the largest point
  (`None` when it is the midpoint).
* `held_floor_at(h, model, x)`: the floor of ADR 0665 D2 at one point `x` (domain order); `sample: None`.
* `floor_seed(hash, seed)`, `floor_sample_pairs(h)` (public, for the checks and callers).
* `register_with_floor`: unchanged when the record has no sample (the registration entry carries ADR 0665's `held_floor` with no
  new key). With a sample the stored `settings.held_floor` gains `"sample": {"points", "seed", "pairs", "midpoint", "values",
  "largest"}`; the refusal reads "min_effect: <m> is below 10 times the held-tolerance floor <F_s> of '<response>' over the
  domain midpoint and <k> sampled design pairs (largest at <pair p | the midpoint>; the midpoint floor is <F_mid>; ADR 0670)",
  naming `min_effect`, the sample floor and the midpoint floor.
* `verify` (without a model): a record with a floor sample and no stored floor, or a stored floor without `sample`, is a problem
  ("floor sample"); a stored `sample` on a record without one is a problem; the stored `pairs` differing from
  `floor_sample_pairs` of the record is a problem ("sampled pairs"); the stored `value` differing from the largest of
  `midpoint` and `values` is a problem ("not the largest of its sample"). With a re-run model the floor is recomputed as before
  (ADR 0665 D3), now with the sample; a value differing in its bits is "not reproduced".

## Set-ups (fixed now)

Every ledger uses the fixed clock `1_700_000_000_000`. WC and its helpers are ADR 0665's (copied into the new test file).

* **WQ.** WC with `G = 1 + 3 c^2` in place of `1 + gamma c` (`beta = 1`): speed `n = a B / (G (1 + 0.2 b))`, flow `q = b n A`,
  response `power = a b C`, `A = 1 + 0.5 s`, `B = 1 + 0.3 s`, `C = 1 + s`. Evaluator `synthetic.wq` version `1`,
  deterministic, tier analytic. Holds `q` (target `Value(1)`, tolerance 1e-6, by `b`, bracket [0.001, 0.1]) and `n`
  (`Value(50)`, 1e-5, by `a`, bracket **[10, 250]**: `a*` reaches 200.8 at c = 1, s = 0). Claim as WC, `Increases`, 32 pairs,
  resolution 0, domain `c` [0, 1].
* **Closed forms of WQ.** Joint solution and sensitivities: ADR 0665's with `G(c)`. Floor of a point with value targets:
  `F(c) = phi(s=0, c) + phi(s=1, c)`, `phi = |S_q| 1e-6 + |S_n| 1e-5` (the WC formulas of `f728_coupled_held.rs` with `G(c)`).
  As-low-arm targets (b at 0.02, a at 50): low arm `n_l = 50 / (G 1.004)`, `q_l = 0.02 n_l`; `F(c) = phi(s=1, c)` at
  `(q_l, n_l)`.

## Checks (limits fixed now)

All in `crates/rcfd-api/tests/f749_floor_sample.rs`, release profile. Clause numbers: (1) the floor the largest over a
registered seeded sample of design points against a closed form; (2) the evaluator's midpoint floor exceeded at some point by a
registered factor; (3) a registration accepted by the midpoint floor and below the sample floor refused with both values named;
(4) midpoint registrations keep their bytes; (5) every ADR 0624, 0662 and 0665 check keeps its result.

| Id | Clause | What | Limit |
|---|---|---|---|
| P0 | (4) | the three smoke ledgers written by the new code (`register_with_floor` with no sample, `run` workers 1) | entry count, byte length, FNV-1a 64 of the bytes and record hash equal to the smoke literals of 6600f7c9 (below); no `floor_sample` and no `sample` key in the file; `held_floor(..).sample` is `None` |
| P1 | (1) | WQ value targets, `floor_sample` 24 of 32, `min_effect` 0.01, seed 749_001, `register_with_floor`: (a) the stored pairs equal `rerun_pairs(32, 24, s)` with `s` the FNV-1a 64 of `"<hash>:floor:749001"` computed in the test; (b) each stored per-pair value against the closed form `F(c_i)` at the design point `c_i` of `design_points(h, derived_seed)`; (c) the stored midpoint value against `F(0.5)`; (d) the stored value against `max(F(0.5), max_i F(c_i))` and its probe the point of the largest closed form; (e) `held_floor` with the model equal to the stored value bit for bit; then `run` (workers 1), `verify` with the model | (b), (c), (d) relative <= 1e-6 each (24 of 24 in (b)); (a), (e) exact; (d) probe equal; confirmed, no problem, genuine |
| P2 | (2) | (a) the P1 sample floor over its midpoint floor; (b) `held_floor_at` at c = 1 against `4 F_WC`; (c) the closed-form ratio `F(1) / F(0.5)` | (a) >= 1.5 (the registered factor); (b) relative <= 1e-6; (c) >= 2 |
| P3 | (3) | WQ value targets, seed 749_003, `min_effect` 5e-5, `floor_sample` 24: `register_with_floor`; then the same record without the sample (the midpoint rule) in the same ledger | the first refused: the message contains "min_effect", "0.00005", "held-tolerance floor", "sampled design pairs", the library's sample floor of that record as printed (`{}`) and its midpoint floor as printed; the sample floor relative <= 1e-6 of the closed-form maximum; 10 times the midpoint floor below 5e-5 and 10 times the sample floor above it; the file unchanged by the refusal; the midpoint registration accepted at slot 1 |
| P4 | (1) | WQ as_low_arm targets (b at 0.02, a at 50, bracket of `a` [10, 250]), `floor_sample` 8, seed 749_004 | one arm (high) at every point; each of the 8 values and the midpoint relative <= 1e-6 of `phi_high(c) = G(c) 1.0316581197e-6`'s closed form (written from the formulas, not the literal) |
| P5 | (1) | negative control: WC (G = 1), `floor_sample` 32 (every pair), seed 749_005 | the sample floor over the midpoint floor minus 1 <= 1e-6 (no variation of the sensitivity, no inflation) |
| P6 | — | guards on a registration-only ledger of the P1 record: (a) `sample` removed from the stored floor; (b) the first stored pair index replaced by another index not in the list; (c) the stored value replaced by the stored midpoint value; (d) a `sample` object (copied from (a)'s original) added to the stored floor of a midpoint registration (the P3 accepted one) | (a) a problem containing "floor sample"; (b) a problem containing "sampled pairs"; (c) a problem containing "not the largest of its sample" without a model and one containing "not reproduced" with the model; (d) a problem containing "floor sample"; 4 of 4 |
| P7 | — | refusals: plain `register` with a floor sample; `register_batch` with one; `floor_sample` 0 and 33 through `register_with_floor`; WQ with the bracket of `a` [10, 150] and `floor_sample` 32 | 5 of 5 refused with the field named ("floor_sample" and "register_with_floor"; the same; "floor_sample"; "floor_sample"; "design pair"); the file unchanged; the [10, 150] record without the sample accepted by the midpoint rule |
| P8 | — | the P1 registration written into two fresh ledgers | byte-identical |
| C1 | (5) | `--test f701_hypothesis` (19 fast) and once `-- --ignored` | 19 pass; heavy S1 1, S2 1592 of 1595, NC1 400, NC2 61, S4 118 |
| C2 | (5) | `--test f727_rerun_sample` (8 fast) and once `-- --ignored` | pass; heavy A1 394, NC6 0, NC7 116 |
| C3 | (5) | `--test f728_coupled_held` (10 fast) | 10 pass with ADR 0665's printed numbers (J1 (d) error 8.67e-9, J2 relative 2.8e-11, J0 literals) |
| C4 | — | `cargo test -p rcfd-api --release --lib`; `cargo test -p rcfd-cli --release --test docs_site` (docs edited) | pass |
| R | — | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings` | clean |

Smoke literals of P0 (6600f7c9, `smoke.log`): WC value targets seed 749_900: 66 entries, 63350 bytes, FNV-1a 64
`07ff3427fa1da108`, record hash `fnv1a64:54561c0a1e6d09c4`; WQ value targets seed 749_901 (bracket of `a` [10, 250]): 66
entries, 63469 bytes, `0de2dc24545db9a1`, `fnv1a64:c9810d4e57d4ccd3`; WC as_low_arm (b at 0.02, a at 50) seed 749_902: 66
entries, 61909 bytes, `60277d546d014ba1`, `fnv1a64:166ddb3ab9838223`. All three confirmed with no problem. The smoke's evaluator
(`zz_f749_smoke.rs`, one struct for WC and WQ) is copied into the new test verbatim.

Estimates behind the limits. (b) to (d) of P1, P4, P5: ADR 0665 J2 measured the floor within 2.8e-11 of the closed form, and
the per-point arithmetic is the same, so 1e-6 is a wide margin; a floor computed at the midpoint for a sampled point is off by
`|G(c_i) / 1.75 - 1|`, up to 0.43. P2 (a): with 24 of 32 pairs of a Latin hypercube in `c`, the ratio is at least 1.5 when a
sampled pair has `c >= 0.736`; at least 8 strata lie above it, so all 8 are missed with probability at most
`1 / C(32, 24) = 9.5e-8`. P3: 10 F_mid = 3.571e-5 is below 5e-5; 10 F_s exceeds 5e-5 when a sampled pair has `c > 0.6954`; 9
strata lie above it and 24 of 32 cannot miss 9. P7: with every pair sampled, the pairs with `c > 0.816` (`a*` above 150 in the
low arm) are reached with certainty (4 or more strata). P5: on WC the per-point floors differ only through the Newton residuals,
of relative order 1e-8.

## Negative controls and mutations (registered now)

Negative controls: P5 (the sample adds nothing where the sensitivity does not vary); P3's midpoint registration (the midpoint
rule accepts what the sample rule refuses); P6 (a) to (d) (a stored floor that skips, edits or invents the sample).

Mutations (scratch copy of the code commit, one at a time; each must fail a registered fast test):

| Id | Mutation | Expected to fail |
|---|---|---|
| M35 | the sample ignored by `held_floor` (the midpoint floor returned with the sample's metadata) | P1, P3 |
| M36 | the smallest per-point floor taken in place of the largest | P1 |
| M37 | the pairs drawn with the design seed (`derived_seed`) in place of `floor_seed` | P1 (a) |
| M38 | the floor of a sampled pair computed at the domain midpoint | P1 (b), P4 |
| M39 | the design points taken from `design_points(h, h.seed)` (the caller seed, not the derived seed) | P1 (b) |
| M40 | `verify` does not require `sample` when the record has a floor sample | P6 (a) |
| M41 | `verify` does not compare the stored pairs | P6 (b) |
| M42 | `verify` does not check the stored value against the largest of its sample | P6 (c) |
| M43 | plain `register` accepts a floor sample | P7 |
| M44 | `"sample": null` written by `register_with_floor` with no sample | P0 |
| M45 | the refusal message leaves out the midpoint floor | P3 |

## Decision rule

* **done:** every check above met, P5 as stated, every mutation M35 to M45 caught.
* **partial:** P0, P1, P2, P3 and C1 to C3 met, and another check missed (an ignored `*_known_miss` record with the limit intact),
  or a mutation survives.
* **todo** (the lead does not take the code): P0, P1, P2, P3 or C1 to C3 missed.

## Limits (not covered)

* The sample floor bounds the first-order error at the sampled pairs only; with `k < n_pairs` an unsampled pair may need more.
  `k = n_pairs` covers the whole design (at `k` times the midpoint's cost in evaluations, about 36 per point for two holds).
* First order, as ADR 0665: tolerances large enough that the second-order term matters are not bounded.
* No sample over the domain away from the design points, no adaptive search for the largest floor.
* Library only; no MCP tool (F-702). Synthetic evaluators only; no fan, no CFD.

## References

* ADR 0665 D2 (the held-tolerance floor, first principles) and ADR 0662 D2 (the seeded Fisher-Yates draw: Fisher and Yates
  1938, Durstenfeld 1964, cited from memory there and here; the draw is the existing `rerun_pairs`).
