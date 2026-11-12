# ADR 0274: Sparse direct solver for symmetric positive definite matrices (envelope L D L^T after reverse Cuthill-McKee)

Status: **REGISTERED 2026-11-12, written before any code of F-373 and before any run of its checks; planned by ADR 0270**.
Not yet accepted by the maintainer. Before this text nothing of F-373 was written, compiled or run: the code and documents
under "What was read" were read, and the arithmetic under "Arithmetic before this registration" was done with a short
Python script of closed forms (level sizes of grid graphs, natural-order envelopes, the bound of D2), kept in the scratch
folder `w49i-work/f373-scratch/arith.py`. It does not run `rcm_order` or any factorisation. Results go in the addendum at the
end; nothing above the addendum is changed after a run.

Work package SB-4 of the structural solver, basic tier ([ADR 0270](0270-structural-solver-basic-tier-plan.md), section 3, the
entry for ADR 0274). Roadmap item F-373 (depends on F-025, F-026). Size M. Number: 0274, fixed by the plan. Branch
`f373-direct-spd-solver` from main edfc0b9f, worktree `w49i-work/f373`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment written and committed before the corrected run, the old test stays as an ignored record;
- each registered run is made once; nothing is re-run to pick a result;
- defaults bit-identical: nothing existing changes. The only edit of an existing file is one `pub mod skyline;` line in
  `crates/rcfd-linalg/src/lib.rs`; no existing function, test or golden is edited; nothing outside `crates/rcfd-linalg` changes;
- no new third-party crate, no unsafe code, the crate graph unchanged (`rcfd-linalg` keeps `rcfd-core` and `rayon`);
- clean-room: the envelope method and reverse Cuthill-McKee are textbook methods, derived below from first principles; the
  sources are cited from memory and nothing of them is used beyond what is derived here.

## What F-373 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. rcfd-linalg gains a direct factorisation of a symmetric
> positive definite CsrMatrix, reusable for many right-hand sides: on the 7-point Laplacian of 12^3 unknowns, on a randomly
> permuted copy (registered seed) and on a matrix with three unknowns per node built from it (Kronecker product with a 3 x 3
> SPD block) the relative residual is <= 1e-12 and the solution agrees with cg (tolerance 1e-13) within 1e-9; the permuted
> matrix factorises with an envelope no larger than 1.2 times that of the unpermuted one; a zero or negative pivot, an
> unsymmetric matrix and a non-finite entry are refused with the row; the result is bit-identical for 1 and 8 threads; a heavy
> record factorises 30^3 unknowns and reports time and memory. No existing function of the crate changes.

Clauses: (a1) a direct factorisation of an SPD `CsrMatrix` in `rcfd-linalg`; (a2) reusable for many right-hand sides; (a3)
three matrices (Laplacian 12^3, permuted copy, Kronecker with a 3 x 3 SPD block): relative residual <= 1e-12; (a4) the same
three: agreement with `cg` at tolerance 1e-13 within 1e-9; (a5) envelope of the permuted matrix <= 1.2 x that of the
unpermuted one; (a6) zero pivot, (a7) negative pivot, (a8) unsymmetric matrix, (a9) non-finite entry refused with the row;
(a10) bit-identical for 1 and 8 threads; (a11) heavy record of 30^3 unknowns with time and memory; (a12) no existing function
changes. The lead's brief adds: derive RCM and why it bounds the envelope (start node, tie-break), the envelope `L D L^T` and
its cost, the pivot test and its tolerance from rounding analysis, and measure the memory and time reach on this machine for
the structural models the plan expects (ADR 0270 gave arithmetic only); the existing `rcfd-linalg` tests pass unedited. ADR
0270 adds the signs of `D` (for the eigenvalue count of ADR 0283) and the module name `rcfd_linalg::skyline`.

## What was read (main edfc0b9f, read only)

1. `AGENT_RULES.md`; ADR 0270 in full for sections 1 to 3 (the conventions of 2.2: determinism for any thread count, every
   refusal located; 2.5 item 4: "`rcfd_linalg::skyline` (factor once, solve many, signs of `D`)"), the entries of ADRs 0274,
   0275, 0282, 0283 (0283 needs `L D L^T` of an indefinite shifted matrix without pivoting and the count of negative pivots);
   the stub of this ADR; ADRs 0001 to 0010 (head; 0003: deterministic chunked reductions, bit-reproducible results); ADR 0262
   (the house style).
2. `crates/rcfd-linalg`: `lib.rs`, `matrix.rs` (`CsrMatrix { n, ncols, row_ptr, col, val }`, columns sorted within rows,
   `from_triplets` sums duplicates, `matvec` parallel over fixed row chunks of `CHUNK = 4096`, `residual`, `diag`),
   `reorder.rs` (`rcm_order(n, lower, upper)`, `inverse_permutation`, `bandwidth`, `CsrMatrix::permute_symmetric`),
   `solvers.rs` (`cg` with `SolveControl { tol_abs, tol_rel, max_iter }`: stops when the recursive residual is
   `<= tol_abs` or `<= tol_rel x` the initial one; its thread test builds `rayon::ThreadPoolBuilder` pools), `vecops.rs`
   (`dot`, `norm2` with fixed chunking), the heads of `precond.rs` (`Identity`, `Jacobi`, `Ic0` on LDU), `amg.rs`, `backend.rs`;
   `tests/reorder.rs` (its LCG shuffle).
3. **`rcm_order` as it is** (it is used unchanged): adjacency from the edge list, symmetric, sorted, deduplicated; components
   started from the unvisited node of smallest (degree, index); the root is moved by repeated breadth-first search to the
   (degree, index)-smallest node of the last level while the number of levels grows (a pseudo-peripheral node in the sense of
   George and Liu); Cuthill-McKee numbering breadth-first with the unvisited neighbours of each node in ascending (degree,
   index); the whole sequence reversed. `order[i]` = old index at new position `i`. Deterministic, sequential.
4. The machine: AMD Ryzen 7 9800X3D (8 cores, 16 threads, 96 MB L3), 100.5 GB RAM, 26.3 GB free at the time of reading
   (other agents build concurrently).

## Arithmetic before this registration (disclosed)

All numbers from `f373-scratch/arith.py` (closed forms; no library code run).

- **Level structures from a corner.** On the 7-point graph of an `m^3` grid the breadth-first levels from a corner are the
  planes `i + j + k = s`, `3(m - 1) + 1` levels; on the 27-point graph they are the shells `max(i, j, k) = s`, sizes
  `3 s^2 + 3 s + 1`, `m` levels. For a Kronecker product with a 3 x 3 block (unknown `(p, c)` adjacent to `(q, c')` when
  `p = q` or `p ~ q`) the levels from a corner unknown are `1`, `2 + 3 L_1`, `3 L_2`, `3 L_3`, ...
- **Which root `rcm_order` takes** on these graphs (by reading item 3): the smallest degree is a corner's (3 on the 7-point
  grid; 11 = 2 + 9 for the Kronecker graph; 7 on the 27-point grid), so the start is the corner (or corner unknown) of smallest
  label; its last level is the opposite corner (7-point and Kronecker: one node or its 3 unknowns; 27-point: the far shell,
  whose smallest (degree, index) node is again a corner), from which the number of levels is the same, so the search stops at
  the first corner. This holds for any labelling, in particular for the permuted copy: its level structure is the same.
- **Envelopes** (strict lower envelope, D1) and the bound `U` of D2:

| Matrix | n | levels, widest | natural order envelope `E_nat` | RCM bound `U` (D2) | `U / E_nat` | estimate `sum L_s^2` |
| --- | --- | --- | --- | --- | --- | --- |
| 7-point 12^3 | 1728 | 34, 108 | 229 691 | 271 986 | 1.184 | 137 292 |
| 7-point 12^3 kron 3 x 3 | 5184 | 34, 324 | 2 072 403 | 2 458 294 | 1.186 | 1 235 660 |
| 7-point 30^3 | 27 000 | 88, 675 | 23 516 129 | 26 702 997 | 1.136 | 13 371 756 |
| 27-point 21^3 kron 3 x 3 | 27 783 | 21, 3783 | 36 780 723 | 124 510 132 | 3.385 | 66 079 205 |

  `E_nat` for the 7-point 12^3 by hand: rows with `k > 0`: `12 x 12 x 11 x 144 = 228 096`; `k = 0, j > 0`: `132 x 12 = 1584`;
  `k = 0, j = 0, i > 0`: 11; total 229 691. The "estimate" column takes the envelope width of a row as about its level size
  (D2); it is not a bound.
- **Cost** in multiply-adds, `sum_i w_i^2 / 2` (D3), estimate column above / bound: 7-point 12^3 6.2e6 / 2.4e7; its Kronecker
  1.7e8 / 6.6e8; 7-point 30^3 3.8e9 / 1.5e10; 27-point 21^3 kron: RCM 9.3e10 / 3.3e11, natural order 2.5e10 (exact).
- **Rate** (assumed, not measured): the inner loop is a dot product of two contiguous `f64` slices with four fixed
  accumulators, so about 1e9 to 3e9 multiply-adds per second on one core (adds of latency 3 to 4 cycles in four independent
  chains at about 5 GHz, less the loads; no FMA without `target-cpu`). The working set of one row's step is the window of the
  previous `w` rows, `8 w^2` bytes: 3.6 MB at `w = 675`, in L3.
- **Conditioning.** Dirichlet 7-point Laplacian of 12^3 (diagonal 6): eigenvalues `sum_d (2 - 2 cos(k_d pi / 13))`, `k_d = 1..12`:
  `lambda_min = 0.174349`, `lambda_max = 11.825651`, `kappa = 67.83`. The block `B = [[4, 1, 0.5], [1, 3, 0.25], [0.5, 0.25,
  2]]` (strictly diagonally dominant, so SPD by Gershgorin): eigenvalues 1.8817, 2.3868, 4.7316, `kappa(B) = 2.51`;
  `kappa(A (x) B) = kappa(A) kappa(B) = 170.6`. Distinct eigenvalues of `A` in ascending order begin 0.174349, 0.345321,
  0.516292, 0.619211, 0.687264, 0.790183, 0.961154, 0.980103, 1.064073, 1.151075, 1.235045; 32 eigenvalues (with
  multiplicity) lie below the midpoint 1.193060 of the 10th and 11th.
- **Reach for cube-shaped models with three unknowns per node** (27-point node graph, natural node order, `w ~ 3 (m^2 + m +
  1)` with `m = (N/3)^(1/3)` nodes per edge): `N = 1e4`: envelope 7.2e6 (0.06 GB), cost 2.6e9 (1 to 3 s); `N = 3e4`: 4.4e7
  (0.35 GB), 3.2e10 (11 to 32 s); `N = 1e5`: 3.2e8 (2.6 GB), 5.1e11 (3 to 9 min). Memory is not the limit on this machine
  (26 GB free would hold about 3e9 envelope entries); time is, and it grows like `N^(7/3)` for cubes. Slender bodies (a
  cantilever, a blade) have narrow levels and cost far less. ADR 0270's figure (2.8e4 unknowns, about 0.3 GB) agrees with the
  natural-order line; RCM from a corner is worse than the natural order on a 27-point cube (estimate 1.8 x), which is why D5
  offers the smaller of the two.

## Design

**D1. Envelope.** For a symmetric matrix `A` (stored pattern symmetric or not) in a given order, the first column of row `i` is
`f_i = min({i} U {j < i : a_ij or a_ji stored})`, its width `w_i = i - f_i`, and the (strict lower) envelope
`Env(A) = {(i, j) : f_i <= j < i}`, `|Env| = sum_i w_i`. Stored zeros count (the pattern is what is stored).

*No fill outside the envelope* (George and Liu, Computer Solution of Large Sparse Positive Definite Systems, 1981, chapter 4,
cited from memory; derived here): with `A = L D L^T`, `L` unit lower triangular, row `i` of `L` below the diagonal is the
solution of `L_[i-1] D_[i-1] l_i = a_i`, where `L_[i-1]`, `D_[i-1]` are the leading `(i-1) x (i-1)` blocks and `a_i` is the part
of row `i` left of the diagonal. Forward substitution with a lower triangular matrix maps a vector whose first `f_i` entries are
zero to a vector whose first `f_i` entries are zero. Hence `f_i(L) = f_i(A)`: the factor fits exactly into the envelope of
`A`, and the storage is fixed before the factorisation.

**D2. Reverse Cuthill-McKee, and why it bounds the envelope.** Let a breadth-first search from a root `r` give levels
`S_0 = {r}, S_1, ..., S_h` of sizes `L_s`, and let the Cuthill-McKee order number the nodes level by level (positions `P_s` to
`P_{s+1} - 1` for level `s`). Every edge joins nodes of the same or of adjacent levels. In the *reversed* order, row `v` of
level `s` has as lower neighbours exactly its neighbours with larger Cuthill-McKee position, all in levels `s` and `s + 1`,
so `w_v <= (P_{s+2} - 1) - P_s = L_s + L_{s+1} - 1`, and

`|Env| <= U = sum_s L_s (L_s + L_{s+1} - 1)`.

The envelope is therefore controlled by the level widths, which is what a good root achieves: the widths sum to `n` over
`h + 1` levels, so the more levels, the narrower they are on average. A pseudo-peripheral root (a node of nearly maximal
eccentricity, found by the repeated breadth-first search of `rcm_order`: move to the smallest-degree node of the last level
while the number of levels grows) gives nearly the most levels. Within a level, numbering neighbours in ascending degree puts
the nodes with few connections to the next level first, which keeps the actual widths near `L_s` rather than at the bound
(the estimate column). Reversal is the step that makes the order profile-friendly for a row-oriented envelope: George's
observation (George, thesis, Stanford 1971; Liu and Sherman, SIAM J. Numer. Anal. 13 (1976) 198, that the reversed order's
envelope is never larger than the forward one's; both from memory) is not used as a premise here, only the bound `U`, which
is derived above. Tie-breaks are deterministic: (degree, label) everywhere, so the order depends on the labelling only through
ties, and the bound `U` does not depend on it at all.

For the 12^3 Laplacian and its permuted copy the root is a corner (arithmetic), so both orders satisfy `|Env| <= U = 271 986
< 1.2 x E_nat = 275 629`: clause (a5) holds by derivation whatever the labels do inside the levels; the check measures it.
The same holds for the Kronecker matrix (`U / E_nat = 1.186`). The order is computed on the scalar graph of the matrix (every
unknown a node), with the existing `rcm_order` unchanged.

**D3. Envelope L D L^T, row by row** (the bordering, or row-Doolittle, form). Storage: `first[i] = f_i`, `ptr[i]` (start of
row `i` in `env`, `ptr[i+1] - ptr[i] = w_i`), `env` (the `|Env|` values of row `i`, columns `f_i .. i-1`), `d` (n pivots).
For `i = 0 .. n-1`, with row `i` loaded with `a_ij` (zero where not stored):

1. for `j = f_i .. i-1`: `lo = max(f_i, f_j)`; `u_ij = a_ij - sum_{k=lo}^{j-1} u_ik l_jk` (row `i` holds the unscaled
   `u_ik = l_ik d_k` for `k < j`, row `j` the final `l_jk`): this is `sum_k l_ik d_k l_jk = (L D L^T)_ij` solved for `l_ij d_j`;
2. `l_ij = u_ij / d_j` for every `j`, stored over `u_ij`; then `t = sum_j u_ij l_ij` and `d_i = a_ii - t`;
3. the pivot test (D4) on `d_i`.

Every sum is over contiguous slices of two rows (`env[ptr[i] + lo - f_i ..]` and `env[ptr[j] + lo - f_j ..]`), evaluated by
one fixed routine: four accumulators over blocks of four in index order, the tail into the first, combined as
`(s0 + s1) + (s2 + s3)`. **Cost:** step 1 for row `i` is `sum_{j} (j - lo) <= w_i (w_i - 1) / 2` multiply-adds, steps 2 and 3
`2 w_i`; the factorisation costs at most `sum_i w_i^2 / 2 + O(|Env|)`, storage `|Env| + n` values plus `3 n` indices. For
`w_i ~ w` this is `n w^2 / 2`, the band-Cholesky cost, but with each row paying only for its own width.

**Solve** (many right-hand sides, the factor is not changed): `y_i = b_{order[i]}`; forward `y_i -= sum_{j=f_i}^{i-1} l_ij y_j`
(the same dot routine); `y_i /= d_i`; backward for `i = n-1 .. 0`: `y_j -= l_ij y_i` for `j = f_i .. i-1` in ascending `j`;
`x_{order[i]} = y_i`. Cost `2 |Env| + n` multiply-adds.

**Threads.** The factorisation, the ordering and the solve are sequential with a fixed operation order, so their bits do not
depend on the thread count (ADR 0003's requirement, met trivially). The validation scans (D6) are sequential too. A parallel
factorisation (independent rows of a level, or column blocks) is left out; its gain would be bounded by the window
dependency of D3 and it is not needed for the reach above.

**D4. Pivot test and its tolerance** (rounding analysis). In step 2, `d_i` is computed as `a_ii - t` with `t` a sum of
`w_i` products. With unit roundoff `u = 2^-53` and the standard bound for a computed sum of `m` terms (Higham, Accuracy and
Stability of Numerical Algorithms, 2nd ed. 2002, chapter 3 and Theorem 10.3 for Cholesky, from memory; derived: each of the at
most `m` additions and the products contributes a relative error `<= u`, first order), the computed pivot satisfies

`|fl(d_i) - (a_ii - sum_j u_ij l_ij)| <= gamma_{w_i + 1} s_i`, `s_i = |a_ii| + sum_j |u_ij l_ij|`, `gamma_m = m u / (1 - m u)`

(the four-accumulator order has a smaller bound, `gamma_{ceil(m/4) + 2}`; the larger one is used). A computed pivot below this
size has no certified sign with respect to the computed row. The test is therefore

`d_i <= theta_i = (w_i + 1) eps s_i` with `eps = f64::EPSILON = 2 u` (refuse: not positive definite),

a factor of two above `gamma_{w_i + 1}` (for `(w_i + 1) u << 1`). For an SPD matrix `s_i = a_ii + sum_j u_ij^2 / d_j =
2 a_ii - d_i` lies in `[a_ii, 2 a_ii]`, so the test is `d_i / a_ii <~ 2 (w_i + 1) eps`: `1.3e-13` at `w_i = 300`, `3e-12` at
`w_i = 7000`. Since `d_i / a_ii >= 1 / kappa` of the leading block (`1/d_i` is the last diagonal entry of the inverse of the
leading `(i+1) x (i+1)` block, and that entry is at most `1 / lambda_min`, while `a_ii <= lambda_max`), an SPD matrix with a
condition number below about `1 / (2 (w + 1) eps)` (2e12 at `w = 1000`) always passes. The bound covers the last step of the row
only; the errors already in the `u_ij` are the backward error of the whole factorisation (Higham's theorem: the computed factor
is the exact factor of `A + dA`, `|dA| <= gamma_{w+1} |L| |D| |L^T|`), which the residual checks measure. The test is the
refusal of (a6) and (a7): a zero pivot (`d_i = 0 <= theta_i`), a negative pivot, and a positive pivot too small to be told
from zero.

The ADR 0283 use (`L D L^T` of the indefinite `K - sigma M`) gets a second rule, `PivotRule::NonZero`: refuse only
`|d_i| <= theta_i`; the factor then reports the inertia (counts of negative and positive `d_i`), equal by Sylvester's law of
inertia to the counts of negative and positive eigenvalues of `A` when the factorisation is exact (`A = L D L^T` is a
congruence). Without pivoting this is not backward stable for indefinite matrices (growth of `|L| |D| |L^T|`), which C11
measures on mildly indefinite shifts only.

**D5. The API** (new module `crates/rcfd-linalg/src/skyline.rs`, declared by one `pub mod skyline;` line; no re-export):

- `enum SkylineOrdering { Rcm, Natural, Given(Vec<usize>), SmallerEnvelope }`: `Rcm` (the default) is `rcm_order` of the
  matrix graph; `SmallerEnvelope` takes RCM or the natural order, whichever has the smaller `|Env|` (ties: RCM).
- `enum PivotRule { Positive, NonZero }` (D4), default `Positive`.
- `struct SkylineOptions { ordering, pivots, symmetry_tolerance }`, default `Rcm`, `Positive`, `1e-12` (D6).
- `struct SkylineLdlt` (Clone, PartialEq, Debug): `factor(&CsrMatrix) -> Result<SkylineLdlt, SkylineError>` with the
  defaults, `factor_with(&CsrMatrix, &SkylineOptions)`; `solve(&self, b) -> Vec<f64>`, `solve_into(&self, b, x)`;
  `n()`, `order()`, `pivots()` (the `d_i` in factor order), `inertia() -> (negative, positive)`, `envelope_size()`,
  `storage_bytes()` (the factor's arrays: `8 (|Env| + n) + 8 (3 n + 1)`).
- `fn envelope_size(a: &CsrMatrix, order: Option<&[usize]>) -> usize` (D1).
- `enum SkylineError` with `Display` and `std::error::Error`, every variant located:
  `NotSquare { rows, cols }`; `NonFinite { row, col, value }`; `Unsymmetric { row, col, a_rc, a_cr }`;
  `Pivot { row, position, pivot, threshold }` (`row` is the original index, `position` its place in the factor order);
  `BadOrdering { reason }` (a `Given` vector that is not a permutation of `0..n`). Rows and columns are original indices.

**D6. Validation before the factorisation** (in this order, each the first offender in row-major order of the input):

1. `ncols != n`: `NotSquare`;
2. a stored value not finite: `NonFinite` with its row and column;
3. symmetry: for each stored off-diagonal `(r, c)`, `a_cr` (by binary search in row `c`, zero if not stored) must satisfy
   `|a_rc - a_cr| <= delta sqrt(|a_rr| |a_cc|)`, `delta = symmetry_tolerance = 1e-12`, else `Unsymmetric { row: r, col: c }`.
   Reason for the scale: an SPD matrix has `|a_rc| <= sqrt(a_rr a_cc)`; an assembled matrix whose element blocks are symmetric
   up to rounding differs by a few `eps` of that, and `1e-12` is 4500 `eps`. The factorisation then reads the lower
   triangle (`a_ij`, `j < i`, in the factor order: the input entry `(order[i], order[j])` if stored, else its transpose).
4. `Given` ordering: a permutation, else `BadOrdering`.

A missing or zero diagonal is not a separate error: it fails the pivot test at its row (`d_i <= 0`). Columns are assumed
sorted within rows, as `CsrMatrix` documents.

## Set-ups

New test file `crates/rcfd-linalg/tests/sb_direct.rs`. All runs in release.

- **A** (7-point Laplacian of 12^3): unknown `p = i + 12 j + 144 k`, diagonal 6, `-1` to each grid neighbour (Dirichlet
  boundary by omission), built with `CsrMatrix::from_triplets`.
- **P** (permuted copy): `A.permute_symmetric(perm)` with `perm` the Fisher-Yates shuffle of `tests/reorder.rs`'s LCG
  (`s <- s * 6364136223846793005 + 1442695040888963407`, `j = (s >> 33) mod (i + 1)`) from seed `0x9E3779B97F4A7C15`.
- **K** (three unknowns per node): `A (x) B`, unknown `3 p + c`, `B` as in the arithmetic, from triplets.
- **Right-hand sides**: `b_q` from the same LCG with seed `0x2545F4914F6CDD1D + q` (q = 0, 1, 2, 3), entries
  `((s >> 11) as f64 / 2^53) * 2 - 1`, in `[-1, 1)`. For P: `b_P[i] = b_A[perm[i]]` (the permuted system of A's).
- **cg reference**: `cg(&M, &b, &mut x0 = 0, &Identity, &SolveControl { tol_abs: 0.0, tol_rel: 1e-13, max_iter: 5000 })`;
  estimate of iterations: `sqrt(kappa)/2 ln(2/1e-13)`: 128 for A, P; 202 for K.
- **Norms**: relative residual `||b - M x||_2 / ||b||_2` with `CsrMatrix::residual`; agreement
  `||x - x_cg||_2 / ||x_cg||_2` (the 2-norm, in which the forward-error bound `kappa x` relative residual holds; in the max
  norm the worst case would carry a further `sqrt(n)`, 72 for K, which would put K's bound at 1.4e-9).
- **H1** (heavy): 7-point Laplacian 30^3 (27 000 unknowns), default options, `b_0`.
- **H2** (heavy): the sparsity of a Hex8 box of 20^3 elements: 21^3 nodes, the 27-point node graph Laplacian
  (diagonal = degree, `-1` per neighbour) plus the identity, Kronecker with `B` (27 783 unknowns); factorised once with
  `Natural` and once with `Rcm`, `b_0`.

## Registered checks

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| C1 | a1, a3, a4 | A, default options: relative residual; agreement with cg; cg converged (control) | `<= 1e-12`; `<= 1e-9`; converged | D4: `|r| <~ c sqrt(w) u ||A|| ||x||`; with random `b`, `||A|| ||x|| / ||b|| ~ 10`, `w <= 215`: about 3e-14. Agreement: `kappa (1e-13 + 1e-14) = 7.5e-12` |
| C2 | a3, a4 | P: the same three | the same | the same (`kappa` unchanged by permutation) |
| C3 | a3, a4 | K: the same three | the same | `w <= 650`: about 5e-14; agreement `170.6 x 1.1e-13 = 1.9e-11` |
| C4 | a5 | `envelope_size(A, None) = E_nat`; the factor of P (default RCM): `|Env| <= 1.2 E_nat`; `|Env| <= U` for the factors of A and P | `= 229 691`; `<= 275 629`; `<= 271 986` | D1 (by hand), D2 (bound) |
| C4k | a5 (extended) | K: `envelope_size(K, None)` and the RCM factor's `|Env|` | `= 2 072 403`; `<= 2 458 294` | arithmetic |
| C5 | a2 | one factor of K, the four `b_q`: each relative residual; the solve of `b_0` repeated after the others: bit-identical to the first; the factor `==` a clone taken before the solves | `<= 1e-12`; equal bits; equal | `solve(&self)`; D3 |
| C6 | a6 | path Laplacian of 50 nodes (diagonal 1, 2, ..., 2, 1; `-1` off): `Natural`: `Pivot { row: 49, position: 49 }` with `pivot = 0.0` exactly; default RCM: `Pivot` at `position = 49`, `row = order[49]`, `pivot = 0.0` | as stated | exact integer arithmetic: `d = 1, 1, ..., 1, 0` |
| C7 | a7 | A with `a_777,777 = -1`: `Pivot { row: 777 }`, `pivot < 0` (default RCM); `diag(1, 2, -3, 4)`, `Natural`: `Pivot { row: 2, position: 2, pivot: -3 }`; A with diagonal 3 (A - 3I, indefinite, all diagonal entries positive): a `Pivot` error with `order[position] = row` and `pivot <= threshold` | as stated | rows eliminated before 777 do not involve `a_777,777`; a non-SPD matrix must fail some pivot (D4) |
| C8 | a8 | A with `a_5,6 = -1 + 1e-9`: `Unsymmetric { row: 5, col: 6 }`; A with an extra stored `a_0,100 = 0.5`: `Unsymmetric { row: 0, col: 100 }`; negative control: A with `a_5,6 = -1 + 1e-15` factorises | as stated | D6 |
| C9 | a9 | A with `a_3,3 = NaN`: `NonFinite { row: 3, col: 3 }`; A with `a_10,11 = +inf`: `NonFinite { row: 10, col: 11 }`; a 2 x 3 matrix: `NotSquare`; `Given` with a repeated index: `BadOrdering` | as stated | D6 |
| C10 | a10 | K: factor and solve of `b_0` inside rayon pools of 1 and of 8 threads: `order`, every `f64` of the envelope and of `d`, and of `x` | bit for bit | D3 (sequential) |
| C11 | ADR 0283 use | `NonZero`: `diag(1, 2, -3, 4)` inertia `(1, 3)`; `A - sigma_1 I`, `sigma_1 = (0.174349 + 0.345321) / 2 = 0.259835`: inertia `(1, 1727)`; `A - 1.193060 I`: inertia `(32, 1696)` | exact counts | Sylvester; arithmetic. Risk: growth without pivoting (D4) |
| C12 | a12, brief | `cargo test -p rcfd-linalg --release`: every existing test passes, no existing test file edited; `git diff main --stat` touches `lib.rs` (one added line) and new files only | as stated | D5 |
| C13 | (extended) | P's solution against A's solution permuted, `||x_P - x_A[perm]||_2 / ||x_A||_2` | `<= 1e-10` | both backward stable: `kappa x 2 x 3e-14 ~ 4e-12`; margin 25 |
| H1 | a11 (ignored, heavy record) | H1: `|Env| <= U`; relative residual; wall time of order + factor; time of one solve; `storage_bytes`; the process's peak working set, read by an outside PowerShell poll of the test process (every 0.2 s) | `<= 26 702 997`; `<= 1e-12`; total `<= 300 s` (budget); bytes and peak recorded | estimate 1.3 to 4 s, 107 MB (estimate) / 214 MB (bound) |
| H2 | brief: reach of structural models (ignored, heavy record) | H2 `Natural`: `|Env|`; `Rcm`: `|Env|`; both: relative residual, times, `storage_bytes`; peak working set as in H1 | `= 36 780 723`; `<= 124 510 132`; `<= 1e-12`; each factorisation `<= 600 s` (budget); recorded | natural 8 to 25 s and 0.29 GB; RCM 31 to 93 s (estimate) and 0.53 GB (estimate) / 1.0 GB (bound) |
| R1 | record | Neumann graph Laplacian of 12^3 (singular, null vector 1): refused or not; the last `d` and `theta` | recorded, no limit | estimate (backward error projected on the null vector): `|d_n| ~ 7e-14` against `theta_n ~ 1.4e-13`: borderline, so not registered as a check |
| R2 | record | `|Env|` of the factors of A and P, and their ratio | recorded | D2: both `<= U`; the ratio is not bounded tighter than `U / (lower bound)` |

C8's negative control: `|a_56 - a_65| = 1e-15 <= 1e-12 sqrt(6 x 6) = 6e-12`.

**Negative controls** inside the checks: cg convergence in C1 to C3; C8's tolerated perturbation; C7's indefinite matrix with a
positive diagonal (the refusal does not come from a diagonal scan).

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f373-scratch`, own target directory; each applied alone
by exact text replacement, the named tests run once, the file restored and compared byte for byte):

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | the default ordering is `Natural` instead of `Rcm` | C4 (P's envelope) |
| M2 | step 1 sums from `f_i` instead of `max(f_i, f_j)` (offsets of row `j` misread) | C1 to C3 (or a panic) |
| M3 | the pivot test `d_i <= theta_i` replaced by `d_i < 0.0` | C6 |
| M4 | the symmetry scan removed | C8 |
| M5 | the finite scan removed | C9 |
| M6 | `l_ij = u_ij / d_j` not applied (rows kept unscaled) | C1 to C3 |
| M7 | the solve writes `x_i = y_{order[i]}` instead of `x_{order[i]} = y_i` | C1 to C3 |
| M8 | the forward-solve dot split into `rayon::current_num_threads()` chunks summed in chunk order | C10 |
| M9 | the pivot threshold `theta_i` set to 0 in `NonZero` and the inertia counted with `d_i >= 0` as positive | C6 (with `NonZero` in its `Natural` case it must still refuse `0.0`) |

**Decision rule.** F-373 is proposed **done** (pending the maintainer's acceptance) if C1 to C10, C12 and H1 are met (every
acceptance clause) and every mutation fails its test. C4k, C11, C13 and H2 are registered extensions: a miss among them is a
`*_known_miss` record and a follow-up item, and does not alone block done. A miss or an unrun check among C1 to C10, C12, H1, or
a mutation not caught (after the test is checked to cover it): **partial**, with the miss named. No factorisation that solves
A: **todo**.

## Limits (what is not covered)

- No fill-reducing ordering (minimum degree, nested dissection): the envelope of a cube-shaped 3-D model grows like `N^(5/3)`
  and the cost like `N^(7/3)`; beyond about 1e5 unknowns the iterative route of ADR 0282 is the answer, a better ordering is
  intermediate tier (ADR 0270 section 1.2).
- No pivoting: indefinite matrices are factorised only by the `NonZero` rule, without stability guarantees (D4).
- No parallel factorisation; no supernodes or blocking; no out-of-core storage.
- No complex, unsymmetric or rectangular matrices; no update or downdate of a factor.
- The memory figure is the factor's arrays plus the peak working set of a test process; allocator overhead is not separated.

## Amendment 1 (2026-11-12, written and committed before the runs it registers)

The registered fast checks C1 to C13 were run once (all met; the numbers go in the addendum) and the mutations M1 to M9 were run
once on a scratch copy. Two mutations survived, and both are gaps of the test set, not of the code:

- **M2** (`lo = f_i` instead of `max(f_i, f_j)`) passed C1 to C3. Diagnosis: on every matrix of C1 to C3 the envelope is
  monotone (`f_j <= f_i` for `j < i`): true of a breadth-first (Cuthill-McKee type) order and of the lexicographic order of a
  grid, so `max(f_i, f_j) = f_i` and the mutant is equivalent there. It is not equivalent for a non-monotone envelope, which
  the default RCM never produced in these checks but a `Given` or `Natural` order of a scrambled matrix does.
- **M9** (`NonZero` refuses nothing) passed C6: C6 has no `NonZero` case, contrary to what the mutation table assumed.

Added checks (new tests; C1 to C13 and their limits are unchanged):

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| C2b | P factorised with `Natural` (its scrambled input order, a non-monotone envelope of 1 098 246 as printed by C4): relative residual; agreement with cg; and the envelope is not monotone (some `j < i` with `f_j > f_i`) | `<= 1e-12`; `<= 1e-9`; not monotone (control) | as C1 with `w <= 1727`: `sqrt(1727) u 10 ~ 5e-14`; agreement as C1 |
| C6b | path Laplacian of 50 nodes, `Natural`, `NonZero`: `Pivot { row: 49, position: 49, pivot: 0.0 }` | as stated | exact arithmetic as C6 |

M2 is then rerun against C2b and M9 against C6b, once each.
