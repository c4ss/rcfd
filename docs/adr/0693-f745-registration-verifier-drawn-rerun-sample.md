# ADR 0693: F-745 registration: a verifier-drawn re-run sample for hypotheses from evaluators outside the maintainer's control

Status: **REGISTERED 2026-11-17, written before any code of F-745 and before any run of its checks.** Written by the
implementing agent on branch `f745-work` (from 3b9ea889, which holds F-701 / ADR 0624, F-727 / ADR 0662, F-728 / ADR 0665 and
F-749 / ADR 0670), worktree `w49i-work/f745`, scratch `w49i-work/f745-scratch`. Not yet accepted by the maintainer. Every choice
below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation) and is marked "(standing instruction)".

Roadmap item F-745 (deps F-727), the follow-up proposed by ADR 0662 (addendum 1, remaining question 1, option (b)). The binding
acceptance:

> Every limit not stated here is registered in the ADR before the run. The verifier draws the k pairs from a seed not stored in the ledger before the check and records it after; an impostor that is honest exactly on the pairs derivable from the file (the F-727 sample) is caught on at least the registered share of synthetic worlds, while the F-727 sample misses it in all of them (negative control); every ADR 0624 and ADR 0662 check keeps its result.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact); each registered run made once; existing tests not edited; defaults bit-identical; no golden, hash
table or `baseline_hashes.rs` edit; no new dependency, no unsafe code; clean-room. The library change is confined to
`crates/rcfd-api/src/design/hypothesis.rs` and adds functions and types only: no existing function changes its code path, so
`register`, `run`, `verify`, `lessons` and the record and entry bytes of ADR 0624, 0662, 0665 and 0670 are untouched. The checks go
in a new `crates/rcfd-api/tests/f745_drawn_sample.rs`. `f701_hypothesis.rs`, `f727_rerun_sample.rs`, `f728_coupled_held.rs`,
`f749_floor_sample.rs` and their ignored records are not edited. ADR 0662's record-format rule is kept: nothing new is written into
a registration, an evidence entry or a result entry; the verifier's record is a new entry of a new kind, appended only when the new
function is called.

## What was read

`AGENT_RULES.md`; the brief; ADR 0662 (D1 to D6, the set-up W5 and its impostor, A0 to A6, NC6, NC7, addendum 1 and its remaining
question 1: "a verifier-drawn sample from a seed not in the ledger, recorded after the check"); ADR 0624 (the record, verify,
lessons; addendum 1's numbers); ADR 0670 (the record-format rule restated); `design/hypothesis.rs` (`RerunSample`, `RerunPlan`,
`rerun_seed`, `rerun_pairs`, `rerun_miss_probability`, `rerun_sample_size`, `check_member`, `is_evidence_of`,
`result_entries_of`, `verify_slot`, `verify`); `design/ledger.rs` (`Entry`, `Entry::validate`, `Ledger::append`, `entry_ref`,
`resolve`, `rerun_check`); `rcfd_core::rng::Rng`; `crates/rcfd-api/tests/f727_rerun_sample.rs` (W5, the impostor, the world loop).

## Disclosed: arithmetic before this registration

Python with exact binomial coefficients (`f745-scratch/karith.py`, log `karith.log`); no Rust code of F-745 ran and no check ran.

* D1 set-up (n = 64, the impostor honest on the m = 13 pairs of the F-727 sample, dishonest on the other 51): `P_miss(k) =
  C(13, k) / C(64, k)`: k = 1 0.2031, k = 2 0.03869, **k = 3 0.006864 (143 / 20832)**, k = 4 0.001125. Over 400 worlds,
  P(missed > 20) is 0.100 at k = 2 and 1.6e-12 at k = 3. Expected caught at k = 3: 397.25.
* D2 set-up (dishonest on 16 pairs chosen outside the F-727 sample): `P_miss(13) = C(48, 13) / C(64, 13) = 0.014686`;
  P(missed > 20 of 400) 1.7e-4 at k = 12, 7.9e-7 at k = 13 (the numbers of ADR 0662 D3, since the draw does not depend on the file).
* k = 1 on D1: catch 51/64; Binomial(400, 0.797): mean 318.75, sd 8.05, +-4 sd [286.6, 350.9].
* Per-pair count of 400 honest k = 13 draws: Binomial(400, 13/64), mean 81.25, sd 8.05, +-4 sd [49.1, 113.4].

## Design (fixed now; names are the implementation's)

### D1. Why the F-727 sample does not stop a forger, and what does

The F-727 sample `rerun_pairs(n, k, rerun_seed(hash, seed))` is a function of the record hash and the seed, both in the file. An
evaluator that reads the registration before producing evidence can compute it and be honest exactly on those pairs; `verify`
then re-runs only honest entries and reports nothing. The cure is a sample the evaluator cannot compute when it writes its evidence:
**the verifier draws the seed after the evidence exists, from a source outside the file, uses it, and only then appends it.** The
evidence entries are already written and hash-referenced before the seed exists, so no evidence can depend on it (the order is the
guard, as in commit-then-challenge audit sampling; first principles, no citation needed).

### D2. The draw and its closed form

* Seed source (`DrawSeed`): `Given(u64)` supplied by the caller (tests inject it so runs are reproducible), or `Entropy`:
  `entropy_seed()`, a u64 from `std::collections::hash_map::RandomState` (whose keys the standard library draws from the operating
  system's random source once per thread and then increments) hashing the system time in nanoseconds, the process id and a process
  counter. Std only, no new crate; it is unpredictable to the evaluator but not a cryptographic generator (stated in the limits).
  The seed is drawn **after** the ledger is read and before the re-run.
* Pairs: `rerun_pairs(n, k, verifier_seed)`, ADR 0662's partial Fisher-Yates (uniform over k-subsets), with the verifier's seed
  used as is. Both arms of each drawn pair (those present) are re-run with `ledger::rerun_check(entry, model, rtol, atol)`.
* Closed form. With b of the n pairs dishonest (on at least one arm, by more than the tolerances) and the draw independent of
  which, the check misses every dishonest pair with probability

  `P_miss(k) = C(n - b, k) / C(n, k) = prod_{i<k} (n - b - i) / (n - i)`

  (`rerun_miss_probability(n, b, k)`). For the impostor of the acceptance, honest exactly on the m pairs of the F-727 sample and
  dishonest on the other `b = n - m`: `P_miss(k) = C(m, k) / C(n, k)` (zero for k > m), and the caught share is `1 - C(m, k) /
  C(n, k)`. The F-727 sample is those m pairs, so its own catch probability is 0 for this impostor, whatever k.
* Registered share (standing instruction, as in ADR 0662): **caught in at least 0.95 of 400 synthetic worlds (380 of 400)**, with
  k the smallest for which a correct implementation misses that limit with probability at most 1e-4 (disclosed arithmetic): D1
  (n = 64, m = 13, b = 51) **k = 3** (P_miss 0.006864; P(missed > 20) 1.6e-12); D2 (b = 16 outside the F-727 sample) **k = 13**
  (P_miss 0.014686; 7.9e-7).

### D3. API (additions only)

* `check_drawn(entries, id, model, sample: &RerunSample, seed: u64) -> Result<DrawnCheck, String>`: pure, on entries in memory.
  `DrawnCheck { id, registration, k, seed, seed_source, pairs, rtol, atol, rerun: Vec<usize> (evidence entries re-run),
  mismatched: Vec<usize>, problems: Vec<String>, reproduced: bool, entries_seen: usize, record: Option<usize> }`. A problem reads
  "entry {k}: evidence of {id} not reproduced by a verifier-drawn re-run of {model} {version}: ...".
* Refusals (`Err`, each message contains `drawn`): no registration of the id; the record unreadable; `k` outside `1..=n_pairs`;
  `rtol` or `atol` not finite or negative; a model whose id or version differs from the registration; tier measured; an evaluator
  declared non-deterministic with `rtol = atol = 0`; evidence incomplete (a drawn check is of finished evidence).
* `verify_drawn(ledger, id, model, sample, seed: DrawSeed) -> Result<DrawnCheck, String>`: (1) read the ledger; (2) draw the seed
  (`Given` or `entropy_seed()`); (3) `check_drawn`; (4) append **one** entry of kind `hypothesis_drawn_check` (`KIND_DRAWN`) and set
  `record`. Nothing is appended when (1) or (3) fails.
* The record: study `hypothesis`, variant the id, evaluator `rcfd.hypothesis` version 1, fidelity the registered tier, verdict ok;
  metrics `k`, `n_rerun`, `n_mismatched` [1]; settings `{"seed", "seed_source": "given"|"entropy", "k", "pairs", "rtol", "atol",
  "rerun_by": {"id", "version"}, "reproduced", "mismatched", "entries_seen", "record_hash"}`; parents the registration and every
  re-run evidence entry by `entry_ref` (index and hash, so a later edit of a checked entry breaks the reference); provenance
  `{"driver": "hypothesis_drawn", "hypothesis": id, "hash": record hash}`. Its provenance driver is not `hypothesis`, so
  `is_evidence_of` ignores it; its kind is none of `hypothesis`, `hypothesis_result`, `hypothesis_withdrawn`, so `registrations`,
  `result_entries_of` and the withdrawal rule ignore it. `verify` and `lessons` therefore give the same answer with or without
  drawn records (checked by E1).
* `drawn_record_problems(entries, index, model) -> Vec<String>`: re-checks a recorded draw: the entry is a drawn record; its parents
  resolve (`ledger::resolve`); `check_drawn(&entries[..entries_seen], id, model, recorded sample, recorded seed)` gives the recorded
  pairs, mismatched list and `reproduced`. Empty when consistent; each problem names the record's index.
* Opt-in: nothing calls these functions; `verify` keeps its code and bits. `lessons` does not read drawn records (a follow-up).

## Set-ups (fixed now)

Fixed clock `1_700_000_000_000`; ADR 0662's W5 and `noise` copied into the new test file (the F-727 file is not edited).

* **W5, tier cfd**, 64 pairs, claim "raising x0 raises y by at least 0.5", the record carrying ADR 0662's sample
  `RerunSample { pairs: 13, rtol: 0, atol: 0 }` (so the F-727 sample S727 exists and is the negative control).
* **File-aware impostor.** Same id, version, tier. After registration the test computes, from the registered record only, S727 =
  `rerun_pairs(64, 13, rerun_seed(hash, seed))` and the design points; the impostor returns `y + 0.3` on the high arm of each
  dishonest pair and the real `y` otherwise (pairs recognised by the bits of `x1`, as in ADR 0662). **D1:** dishonest = every pair
  not in S727 (51). **D2:** dishonest = the first 16 of the ascending complement of S727 shuffled by `Rng::new(world ^ 0x0745_0D02)`.
* **Worlds.** w = 0 to 399, world and record seed `745_000 + w`; verifier seed `s_w = fnv64("f745:verifier:<w>")` (not derivable
  from the ledger). Three ledgers per world: D1 impostor, D2 impostor, the real model (honest). rtol = atol = 0. Caught = the
  drawn check's `problems` not empty.

## Checks (limits fixed now)

All in `crates/rcfd-api/tests/f745_drawn_sample.rs`, release profile.

| Id | Clause | What | Limit |
|---|---|---|---|
| D0 | (2) | world 745_990, D1: (i) `verify(.., Some(real))` (the F-727 sample): 0 problems, `evidence_rerun = Some(true)`; (ii) `check_drawn` k = 3 with `s` = 1234567891011121314: problems = number of drawn pairs not in S727, each on a high-arm entry of a drawn pair, `reproduced` = (that number is 0), `rerun` = the present arms of the drawn pairs (6 entries); (iii) the honest ledger, same check: 0 problems, `reproduced` | exactly as stated |
| D1 | (2) | worlds, D1 impostor, `verify_drawn` k = 3 with `Given(s_w)`: worlds caught | **at least 380 of 400** |
| D2 | (2) | worlds, D2 impostor, `check_drawn` k = 13, `s_w`: worlds caught | **at least 380 of 400** |
| D3 | (2) | D1 and D2: number of problems equal to the size of (drawn ∩ dishonest), and every problem on a high arm | 800 of 800 |
| NC1 | (2) | worlds, D1 and D2 ledgers, the F-727 sample (`verify(.., Some(real))`): worlds with a re-run problem; `evidence_rerun = Some(true)` | **0 of 800**; 800 of 800 |
| NC2 | (2) | worlds, D1, `check_drawn` k = 13 with the file-derived seed `rerun_seed(hash, seed)` (a "verifier" seed that is in the file): caught; drawn pairs equal to S727 | 0 of 400; 400 of 400 |
| NC3 | (2) | worlds, D1, `check_drawn` k = 1 with `s_w` | caught in 287 to 350 of 400; below 380 (fails D1) |
| H1 | (2) | worlds, honest ledgers, `check_drawn` k = 3 and k = 13 with `s_w`: worlds with a problem | 0 of 800 |
| H2 | (2) | honest k = 13 draws: count of each pair over 400 worlds | every count in [50, 113] |
| E1 | (1) | world 745_991, D1, `verify_drawn` k = 3 `Given(s)`: the file before is a byte prefix of the file after; exactly one new entry, of kind `hypothesis_drawn_check`, its settings seed, k and pairs equal to the report and pairs = `rerun_pairs(64, 3, s)`; the decimal and the 16-digit hex text of `s` absent from the bytes before and the decimal present after; `verify(.., Some(real))` and `lessons` equal before and after the append (struct equality, row count); `drawn_record_problems` empty | all, exactly |
| E2 | (1) | the E1 file edited (separately): (a) the recorded seed + 1; (b) `reproduced` set to the opposite; (c) one re-run evidence entry's `y` + 1e-3 (the record's parent hash no longer resolves) | `drawn_record_problems` not empty in 3 of 3, each naming the record's index; unedited: empty |
| E3 | (1) | `entropy_seed()` twice; `verify_drawn` with `DrawSeed::Entropy` on the honest ledger of world 745_992 | the two seeds differ; the record's `seed_source` is `entropy`, its seed equals the report's, its pairs `rerun_pairs(64, 3, seed)`; `drawn_record_problems` empty |
| DT | (2) | world 745_993, impostor `y (1 + 1e-10)` on every arm, k = 4: rtol 1e-9; rtol 0 | 0 problems, `reproduced`; 8 problems, not reproduced |
| DR | — | refusals: k = 0, k = 65 of 64, rtol NaN, atol -1, model version 2, model id other, non-deterministic record with zero tolerances, incomplete evidence (registered, not run), unknown id; `verify_drawn` on each leaves the file unchanged | 9 of 9 refused naming `drawn`; the file unchanged |
| C1 | (3) | every ADR 0624 check: `cargo test -p rcfd-api --release --test f701_hypothesis` (19 fast) and once `-- --ignored` | 19 pass; heavy: S1 1, S2 1592 of 1595, NC1 400, NC2 61, S4 118 (ADR 0624 addendum 1) |
| C2 | (3) | every ADR 0662 check: `--test f727_rerun_sample` (8 fast, among them C2's byte literals of main 9f06e866) and once `-- --ignored` | 8 pass; heavy: A1 394 of 400, A2 0, NC6 0, NC7 116, A3 counts 60 to 107 (ADR 0662 addendum 1) |
| C3 | — | `--test f728_coupled_held`, `--test f749_floor_sample` (fast), `--lib` | pass |
| C4 | — | `cargo test -p rcfd-cli --release --test docs_site` (for the `docs/design-loop.md` bullet) | pass |
| R | — | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings` | clean |

D1, D2, D3, NC1 to NC3, H1 and H2 run in one heavy test over the 400 worlds, `#[ignore]`d, run once; a fast counterpart runs
worlds 0 to 15 with the assertions that hold world by world (D3, NC1, NC2, H1) and prints the counts of D1, D2 and NC3.

## Negative controls and mutations (registered now)

Negative controls: NC1 (the F-727 sample: 0 of 800 by construction, the clause's control), NC2 (a verifier seed derivable from the
file: the draw is S727 and catches nothing; the seed source is what matters, not the second function), NC3 (k = 1: 0.797, below
the share), H1 (the honest evaluator is never flagged).

Mutations (scratch copy, one at a time; each must fail a registered fast check):

| Id | Mutation | Expected to fail |
|---|---|---|
| M24 | the verifier seed ignored: pairs from `rerun_seed(hash, seed)` | D0, worlds fast |
| M25 | only the low arm of a drawn pair re-run | D0, DT |
| M26 | `verify_drawn` appends no record | E1 |
| M27 | the record stores `seed ^ 1` | E1 |
| M28 | the tolerances ignored (re-run at 0, 0) | DT |
| M29 | the `1..=n_pairs` refusal of k removed | DR |
| M30 | `entropy_seed` returns a constant | E3 |
| M31 | `drawn_record_problems` does not re-check (returns empty) | E2 |
| M32 | the drawn record written with provenance driver `hypothesis` and kind `evaluation` (seen by verify as evidence) | E1 |

## Decision rule

* **done:** every check met, NC1 to NC3 and H1 as stated, M24 to M32 all caught.
* **partial:** D0, D1, NC1, E1, C1 and C2 met, and another check missed (an ignored `*_known_miss` record with the limit intact),
  or a control not as stated, or a mutation survives.
* **todo** (the lead does not take the code): D0, D1, NC1, E1, C1 or C2 missed.

## Limits (not covered)

* The guard is the order: evidence entries exist and are hash-referenced before the seed is drawn. It does not stop an evaluator
  that also writes the ledger after the check (it could append a forged drawn record); a drawn record is evidence of a check made
  by whoever holds the trusted model, and `drawn_record_problems` lets anyone holding that model redo it. Signing the record is not
  built.
* `entropy_seed` is unpredictable to an evaluator that does not run in the verifier's process; it is not a cryptographic generator
  (std `RandomState`). A verifier that needs one supplies its own seed (`DrawSeed::Given`).
* A verifier that repeats draws until one passes can launder evidence; every draw made by `verify_drawn` is recorded, so repeats are
  visible, but `lessons` does not yet read drawn records (follow-up).
* The share is shown for the two registered impostors; one dishonest on fewer pairs is caught less often (closed form D2).
  Synthetic evaluator labelled cfd; no CFD run. Library only; no MCP tool (F-702).

## References

* Hypergeometric miss probability and the order argument: first principles, above.
* R. Durstenfeld, "Algorithm 235: Random permutation", Comm. ACM 7 (1964) 420 (cited from memory, as in ADR 0662; the draw is
  ADR 0662's `rerun_pairs`).
* ADR 0624 (F-701), ADR 0662 (F-727), ADR 0670 (the record-format rule), ADR 0632 (`entry_ref`, `resolve`).

## Addenda

## Addendum 1: results (2026-11-17)

Code: commit 5aaab34a (registration af8e5ca4 before it). `crates/rcfd-api/src/design/hypothesis.rs`: additions only (the module
doc bullet; `KIND_DRAWN`, `DrawSeed`, `entropy_seed`, `DrawnCheck`, `check_drawn`, the private `drawn_entry`, `verify_drawn`,
`drawn_record_problems`, inserted before the `run` section); no existing function edited. New
`crates/rcfd-api/tests/f745_drawn_sample.rs` (7 tests: 6 fast, the heavy 400-world record ignored). `docs/design-loop.md`: one
bullet in the hypothesis section. No shared solver file, golden, hash table, case key or MCP tool.

Runs (release throughout, `CARGO_TARGET_DIR` in the scratch folder): the registered fast run `--test f745_drawn_sample --
--test-threads=2` once (`f745-scratch/run1.log`, 6 passed); the heavy record once (`heavy.log`, 21.9 s); C1 (`c1_fast.log`,
`c1_heavy.log`, 309 s), C2 (`c2_fast.log`, `c2_heavy.log`, 39.7 s), C3 (`c3_f728.log`, `c3_f749.log`, `c3_lib.log`), C4
(`docs_site.log`), fmt and clippy (clean, no code change after the registered run). Mutations on a `git archive` copy of 5aaab34a
with its own target directory (`mutate.py`, `mutate_run.log`, `mut_M*.log`; the unmutated control M0 6 of 6).

| Check | Limit | Measured | Met |
|---|---|---|---|
| D0 | (i) 0 problems, `Some(true)`; (ii) problems = drawn pairs outside S727, on drawn high arms, 6 re-run entries; (iii) 0 | (i) 0, `Some(true)`, S727 = [1, 6, 9, 11, 21, 27, 42, 45, 46, 52, 58, 59, 62]; (ii) drawn [13, 28, 37], 3 outside, 3 problems on drawn high arms, not reproduced, re-run [27, 28, 57, 58, 75, 76]; (iii) 0, reproduced | yes |
| D1 | at least 380 of 400 | **399 of 400** (expected 397.25) | yes |
| D2 | at least 380 of 400 | **394 of 400** (expected 394.1) | yes |
| D3 | 800 of 800 | 800 of 800 | yes |
| NC1 | 0 of 800; `Some(true)` 800 | **0 of 800**; 800 | yes (fails D1 and D2 as it must) |
| NC2 | 0 of 400; draw = S727 400 | 0; 400 | yes |
| NC3 | 287 to 350 of 400 | 305 (expected 318.75, z -1.7) | yes (below 380) |
| H1 | 0 of 800 | 0 | yes |
| H2 | counts in [50, 113] | 65 to 98 | yes |
| E1 | prefix, one new `hypothesis_drawn_check` entry, seed / k / pairs equal, seed text absent before and present after, verify and lessons equal, no record problem | 95367 then 96463 bytes, prefix; 1 entry (index 130); seed 7267193595651574403, k 3, pairs [10, 42, 62] = `rerun_pairs(64, 3, s)`; decimal and hex absent before, decimal present after; `Verification` equal, lessons equal; no problem | yes |
| E2 | 3 of 3 flagged naming the record; unedited empty | (a) "recorded pairs are not the draw of the recorded seed" (and the mismatched list differs); (b) "recorded outcome differs from the re-run"; (c) "parent 21 does not resolve" (and the mismatched list differs); 3 of 3; the re-serialised unedited file byte-equal | yes |
| E3 | seeds differ; record `entropy`, seed equal, pairs the draw; no problem | 9063337041387391991 and 1092531133075791110; record seed 1832182879083502506 `entropy` = report; pairs [4, 21, 23]; no problem | yes |
| DT | rtol 1e-9: 0, reproduced; rtol 0: 8, not | as stated | yes |
| DR | 9 of 9 naming `drawn`; file unchanged | 9 of 9; unchanged | yes |
| C1 | 19 fast; S1 1, S2 1592 / 1595, NC1 400, NC2 61, S4 118 | 19 passed (K1 1.33e-11, G1 hash, V1 22 of 22, NC3 75, R1 2347886 and 56361 bytes); S1 1, S2 1592 / 1595, NC1 400, NC2 61, S5 0, S3 0.047570, S4 118 | yes |
| C2 | 8 fast; A1 394, A2 0, NC6 0, NC7 116, A3 60 to 107 | 8 passed (C2 literals: 34 entries, 26298 bytes, `ef3add9a434918c6`, the three hashes); A1 394, A1b 400, A2 0 of 400, A3 800 and 60..107, NC6 0 of 400 and None 400, NC7 116 | yes |
| C3 | pass | f728 10 passed, f749 8 passed, `--lib` 141 passed 4 ignored | yes |
| C4 | pass | docs_site 2 passed | yes |
| R | clean | `cargo fmt -p rcfd-api --check` clean; `cargo clippy -p rcfd-api --all-targets -- -D warnings` clean | yes |

Mutations (caught = at least one fast test fails):

| Id | Mutation | Expected | Failing tests | Caught |
|---|---|---|---|---|
| M24 | verifier seed ignored (pairs from `rerun_seed`) | D0, worlds fast | d0, d_worlds_fast, e1_e2, e3 | yes |
| M25 | only the low arm re-run | D0, DT | d0, d_worlds_fast, dt | yes |
| M26 | no record appended | E1 | d_worlds_fast, e1_e2, e3 | yes |
| M27 | record stores `seed ^ 1` | E1 | d_worlds_fast, e1_e2, e3 | yes |
| M28 | tolerances ignored | DT | dt | yes |
| M29 | k range refusal removed | DR | dr | yes |
| M30 | `entropy_seed` constant | E3 | e3 | yes |
| M31 | `drawn_record_problems` returns empty | E2 | e1_e2 | yes |
| M32 | record written as evidence (kind `evaluation`, driver `hypothesis`) | E1 | d_worlds_fast, e1_e2, e3 | yes |

Deviations: none in limits or set-ups. D0 also asserts that the drawn pairs equal `rerun_pairs(64, 3, s)` (the D2 recipe), written
into the test before the run so that M24 is caught by D0 itself. E2's edits are made on the parsed entries and written back with
`Entry::to_json`; the test asserts that rewriting the unedited entries gives the file byte for byte, so each edit is the only
change. D2 caught 394, the same count as ADR 0662's A1: both are Binomial(400, 0.0147) miss counts from different draws, and 394 is
the expected value (394.1). Not run: the whole workspace and whole integration suites (the daily gate), `baseline_hashes` (no
shared solver file), rcfd-mcp tests (not touched).

Decision by the registered rule: **done.** Every check met, NC1 to NC3 and H1 as stated, M24 to M32 all caught.

### Remaining questions (each with a recommendation)

1. **`lessons` and drawn records.** Options: (a) leave `lessons` blind to them (now); (b) drop a hypothesis whose latest drawn
   record (consistent by `drawn_record_problems` with the caller's model) did not reproduce, and report the number of drawn checks
   so repeated draws are visible. Recommendation: (b) as a follow-up; it changes no existing result since no ledger has drawn
   records yet.
2. **Who may write a drawn record.** A forger who can append to the ledger can also append a forged drawn record. Options: (a)
   keep (anyone with the trusted model redoes a record by `drawn_record_problems`); (b) sign records with a verifier key.
   Recommendation: (a) until the ledger is shared with outside writers; (b) needs a signature scheme the repository does not have
   (no new crate), so it stays open for the maintainer.
3. **Default k.** Options: (a) caller-chosen (now); (b) `rerun_sample_size(n, n - m, 0.02)` for the impostor honest on the m-pair
   F-727 sample, together with F-727's own k for one dishonest on a quarter of the pairs. Recommendation: (b) as the MCP tool
   default, decided with F-702.

## For the lead: roadmap entries

> Proposed status: done

> Follow-up of F-727 (ADR 0662 question 1). ADR 0693, 2026-11-17. DONE. Library only, opt-in: design::hypothesis::verify_drawn reads the ledger, then draws k pairs from a verifier's seed that is not in the file (DrawSeed::Given, or DrawSeed::Entropy from std's OS-seeded RandomState; not cryptographic), re-runs both arms of each with the trusted evaluator, and only then appends one hypothesis_drawn_check entry with the seed, the pairs, the mismatched entries and hash references to every re-run entry; check_drawn is the pure check, drawn_record_problems redoes a recorded one. Closed form: an impostor honest on m of n pairs is missed with probability C(m,k)/C(n,k); for the 13-pair F-727 sample of 64 pairs, k = 3 (miss 0.0069). Checks on 400 synthetic worlds: an evaluator that reads the registration and is honest exactly on the F-727 sample is caught in 399 (limit 380) with k = 3, one dishonest on 16 pairs outside that sample in 394 (limit 380) with k = 13, while the F-727 sample catches neither, 0 of 800 (negative control); a "verifier" seed derived from the file catches 0 of 400; k = 1 catches 305 (287 to 350); the honest evaluator is never flagged; the problem count equals the drawn dishonest pairs in 800 of 800. The seed is absent from the file before the check, the file before is a byte prefix of the file after, verify and lessons are unchanged by the record, and edits of the recorded seed, the outcome or a checked entry are flagged (3 of 3). Every ADR 0624 and ADR 0662 check keeps its numbers (S1 1, S2 1592 of 1595, NC1 400, NC2 61, S4 118; A1 394, NC6 0, NC7 116, the C2 byte literals). Nine mutations caught. Not covered: a forger who can also append to the ledger (records are not signed), lessons does not read drawn records, no MCP tool (F-702).

> Note for F-727 (append): ADR 0693 (F-745, 2026-11-17): a verifier-drawn sample from a seed outside the file (verify_drawn) catches an evaluator that is honest exactly on the F-727 sample, which that sample never catches.

> Follow-up proposed: Lessons read the verifier-drawn checks. Deps F-745. Acceptance: Every limit not stated here is registered in the ADR before the run. lessons drops a hypothesis whose latest drawn record, redone consistently by drawn_record_problems with the caller's model, did not reproduce, and reports the number of drawn checks per hypothesis; on synthetic ledgers an impostor caught by a drawn check gives 0 rows and an honest one keeps its row; ledgers without drawn records give the lessons of ADR 0624 bit for bit; every ADR 0624, ADR 0662 and ADR 0693 check keeps its result.
