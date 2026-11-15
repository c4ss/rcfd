# ADR 0566: F-624 registration: cached dissection order and symbolic analysis of the Newton driver's supernodal factor

Status: **REGISTERED 2026-11-15, written before any code of F-624 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text one diagnostic probe was run (disclosed below with every number used here): the base b29c1383
copied by `git archive` into `w49i-work/f624-scratch/probe`, with `eprintln!` timers added to `SupernodalLdlt::factor_with`
and `supernodal_factor` of that copy and one probe test appended to that copy's `tests/sa_newton_supernodal_f618.rs`; log
`f624-scratch/probe1.log`. Nothing was written in the worktree. Results go in the addenda at the end; nothing above the
addenda is changed after a run.

Roadmap item F-624 (dep F-618), the follow-up proposed by
[ADR 0542](0542-f618-registration-supernodal-factor-in-the-newton-driver-for-the-total-lagrangian-contact-body.md) (its
remaining question 1, after its T1 missed: 0.355 against `< 1/3`). Number 0566 given by the lead. Branch `f624-work` from
b29c1383, worktree `w49i-work/f624`, scratch `w49i-work/f624-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice the maintainer would normally be asked
about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and marked so.

## What F-624 asks

The acceptance, verbatim, binding:

> Every limit not stated here is registered in the ADR before the run. The supernodal factor of NewtonOptions reuses its nested dissection order and symbolic analysis while the tangent's pattern is unchanged, with the same bits as without the reuse: the total Lagrangian contact solve of the ADR 0517 hub at 16 cells (d = 1e-4 a) in under a third of the present time, measured with no other build on the machine, and every result of ADR 0542 bit-identical.

Clauses: **a0** every limit registered here; **a1** the supernodal factor of `NewtonOptions` reuses its nested dissection
order and symbolic analysis while the tangent's pattern is unchanged; **a2** with the same bits as without the reuse; **a3** the
total Lagrangian contact solve of ADR 0517's hub at 16 cells, `d = 1e-4 a`, in under a third of the present time; **a4**
measured with no other build on the machine; **a5** every result of ADR 0542 bit-identical.

Readings, registered. "The present time" is that of the present driver, the default envelope factor (as in ADR 0542 c3, whose
clause F-624 repeats); it is measured again in the same process, alternating, not taken from ADR 0542. "Without the reuse" is
the code of b29c1383: `SupernodalLdlt::factor` computing the order and the symbolic phase at every factorisation; its bits are
recorded before any library code (check B) and, for the hub, by ADR 0542's own digests (V0, V16).

## What was read (read only)

ADR 0542 (whole: D1 to D5, the time protocol, the results, the diagnosis of T1, M3), ADR 0503 (D1 the dissection, D2 the
supernodes and their row structures, D3 the multifrontal factor, the pivot test and the determinism), ADR 0447 D2 (the
elimination tree and the column counts by row subtrees), ADR 0517 D3 and D4 (the hub and its readings, by way of ADR 0542's
copy). Code: `crates/rcfd-linalg/src/sparse_chol.rs` (`adjacency`, `dissect`, `nested_dissection_order`, `permuted_lower_parts`,
`symbolic_rows`, `columns_of`, `supernodal_factor`, `partial_factor`, `SupernodalLdlt::factor_with`), `skyline.rs`
(`validate`, `check_permutation`), `matrix.rs` (`CsrMatrix::from_triplets`: sorted, duplicates summed, zeros kept),
`crates/rcfd-fem/src/nonlinear.rs` (`Problem`, `attempt_with`, `factor_solve`, `add_triplets`, `solve_with`, `solve_from`,
`solve_from_at`, `solve_from_field`, `controlled_attempt`), `constraint.rs` (`reduce`: the reduced pattern is the stored pattern
of the free rows and columns, no value test), `contact.rs` (the total Lagrangian body's loop over steps and augmentations calling
`solve_from`), tests `sa_newton_supernodal_f618.rs`, `sa_newton_f618_default_bits.rs`, `rcfd-linalg/tests/sa_supernodal.rs`.
Nothing outside the repository. No algorithm is new: the separation of a sparse factorisation into an ordering and symbolic
phase that depends on the pattern only, done once, and a numeric phase done per matrix of that pattern, is the standard
organisation of sparse direct solvers (George and Liu, *Computer Solution of Large Sparse Positive Definite Systems*,
Prentice-Hall 1981, ch. 5; cited from memory); here it follows from the code itself (finding 1).

Findings that shape the design:

1. **The order and the symbolic phase read the pattern only.** `nested_dissection_order` works on `adjacency(a)`, built from
   `row_ptr` and `col`; `permuted_lower_parts` places each stored entry by `row_ptr`, `col` and the order (it copies values, no
   arithmetic); `symbolic_rows` (tree, column counts, row counts `w_k`), `columns_of` (the column structure, a copy of values),
   the fundamental supernodes, their children and the rows of each front read indices only. The arithmetic (assembly by `+=` in
   a fixed order, the partial factorisations, the extend-add) reads the values. So for two matrices with the same `n`,
   `row_ptr` and `col`, every index array is equal, and a numeric phase that receives the same values at the same places
   performs the same operations in the same order: the same bits (a2). ADR 0542 Q4 already saw the pivots bit-equal with the
   order given.
2. **The pattern of the Newton tangent changes when the contact set adds entries.** `add_triplets` keeps the pattern when every
   triplet is on it and rebuilds on the union otherwise; `reduce` keeps the stored pattern. The pattern is therefore a function of
   the mesh and of the contact entries present, not of the values.
3. **Errors.** `SupernodalLdlt::factor` validates (square, finite, symmetric) before ordering; the nested dissection has no error.
   A factor that reuses an analysis must validate the new values first in the same way, so that every refusal keeps its text.

## The probe (disclosed; every number used)

Release, this machine (16 logical processors), other agents' builds running. The copy's probe test solved ADR 0542's total
Lagrangian hub with `ContactOptions::factor(SupernodalNestedDissection)` at `n = 4` and `n = 16`, `d = 1e-4 a`; per
factorisation it printed the pattern (`n`, entries, an FNV hash of `row_ptr` and `col`) and the times of: `validate`, the order,
the permutation, the symbolic phase (tree, counts, columns, supernodes, rows), the numeric phase.

| `n` | factorisations | patterns | validate | order | permute | symbolic | numeric | solve |
|---|---|---|---|---|---|---|---|---|
| 4 | 7 | 2: the first (551 890 entries), then six of one (552 806) | 0.008-0.011 s | 0.036-0.044 s | 0.014-0.022 s | 0.009-0.026 s | 0.20-0.57 s | 3.59 s |
| 16 | 6 | 2: the first (2 458 668 entries), then five of one (2 460 544), across both Newton solves | 0.046-0.060 s | 0.214-0.296 s | 0.062-0.074 s | 0.053-0.058 s | 0.95-1.87 s | 12.06 s |

The digests of ADR 0542's `flat` were `0xecd8f40c5721a14f` (`n = 4`; ADR 0542 C1) and `0xfc936c7553ff35d5` (`n = 16`; ADR 0542
D and R2): the instrumented copy reproduced ADR 0542's bits.

Arithmetic (disclosed). At `n = 16` the pattern-only work is order + permute + symbolic `= 0.33` to `0.43` s per factorisation,
`2.2` s over six. With one cached analysis and the pattern sequence seen (A, B, B, B | B, B), two analyses are made and four
reused: saved `4 x 0.36 = 1.4` s, less `6 x ~0.02` s for placing the values through a stored map: about `1.3` s of the probe's
12.06 s, giving about `10.7` s under that load. The numeric phase is `7.4` s of the 12.06 s (its spread, 0.95 to 1.87 s per
factorisation, is the load of the machine, ADR 0542's diagnosis). On a quiet machine (numeric about `6 x 1.0` s, the rest
`2.1` s as measured, validate and map `0.4` s, two analyses `0.7` s): about `9.2` s. ADR 0542's present times: 35.9 to 38.1 s
(Q1: 34.3 s). Estimated ratio: `9.2 / 35` to `10.7 / 36`, **about 0.26 to 0.30**.

## Design

### D1. The analysis (`crates/rcfd-linalg/src/sparse_chol.rs`)

`pub struct SupernodalAnalysis` (Debug, Clone, PartialEq): the pattern it was made for (`n`, `row_ptr`, `col`), the nested
dissection order, and the symbolic phase of ADR 0503 D2 of `P A P^T`: the row counts `w`, the column structure of the strict
lower triangle (pointers and rows), the fundamental supernodes (`sn_ptr`, `col_sn`), the children of each supernode and the rows
of each front; plus the **value map**: for every entry of the column structure the index in `a.val` it is copied from, and for
every position the index of its diagonal entry (or none: the value 0). The value map is made by running the existing
`permuted_lower_parts` and `columns_of` on the indices `0..nnz` in place of the values (they are generic copies; both are made
generic over a `Copy` value with an explicit zero, no arithmetic changes), so it is by construction the placement the existing
path makes.

- `SupernodalAnalysis::nested_dissection(a) -> Result<SupernodalAnalysis, SkylineError>`: `NotSquare` if `a` is not square,
  else the analysis with `nested_dissection_order(a)`.
- `n()`, `order()`, `matches(a) -> bool` (`a.n`, `a.ncols`, `row_ptr` and `col` equal to the analysed ones).
- `SupernodalLdlt::factor_analysed(a, &analysis) -> Result<SupernodalLdlt, SkylineError>`: positive pivots and the symmetry
  tolerance 1e-12, the rule of `factor`. `validate(a, 1e-12)` first (finding 3), then the values placed through the value map,
  then the numeric phase. **Panics** if `!analysis.matches(a)` (documented; the caller tests `matches`).

`supernodal_factor` is split into the symbolic phase (`analyse`, the text of its first half) and the numeric phase (`numeric`,
the text of its second half reading the analysis' arrays), and is `analyse` then `numeric`: the existing real and complex paths
keep their operations and order (B, and the existing tests of ADR 0503 and ADR 0509).

### D2. The reuse in the Newton driver (`crates/rcfd-fem/src/nonlinear.rs`, a1)

`pub struct FactorReuse` (Debug, Default): one cached `SupernodalAnalysis` (a `RefCell<Option<..>>`) and two counters,
`analyses()` (analyses made) and `factorisations()` (supernodal factorisations made). `factor_solve`'s supernodal arm takes a
`&FactorReuse`: if the cached analysis does not match the reduced tangent, it is replaced by
`SupernodalAnalysis::nested_dissection(a)` (an error mapped to "tangent factorisation: .." as now; `analyses += 1`); then
`SupernodalLdlt::factor_analysed` with the same mapping of a `Pivot` refusal to "singular tangent: .." and of the others to
"tangent factorisation: ..", the same post-test and the same solve (`factorisations += 1`). One slot, the last pattern: the
probe's sequence (A, then B throughout) needs no more, and a pattern that alternates is analysed again each time, never
wrongly reused. The envelope arm is unchanged in text and never reads the `FactorReuse` (defaults bit-identical).

Where the reuse lives:
- `Problem` gains `reuse: &FactorReuse`. `solve_with` makes one for the whole solve (every step, attempt and halving);
  `controlled_attempt` (the arc-length path, which refuses the supernodal factor) and `solve_from_field` make a fresh one per
  call (so `solve_from`, `solve_from_at`, `solve_in_time*` and the nonlinear transient reuse within one attempt).
- `pub fn solve_from_reusing(model, prescribed, load, start, states, options, contributions, reuse: &FactorReuse)`: `solve_from`
  with the given `FactorReuse`, so that a caller's sequence of attempts shares the analysis. `solve_from` is unchanged in
  behaviour (a fresh `FactorReuse`).
- **The contact driver** (`contact.rs`, total Lagrangian body): one `FactorReuse` per `contact::solve`, and `solve_from_reusing`
  in place of `solve_from` in the loop over steps and augmentations. Nothing else in `contact.rs` changes.

No option is added: the reuse changes no bit (a2), so it is always on with the supernodal factor (taken on the maintainer's
standing instruction of 2026-11-11, take the recommendation: a switch would only select the slower of two bit-equal paths). No
case-file key.

## How the time is measured (a3, a4)

Why not ADR 0542's protocol. ADR 0542's median of three pairs missed (0.355) while one pair met the limit (0.283): the
supernodal factor's parallel update is the part exposed to other processes (ADR 0542's diagnosis; the 1-thread solve took
22.2 s, the supernodal runs spread 1.6 times). Interference from other processes can only lengthen a run, never shorten it, so
the **minimum** of repeated runs is the estimate of the undisturbed time least affected by it; a median of three still takes
a disturbed run whenever two of three are disturbed. The count of repeats is fixed now.

The heavy record `f624_tl_hub_n16_reuse_against_present_heavy` builds the model once and calls `contact::solve` **ten times**
in one process, alternating present, supernodal (by `ContactOptions::factor(SupernodalNestedDissection)`), five of each; each
`t` the wall time of the one call (`Instant` around it). **T1 = `min(t_s) / min(t_p)`** over the five of each, fixed now. Then
one supernodal solve in a rayon pool of one thread (D below). rayon's default pool (16), `NewtonOptions::threads = None`.

Start and quiet machine (a4). The test executable is run directly (not through cargo) by a script
`f624-scratch/heavy.ps1`, so that no cargo or rustc process on the machine is this agent's during the run. Before starting, the
script samples every 30 s, for at most 120 minutes, until a sample has no `cargo` or `rustc` process and at least 3 GB of free
physical memory (estimate of the peak: about 1.5 GB, ADR 0542; free memory was 1.9 to 2.5 GB when this was written, with other
agents' builds), and starts then; after 120 minutes it starts regardless. During the run a sampler records every 5 s the number
of `cargo` and `rustc` processes. **Q** (a4) holds if that number is 0 at the start and at every sample until the run ends. Q is
registered as a check: the acceptance asks for it and this agent cannot stop the other agents; a miss of Q is recorded as such.

## Checks (limits fixed now)

`flat` and `digest` are ADR 0542's (copied from `sa_newton_supernodal_f618.rs`): the displacement, reactions, every contactor
node's force, pressure, gap and local coordinates, every Newton residual, energy, penetration, convex count and active set;
FNV-1a 64 over the bits.

| Id | Clause | Check | Limit | Estimate / basis |
| --- | --- | --- | --- | --- |
| T1 | a3 | heavy, `n = 16`, `d = 1e-4 a`: `min(t_s) / min(t_p)` over five alternating pairs | `< 1/3` | the arithmetic above: about 0.26 to 0.30; a risk remains if the numeric phase is slowed in every one of the five supernodal runs |
| Q | a4 | heavy: `cargo` and `rustc` processes on the machine at the start and at every 5 s sample of the timed run | 0 at every sample | not controlled by this agent; a real risk of a miss |
| V16 | a5 | heavy: the digest of `flat` of each of the five supernodal solves; of each of the five present solves | `0xfc936c7553ff35d5` each (ADR 0542 D, R2); `0xa5faff8918362a96` each (ADR 0542 R2) | finding 1; the probe reproduced the first |
| D | a2 | heavy: the supernodal solve in a rayon pool of one thread: digest | `0xfc936c7553ff35d5` | ADR 0503 D3; ADR 0542 D |
| V0 | a5 | fast, `n = 4`, `d = 1e-4 a`, `ContactOptions::factor(SupernodalNestedDissection)` and `ContactOptions::newton(... .factor(SupernodalNestedDissection))`: digest of `flat` | `0xecd8f40c5721a14f` both (ADR 0542 C1) | probe |
| A1 | a1 | fast, ADR 0542 Q5's cantilever (plain, no device), `NewtonOptions::new(1).factor(SupernodalNestedDissection)`, two calls of `solve_from_reusing` with one `FactorReuse`: the load `0.5 f` from 0, then `f` from the first result: `analyses()`; `factorisations()` | `1`; the number of corrections of both calls (`sum (residuals.len() - 1)`) | finding 2: no contribution, one pattern |
| A1b | a1 | fast, the same two calls with a second `FactorReuse`, the second call with a contribution that adds the tangent entries `(p, q)` and `(q, p)` of value `0.0` off the pattern (`p`, `q` two free equations of nodes not sharing an element): `analyses()` | `2` | finding 2: the pattern changed, a new analysis |
| A2 | a2 | fast: A1's and A1b's displacements, internal forces, residuals and energies against the same calls of `solve_from` (fresh analysis each, the path without reuse) | bit for bit | finding 1 |
| A3 | a2 | fast, `rcfd-linalg`: on the 3-D 7-point Laplacian `lap7(10)` (1 000 unknowns) and on a matrix of the same pattern with other values (every entry scaled by `1 + 0.1 sin(k)`, `k` its index, the result symmetrised by averaging the pair): `factor_analysed` with the first matrix's analysis against `SupernodalLdlt::factor` of the second: `order()`, `pivots()`, every `column(j)`, `solve` of a fixed right-hand side, `factor_entries()`, `supernodes()`, `max_front()`, `split_fronts()`, `peak_front_bytes()` | bit for bit / equal | finding 1 |
| A4 | a2 | fast, `rcfd-linalg`: the refusals equal (`SkylineError` by `==`) between `factor_analysed` and `factor` for: `lap7(10)` with every diagonal entry reduced by 7 (eigenvalues of `lap7` in `(0, 12)`, so indefinite: a `Pivot` refusal), `lap7(10)` with one value NaN (`NonFinite`), `lap7(10)` with one off-diagonal value changed by 1e-3 and its mirror not (`Unsymmetric`); each with the analysis of `lap7(10)` | equal | finding 3 |
| A5 | D1 | fast, `rcfd-linalg`: `SupernodalAnalysis::nested_dissection(a).order()` equals `nested_dissection_order(a)`; `NotSquare` for a 2 x 3 matrix | equal; holds | D1 |
| B | a2, a5, defaults | `crates/rcfd-fem/tests/sa_newton_f624_bits.rs` and `crates/rcfd-linalg/tests/sa_supernodal_f624_bits.rs`, written, run on b29c1383 plus this ADR before any library code, their digests written into them and committed: **G1** `SupernodalLdlt::factor` of `lap7(10)`, of A3's second matrix and of a 2-D 5-point Laplacian of 40 x 40: pivots and a solve; **G2** `ComplexSupernodalLdlt::factor` of a complex symmetric matrix of the `lap7(8)` pattern (`K + i 0.1 K` diagonal-shifted): pivots and a solve; **G3** the supernodal hub at `n = 8`, `d = 1e-4 a`: `flat`; **G4** ADR 0542 Q5's cantilever with D5's device (`tau = 0.35` of the plain tip deflection), supernodal, `nonlinear::solve_with`: per increment (step, load factor, halvings, iterations) and the displacement; **G5** the plain cantilever, supernodal, `nonlinear::solve`: displacement and reactions; **G6** the error text of the supernodal hub at `n = 4`, `d = 1e-3 a`; **G7** and **G8** G3 and G4 with the present factor. After the change: equal. Also unchanged and passing: `sa_newton_f618_default_bits.rs`, the fast tests of `sa_newton_supernodal_f618.rs`, `sa_f594_bits.rs`, `sa_supernodal.rs`, `sa_supernodal_complex.rs`, every other test of `cargo test -p rcfd-linalg --release` and `cargo test -p rcfd-fem --release`, and `cargo test -p rcfd-api --release --test baseline_hashes` | equal; pass | finding 1; D1, D2 |
| R | a3 | heavy, recorded: every `t_p`, `t_s`, the five pair ratios, `median(t_s) / median(t_p)` (ADR 0542's measure), the one-thread time, the free memory at the start, the start delay, the process samples | recorded | |

### Negative controls

| Id | Control | Must hold |
| --- | --- | --- |
| N1 | A1 with `DirectFactor::Envelope`: `analyses()` and `factorisations()` | both 0 (the default path does not touch the reuse) |
| N2 | `rcfd-linalg`: `factor_analysed` of a matrix with the same `n` and the same number of entries as the analysed one but one entry moved (a different `col`) | panics (`should_panic`) |
| N3 | A1b's count is not A1's | `2 != 1` (the pattern change is seen) |

### Planned mutations

On a `git archive` copy of the final tree in `f624-scratch/mut`, its own target directory `f624-scratch/target-mut`; each alone,
by exact text replacement; the fast tests (the new test files, without the heavy record) run once; the file restored.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| M1 | `matches` returns `true` always | A1b (and N2) |
| M2 | `matches` compares `n` and the number of entries only | N2 |
| M3 | the Newton arm does not store the new analysis (an analysis at every factorisation) | A1 |
| M4 | `factor_analysed` skips `validate` | A4 |
| M5 | in `numeric`, the children's updates added in decreasing child order | B (G1) |
| M6 | the value map of the diagonal ignored: every diagonal value placed as 0 | A3 |
| M7 | the envelope arm of `factor_solve` increments `factorisations` | N1 |

M5 changes the summation order of the extend-add (a change of bits, not an error). Declared before the run as **not planned**: the
contact driver's use of a per-call `FactorReuse` instead of one shared over its steps and augmentations is bit-equivalent by
design (a2) and observable only by the time, so no fast check can catch it; T1 measures the shared form.

## Tests (new files)

- `crates/rcfd-linalg/tests/sa_supernodal_f624_bits.rs` (G1, G2) and `crates/rcfd-fem/tests/sa_newton_f624_bits.rs` (G3 to
  G8): B, written and committed with their digests before any library code.
- `crates/rcfd-linalg/tests/sa_supernodal_analysis_f624.rs`: A3, A4, A5, N2.
- `crates/rcfd-fem/tests/sa_newton_reuse_f624.rs`: V0, A1, A1b, A2, N1, N3 (fast) and the heavy record
  `f624_tl_hub_n16_reuse_against_present_heavy` (`#[ignore]`, about 4 minutes, run once by `heavy.ps1`: T1, Q's process
  samples are taken by the script, V16, D, R).

## Decision rule

**Done** if every check meets its limit on its registered run (the heavy record and Q included), every negative control holds,
and every planned mutation is caught by a check named for it on its first run. **Partial** if a check misses or is not run, a
mutation survives or is caught only by another check, or a check is met only after an amendment made after its run. **Todo** if
the code does not build or a default bit changes and cannot be restored without changing a default.

## Audit of existing tests (before this registration)

No existing test asserts that the supernodal factor recomputes its order, or calls the private functions split here. The tests
that pin the bits the change could move are kept as they are and listed in B: `sa_supernodal.rs` and `sa_supernodal_complex.rs`
(ADR 0503, ADR 0509: the real and complex supernodal factors), `sa_f594_bits.rs` (the supernodal factor in the modal, buckling
and harmonic paths), `sa_newton_f618_default_bits.rs` and `sa_newton_supernodal_f618.rs` (ADR 0542), the contact bit records
of ADRs 0533, 0517 and 0507. ADR 0542's heavy record stays an ignored known miss; it is not re-run (its run stands). No test is
edited.

## Limits (not covered)

- One cached analysis (the last pattern); a pattern that alternates between two is analysed each time.
- `solve_from` called by a user in a loop keeps one analysis per call unless the caller uses `solve_from_reusing`; the nonlinear
  transient and `solve_in_time*` reuse within one attempt only (not checked beyond their existing records).
- The numeric phase is not changed: the time is that of the same factorisations less the pattern-only work.
- Q depends on the other agents' work; this agent waits for a quiet sample but cannot keep the machine quiet.
- Case-file key, the default factor and `SparseMinimumDegree` in the Newton driver remain ADR 0542's open questions 1 to 3.

## Open questions

1. **A switch for the reuse.** Not added (D2): taken on the maintainer's standing instruction of 2026-11-11 (take the
   recommendation).
