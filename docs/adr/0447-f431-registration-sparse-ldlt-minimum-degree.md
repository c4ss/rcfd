# ADR 0447: F-431 registration: sparse L D L^T after an approximate minimum degree ordering, with the inertia, selectable in the static solve and accepted by the modal and buckling eigensolvers

Status: **REGISTERED 2026-11-14, written before any code of F-431 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-431 was written, compiled or run: the files under "What was read" were read; the only
arithmetic done beforehand is disclosed below (a closed-form nested-dissection estimate by a short Python script,
`w49i-work/f431-scratch/nd_estimate.py`, and hand estimates). Results go in the addenda at the end; nothing above the addenda is
changed after a run.

Work package SA-24 of the structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md); stub
[ADR 0334](0334-sparse-cholesky-with-fill-reducing-ordering.md), kept unchanged). Roadmap item F-431 (deps F-408, F-373, F-374,
F-382, F-402). Size M. Number 0447 given by the lead. Branch `f431-sparse-ldlt` from main 46d78b64, worktree `w49i-work/f431`,
scratch `w49i-work/f431-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact); each registered run made once; defaults bit-identical; no edit of `baseline_hashes.rs`, of the case
and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph unchanged; no existing test edited
except the standing decision on retired scaffold rows (below); clean-room.

## What F-431 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Five-point grid 100 x 100 and seven-point grid 30^3: residual below 1e-12 relative, and on the 30^3 grid at most half the factor entries of the envelope after reverse Cuthill-McKee (both counts recorded). On a solid model the solution equals the envelope solver's within 1e-10 and the modal frequencies through either factorisation agree within 1e-9. Inertia: for the tridiagonal (2, -1) of order 200 and three shifts the number of negative pivots of K - s I equals the number of eigenvalues 4 sin^2(j pi/(2 (n + 1))) below s. A matrix that is not positive definite is reported with the equation when positive definiteness was requested. Factor and solution are bit-identical for 1 and 8 threads. Heavy record: a clamped cube of about 3e5 unknowns is solved, time and memory recorded beside the envelope's estimate. The default solver's results keep their bits.

Clauses: (c1) limits registered before the run (this document); (c2) 5-point 100 x 100: relative residual < 1e-12; (c3) 7-point
30^3: relative residual < 1e-12; (c4) on 30^3 the factor entries at most half the envelope after RCM, both counts recorded;
(c5) on a solid model the static solution equals the envelope solver's within 1e-10; (c6) the modal frequencies through either
factorisation agree within 1e-9; (c7) the inertia of `K - s I`, tridiagonal (2, -1), n = 200, three shifts, equals the closed-form
count; (c8) a matrix that is not positive definite is reported with the equation when positive definiteness is requested; (c9)
factor and solution bit-identical for 1 and 8 threads; (c10) heavy record: a clamped cube of about 3e5 unknowns solved, time and
memory recorded beside the envelope's estimate; (c11) the default solver's results keep their bits. The title adds (c12): the
factorisation is used by the static, the buckling and the modal solves (selectable; the default unchanged).

## What was read (main 46d78b64, read only)

1. `AGENT_RULES.md`; the stub ADR 0334; the roadmap entries F-373, F-374, F-382, F-402, F-408, F-431; ADR 0274 (envelope
   `L D L^T`, its arithmetic table: the 7-point 30^3 RCM envelope estimate 13 371 756 and the heavy record 0.79 s, 109 MB; the
   measured rate 4.6e9 multiply-adds per second); ADR 0440 (the F-408 scaffold: `rcfd_linalg::sparse_chol` declared with
   `PENDING = Some("ADR 0334 (F-431)")`, checked by row X1 of `sa_scaffold.rs`); the headings and the decision rules of ADRs
   0440 and 0444; ADR 0291 lines on `PENDING`.
2. Code: `crates/rcfd-linalg/src/{skyline.rs (whole), eigen.rs (lines 450 to 1178: options, factor_shifted, count_unchecked,
   lowest_eigenpairs, lowest_buckling_factors, buckling_count), sparse_chol.rs, lib.rs line 21}`;
   `crates/rcfd-fem/src/{solve.rs (whole), modal.rs (whole), buckling.rs (lines 15 to 252), mpc.rs (lines 1120 to 1240,
   1450 to 1470), cyclic.rs (lines 865 to 880)}`; the struct literals of `SubspaceOptions` and `BucklingEigenOptions` in every
   crate (all end in `..Default::default()` or `..backward(TAU)`); `ModalOptions`, `BucklingOptions`, `StaticOptions`,
   `StaticSolver` are `#[non_exhaustive]`; `GeneralizedEigen` and `BucklingEigen` are constructed only in `eigen.rs`.
3. Tests read for set-ups: `crates/rcfd-fem/tests/si_amg.rs` lines 1 to 70 (the clamped Hex8 cube `cube(n)`: `hex_box([0; 3],
   [1; 3], [n; 3])`, E = 2e11, nu = 0.3, `xmin` clamped, traction (0, 0, -1e6) on `xmax`; `cube(46)` has 304 842 unknowns),
   `sb_static.rs` lines 700 to 780 (the hinge mechanism E5), `sa_scaffold.rs` lines 1130 to 1170 (X1), the rayon pools of
   `sb_direct.rs` line 464.
4. **Not read:** any sparse-direct code outside this repository. The methods are cited from memory and derived in D1 to D4:
   P. R. Amestoy, T. A. Davis, I. S. Duff, "An approximate minimum degree ordering algorithm", SIAM J. Matrix Anal. Appl. 17
   (1996) 886-905 (the quotient graph, element absorption, supervariables, the approximate external degree); A. George and
   J. W. H. Liu, "The evolution of the minimum degree ordering algorithm", SIAM Review 31 (1989) 1-19, and J. W. H. Liu,
   "Modification of the minimum-degree algorithm by multiple elimination", ACM Trans. Math. Softw. 11 (1985) 141-153 (mass
   elimination, indistinguishable nodes); J. W. H. Liu, "A compact row storage scheme for Cholesky factors using elimination
   trees", ACM Trans. Math. Softw. 12 (1986) 127-148 and "The role of elimination trees in sparse factorization", SIAM J. Matrix
   Anal. Appl. 11 (1990) 134-172 (the elimination tree with path compression, the row structure as the tree reach); T. A. Davis,
   "Algorithm 849: a concise sparse Cholesky factorization package", ACM Trans. Math. Softw. 31 (2005) 587-591 (the up-looking
   `L D L^T` by rows); A. George, "Nested dissection of a regular finite element mesh", SIAM J. Numer. Anal. 10 (1973) 345-363
   (used only for the fill estimate). None of these is a premise of a check: the checks are the residual, the fill count, the
   closed-form spectrum and the envelope solver.

Findings that shape the design:

1. The eigensolvers take the factorisation through `SkylineOrdering` and call `SkylineLdlt` directly in four places
   (`count_unchecked`, `factor_shifted`, `buckling_count`, `lowest_buckling_factors`). Every literal of their option structs
   ends in `..Default::default()`, so a new field with a default changes no caller.
2. The static solve chooses with `StaticSolver` (`#[non_exhaustive]`). The transformation path (`mpc::solve_static_mpc`) maps
   every solver other than CG to the envelope (`_ =>`), so a new variant would be silently replaced there: it must be refused
   on that path. The cyclic static solve already refuses every solver but `Direct`; the refinement of ADR 0397 refuses every
   solver but `Direct`.
3. `ModalOptions` is turned into `SubspaceOptions` in `modal.rs`, `mpc.rs` (modal on the transformation) and `geometric.rs`
   (prestressed modes, two sites); `BucklingOptions` into `BucklingEigenOptions` in `buckling.rs` (two sites).
4. `skyline::validate` (square, finite, symmetric within a tolerance) and `check_permutation` are private; the new module needs
   the same refusals. Making them `pub(crate)` changes no bit.
5. Landing the module sets `sparse_chol::PENDING` to `None`, which retires row `sparse_chol` of X1 in `sa_scaffold.rs`.

## Arithmetic before this registration (disclosed)

- **Envelope of the 7-point 30^3 grid after RCM.** ADR 0274's estimate `sum L_s^2` = 13 371 756; its heavy record 109 MB for
  the factor (`8 (|Env| + n) + 8 (3n + 1)` bytes, so `|Env|` about 1.33e7). Half of it: about 6.7e6.
- **Nested dissection estimate** (script above; geometric ND on a box, each separator column holding the later separator nodes
  and the bordering face lines; an estimate of the order of magnitude, not a bound): 7-point 30^3: `nnz(L)` about 1.5e6, 3.9e8
  flops. Minimum degree is known (George and Liu 1989, from memory) to fill more than nested dissection on 3-D grids, by a small
  factor; with a factor 3 the expected ratio to the RCM envelope is about 0.34, under the limit 0.5.
- **Heavy cube** (47 x 47 x 46 free nodes, three unknowns each, 304 842 unknowns): the ND estimate on the node grid times 9 for
  the 3 x 3 blocks gives `nnz(L)` about 8.2e7 (about 1 GB at 12 bytes per entry) and 1.4e11 flops. With minimum degree's excess
  (a factor 2 to 3) and the 27-point node coupling: 1.6e8 to 3e8 entries (2 to 4 GB), 3e11 to 5e11 flops; at an up-looking rate
  of 0.5e9 to 1e9 flops per second, 5 to 15 minutes. The envelope after RCM of the same matrix: from ADR 0274's scaling
  (`N = 3e4`: 4.4e7 entries, growing as `N^(5/3)`), about 2e9 entries (16 GB) and hours: not runnable here, so its estimate is
  computed (D6), not run.
- **Residual estimate.** A backward-stable `L D L^T` gives `|b - A x| <= c eps |A| |x|`; with `|x| <= |b| / lambda_min`:
  5-point 100 x 100, `|A| <= 8`, `lambda_min = 8 sin^2(pi/202)` = 1.93e-3, so `|r| / |b| <= c 4.1e3 eps` = `c 9e-13`; scaled from
  ADR 0274's measured 7.4e-16 to 1.3e-15 on 12^3 (`|A| / lambda_min` = 69) the expected value is about 1e-13 for a pseudo-random
  `b`. 7-point 30^3: `|A| / lambda_min` = 12 / (12 sin^2(pi/62)) = 390, expected about 1e-14.
- **Solid model** S (clamped Hex8 cube `cube(11)`, 4 752 unknowns): the difference of two backward-stable solves is about
  `kappa eps |u|`; `kappa` of a clamped elastic cube with 11 elements per edge about 1e3 to 1e4 (`h^-2` times the material
  ratio `(lambda + 2 mu) / mu` of 3.5): about 1e-12, limit 1e-10.

## Design

### D1. Ordering: approximate minimum degree on the quotient graph (Amestoy, Davis, Duff 1996, from memory)

The graph of the symmetric pattern (both triangles, diagonal excluded) is held as a quotient graph: each uneliminated
*variable* `i` has a list `A_i` of adjacent variables and a list `E_i` of adjacent *elements*; each element `e` (an eliminated
pivot) has the list `L_e` of the variables of its clique. Variables carry a weight `nv_i` (the size of the supervariable it
represents). One step:

1. Take a variable `p` of least approximate degree `d_p` (degree lists by bucket; in a bucket the most recently inserted first;
   all deterministic).
2. Form `L_p = (A_p U the union of L_e for e in E_p) \ {p}` over principal variables; `p` becomes an element; every `e` in
   `E_p` is absorbed into `p` (its clique is contained in `L_p`). The eliminated weight `k` grows by `nv_p`.
3. For each `i` in `L_p` and each live element `e != p` of `E_i`: `w(e) = |L_e \ L_p|` (weighted), computed by starting from
   `|L_e|` at the first visit and subtracting `nv_i` for every `i` in `L_p` that lists `e`.
4. For each `i` in `L_p`: `E_i <- {p} U {e in E_i live, e != p, w(e) > 0}` (an element with `w(e) = 0` lies inside `L_p` and is
   absorbed: aggressive absorption); `A_i <- A_i \ (L_p U {p})` without non-principal variables; the external part
   `x_i = |A_i|_w + sum_{e in E_i \ p} w(e)`. If `x_i = 0` the variable is adjacent to `p`'s clique only and is eliminated with
   `p` (mass elimination: `nv_p += nv_i`, `k += nv_i`).
5. Supervariables: the remaining variables of `L_p` are hashed by `(sum A_i + sum E_i) mod n`; within one hash value they are
   compared in increasing index order; `j` equal to `i` in both lists (as sets) is merged into `i`: `nv_i += nv_j`, `nv_j = 0`,
   `j` points to `i`.
6. The approximate external degree of each remaining `i` in `L_p`: `d_i = min(x_i + |L_p|_w - nv_i, n - k - nv_i)`, inserted
   into its bucket. (ADD 1996 also bound by `d_i^old + |L_p \ i|`; the bound is not needed for correctness and is left out.)

The order lists each eliminated principal variable at its step, followed by the variables merged into it and those mass-
eliminated with it (each recursively in merge order). Dense-row detection of ADD 1996 is not implemented (finite element
matrices have none). A variable of degree 0 is simply taken first.

### D2. Elimination tree and symbolic factorisation (Liu 1986, 1990, from memory)

For the permuted matrix `C = P A P^T`, the parent of `j` is the least `k > j` with `l_kj != 0`. It is computed row by row:
for each stored `c_ki`, `i < k`, follow the ancestors of `i` (with a path-compressed `ancestor` array) up to a node without
ancestor or with ancestor `k`, set the parent of that node to `k`. The structure of row `k` of `L` is the set of nodes reached
from the `i` with `c_ki != 0` by climbing the tree until `k` (the *row subtree*), which gives the column counts in
`O(nnz(L))` by marking. The column pointers of `L` follow; `nnz(L)` (strict lower) is known before any numeric work (used by
H0 for the memory check).

### D3. Numeric up-looking L D L^T (Davis 2005, derived)

Row `k` of `L` solves `L_{0:k,0:k} D y = c_{0:k,k}`: scatter the stored entries `c_ik`, `i < k`, into a dense work vector `Y`;
take the row-subtree nodes in topological order (each climb pushed on a stack, the stack emptied in reverse, as in D2); for
each such `i`: `y = Y_i`, `Y_i = 0`; for every computed entry `l_ri` of column `i`: `Y_r -= l_ri y`; `l_ki = y / d_i`,
`d_k -= l_ki y`, and `l_ki` is appended to column `i`. `d_k` starts as `c_kk`. Every operation is sequential in a fixed order: the
bits of the factor and of a solve do not depend on the number of threads (c9). **Pivot test** (as ADR 0274 D4): with
`t = sum |l_ki y_i|` and `w` the number of entries of row `k`, `theta_k = (w + 1) eps (|c_kk| + t)`; `PivotRule::Positive`
refuses `d_k <= theta_k`, `PivotRule::NonZero` refuses `|d_k| <= theta_k`; a non-finite `d_k` is refused. The refusal is
`SkylineError::Pivot { row, position, pivot, threshold }` with `row` the **original** equation index (c8) and `position` the
elimination position. The input is validated by the envelope solver's `validate` (square, finite, symmetric within the
tolerance, made `pub(crate)`). The inertia is the count of negative `d_k` (Sylvester's law of inertia: `C = L D L^T` is
congruent to `D`).

**Solve.** `y = P b`; `L z = y` by columns (`z_r -= l_rj z_j`); `z_j /= d_j`; `L^T x = z` by columns in reverse
(`x_j -= sum_r l_rj x_r`, the sum in stored order); `x = P^T` back. `solve_many` applies `solve` to each right-hand side.

### D4. Public API (`crates/rcfd-linalg/src/sparse_chol.rs`)

- `pub const PENDING: Option<&str> = None` (landed).
- `pub enum SparseOrdering { MinimumDegree (default), Natural, Given(Vec<usize>) }` (`Given`: `order[i]` = original index at
  position `i`, checked as a permutation).
- `pub struct SparseOptions { ordering, pivots: PivotRule, symmetry_tolerance: f64 }`, default `MinimumDegree`, `Positive`,
  1e-12 (as `SkylineOptions`).
- `pub fn minimum_degree_order(a: &CsrMatrix) -> Vec<usize>` (D1), `pub fn elimination_tree(a, order) -> Vec<Option<usize>>`,
  `pub fn factor_count(a, order) -> usize` (strict lower `nnz(L)`, D2).
- `pub struct SparseLdlt` with `factor(a)`, `factor_with(a, &SparseOptions) -> Result<_, SkylineError>`, `n()`, `order()`,
  `pivots()`, `parent()`, `factor_entries()` (strict lower `nnz(L)`), `column(j) -> (&[usize], &[f64])` (rows in elimination
  order, values), `inertia() -> (negative, positive)`, `storage_bytes()` (values, row indices, pointers, `D`, order and parent,
  8 bytes each), `solve`, `solve_into`, `solve_many`.

### D5. Use by the solves (c12; defaults unchanged, c11)

- `rcfd_linalg::eigen::Factorisation { Envelope (default), Sparse }`, a field `factorisation` of `SubspaceOptions` and of
  `BucklingEigenOptions` (default `Envelope`). A private `enum Factor { Envelope(SkylineLdlt), Sparse(SparseLdlt) }` with
  `solve`, `pivots`, `order`, `inertia` replaces the direct calls in the four places of finding 1; with `Envelope` the same
  `SkylineLdlt::factor_with` is called with the same options in the same order, so every default bit is kept. With `Sparse` the
  options' `SkylineOrdering` is not used (the ordering is minimum degree), and the pivot rule is the one the place uses.
- `rcfd_fem::solve::StaticSolver::SparseDirect`: the sparse factor with minimum degree, the same pivot checks as `Direct`
  (refusal mapped to `FemError::SingularStiffness` with node and component; then `d_i <= 1e-10 a_ii` refused the same way),
  solver name `"sparse-ldlt"`. Refused on the transformation path (`FemError::InvalidValue`, key `solver`), by the cyclic solve
  (already) and with refinement (already).
- `ModalOptions::factorisation` and `BucklingOptions::factorisation` (default `Envelope`) with builder methods, passed into the
  eigensolver options at the sites of finding 3.

### D6. The envelope's estimate for the heavy record (c10)

From the matrix only, without factorising: `E = envelope_size(K, rcm)` (ADR 0274 D1), memory `8 (E + n) + 8 (3 n + 1)` bytes,
work `W = sum_i w_i (w_i + 1) / 2` multiply-adds with `w_i` the envelope width of row `i` (each entry of row `i` is one dot
product of at most `w_i` terms; an upper bound), time `W / 4.6e9` s (ADR 0274's measured rate).

### D7. Files

`crates/rcfd-linalg/src/sparse_chol.rs` (filled), `crates/rcfd-linalg/src/skyline.rs` (two `fn` made `pub(crate)`),
`crates/rcfd-linalg/src/eigen.rs` (the `Factorisation` choice), `crates/rcfd-fem/src/{solve.rs, modal.rs, buckling.rs, mpc.rs,
geometric.rs}` (the choice and its refusal), tests `crates/rcfd-linalg/tests/sa_sparse_chol.rs`,
`crates/rcfd-fem/tests/sa_sparse_solve.rs`, `crates/rcfd-fem/tests/sa_sparse_bits.rs`, and the retirement copy in
`crates/rcfd-fem/tests/sa_scaffold.rs`. `docs/reference-data.md` gains the cited sources. No file of `rcfd-fv` or `rcfd-api`.

## Checks (limits fixed now)

Release profile, each run once: `cargo test --release -p rcfd-linalg --test sa_sparse_chol -- --test-threads=1 --nocapture`
and `cargo test --release -p rcfd-fem --test sa_sparse_solve --test sa_sparse_bits -- --test-threads=1 --nocapture`; the heavy
records `--ignored` by name.

**Bit record procedure (c11).** `sa_sparse_bits.rs` uses only the API of main 46d78b64. Before any library code it is run once
in the worktree at main 46d78b64 plus this ADR and the file itself; it prints one FNV-1a 64 digest of the `to_bits()` of each
quantity per case; the printed digests become constants and the file is committed before the library code. After the change it
runs unchanged and compares. Cases (default options throughout): **B1** `solve_static` of S (displacements, reactions);
**B2** `modal_analysis` of S, 6 modes (eigenvalues, frequencies, modes, iterations, shift); **B3** `buckling::factors` of S under
`Prestress::uniform` with `sigma_xx = -1e8`, 2 factors (factors, modes, iterations); **B4** `rcfd_linalg::eigen::lowest_eigenpairs`
of the 5-point 30 x 30 Laplacian with `M = I`, 6 pairs (values, vectors); **B5** `count_below` of the same at `s = 0.05`;
**B6** `lowest_buckling_factors` of `K` = 5-point 20 x 20 Laplacian, `K_sigma = -I`, 3 factors.

Matrices: the 5-point Dirichlet Laplacian on `m x m` (diagonal 4, neighbours -1), the 7-point one on `m^3` (diagonal 6), the
tridiagonal (2, -1); right-hand side `b_i = 2 u_i - 1` with `u_i` the LCG `s <- 6364136223846793005 s + 1442695040888963407`,
`u = (s >> 11) / 2^53`, seed `0xF431_0447`.

| Id | Clause | Check | Limit | Estimate / basis |
| --- | --- | --- | --- | --- |
| A1 | c2 | 5-point 100 x 100, default options: `|b - A x| / |b|` | `< 1e-12` | about 1e-13 (arithmetic) |
| A2 | c3 | 7-point 30^3, default options: `|b - A x| / |b|` | `< 1e-12` | about 1e-14 |
| A3 | c4 | 7-point 30^3: `factor_entries()` of the default factor, `envelope_size()` of `SkylineLdlt::factor` (RCM); both printed | `nnz(L) <= 0.5 |Env|` | about 0.34 (ND estimate x 3) |
| A3r | c4 (record) | 5-point 100 x 100: both counts printed | record | |
| A4 | D2 | 5-point 20 x 20 with `SparseOrdering::Natural`: `factor_entries() = envelope_size(A, None)` = 7 790 (the band fills, `sum_i min(i, 20)`); `factor_count` = `factor_entries()` on A1's matrix | equal, equal | closed form |
| A5 | c7 | tridiagonal (2, -1), n = 200, `PivotRule::NonZero`, default ordering: negative pivots of `K - s I` at `s` the midpoints of `(lambda_10, lambda_11)`, `(lambda_100, lambda_101)`, `(lambda_190, lambda_191)`, `lambda_j = 4 sin^2(j pi / 402)`; expected count `#{j : lambda_j < s}` computed in the test | equal (10, 100, 190) | Sylvester |
| A6a | c8 | 5-point 20 x 20 with `a_137,137 = -1`, default options: `Err(Pivot { row: 137, .. })`, pivot negative, the message names "row 137" and "not positive definite" | as stated | every earlier pivot is of an SPD principal submatrix |
| A6b | c8 | the matrix of A5 at its first shift with `PivotRule::Positive`: `Err(Pivot { row, .. })` with `row < 200` and `pivot <= threshold`; with `NonZero` accepted | as stated | |
| A7 | c9 | 7-point 20^3: factor and solve of `b` inside rayon pools of 1 and of 8 threads: `order`, `parent`, every value of `L` and `D`, and `x` | bit for bit | D3 sequential |
| A8 | D4 | `solve_many` of three right-hand sides equals three `solve`s; the solve of A1's matrix agrees with `SkylineLdlt` within `1e-10` relative (max norm) | bit for bit; `<= 1e-10` | |
| A9 | D3 | refusals: a 2 x 3 matrix (`NotSquare`), a NaN at (3, 4) (`NonFinite { row: 3, col: 4 }`), `a_12 = -1, a_21 = -2` (`Unsymmetric { row: 1, col: 2 }`), `Given` of a repeated index (`BadOrdering`) | as stated | |
| S1 | c5, c12 | model S (`cube(11)`, 4 752 unknowns): `solve_static` with `SparseDirect` and with `Direct`: `max |u_s - u_e| / max |u_e|`; the stats name `"sparse-ldlt"`; relative residual of the sparse solve | `<= 1e-10`; equal; `<= 1e-12` | about 1e-12 |
| S2 | c6, c12 | S, `modal_analysis`, 6 modes, `factorisation` `Sparse` and `Envelope`: `max_j |f_s - f_e| / f_e` | `<= 1e-9` | residual 1e-10, eigenvalue error of order its square |
| S3 | c12 | S, `buckling::factors` with B3's prestress, 2 factors, `Sparse` and `Envelope`: `max_j |lambda_s - lambda_e| / lambda_e` | `<= 1e-9` | as S2 |
| S4 | c9 | S with `SparseDirect`, `threads(1)` and `threads(8)`: displacements and reactions | bit for bit | assembly bit-identical (ADR 0275), D3 |
| S5 | c8 | the hinge mechanism of `sb_static.rs` E5 with `SparseDirect`: `FemError::SingularStiffness` | as stated | |
| S6 | finding 2 | `SparseDirect` on a model with a rigid set (transformation path): `FemError::InvalidValue` with key `solver`; with `refinement(1)`: refused | as stated | |
| B1-B6 | c11 | the bit record procedure | digests equal to main 46d78b64 | |
| X1' | finding 5 | `x1_modules_and_pending_after_landed`: the X1 table with `sparse_chol` `None` | as stated | |
| H0 | c10 | (heavy, `#[ignore]`) `cube(46)` reduced stiffness: `n`, `nnz(L)` by `factor_count` with the minimum degree order, the predicted factor bytes, the time of the ordering; the envelope estimate of D6 (entries, bytes, work, time) | record | |
| H1 | c10 | (heavy, `#[ignore]`) `cube(46)` solved with `SparseDirect`: wall time of the solve, `factor_entries()`, `storage_bytes()`, relative residual | completes; residual `<= 1e-10` | 5 to 15 min, 2 to 4 GB |

H1 is run only if H0's predicted factor bytes plus 2 GB (stiffness, ordering, work arrays) leave at least 4 GB of the free memory
measured just before (`Get-CimInstance Win32_OperatingSystem`); otherwise it is recorded as not run with the estimate.

### Negative controls

| Id | Control | Must hold |
| --- | --- | --- |
| N1 | A3's counts with `SparseOrdering::Natural` on 30^3 | `nnz(L) > 0.5 |Env|` (the check can fail) |
| N2 | S2: the sparse-path frequencies are not all bit-equal to the envelope's | at least one differs (the sparse factor ran) |
| N3 | A5: the count of `K - s I` at the first shift with the order reversed (`Given`) | the same count (inertia does not depend on the order) |

### Planned mutations

On a `git archive` copy of the final tree in `w49i-work/f431-scratch` with its own target directory; each alone, by exact text
replacement; the named test files run once; the file restored.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `minimum_degree_order` returns `0..n` | A3 |
| MU2 | numeric update `Y_r -= l_ri y` becomes `Y_r += l_ri y` | A1, A2 |
| MU3 | the backward substitution `L^T x = z` skipped | A1 |
| MU4 | `inertia` counts `d > 0` as negative | A5 |
| MU5 | `Positive` tests `|d| <= theta` | A6a |
| MU6 | the refusal reports the elimination position as `row` | A6a |
| MU7 | the `SparseDirect` arm calls the envelope solve | S1 (name) |
| MU8 | `factor_shifted` builds the envelope factor for `Sparse` | N2 |
| MU9 | `Factorisation::default()` is `Sparse` | B2 |
| MU10 | `modal.rs` does not pass `factorisation` | N2 |
| MU11 | the transformation-path refusal removed | S6 |
| MU12 | the supervariable merge adds `nv_j` to `nv_i` twice | A1 or A3 or a panic in A1 to A3 (any) |

## Decision rule

**Done** if every check meets its limit on its registered run (H1 included), every negative control holds and every mutation is
caught by a check named for it on the first run; **partial** if a check misses or is not run (H1 not run for memory counts), a
mutation survives or is caught only by another check, or a check is met only after an amendment made after its run; **todo** if
the module does not build or a default bit changes and cannot be restored without changing a default.

## Limits (not covered)

- No supernodes and no parallel factorisation: the factor is sequential (deterministic) and its rate is that of an indexed
  update; beyond about 1e6 unknowns it is slow (ADR 0334 "what could make it miss").
- No nested dissection, no dense-row handling, no postorder of the tree; the approximate degree leaves out ADD 1996's
  `d_old + |L_p \ i|` bound (D1).
- Symmetric matrices only, no pivoting for indefinite matrices (the `NonZero` rule refuses a tiny pivot; it is a count, not a
  solver of indefinite systems).
- The transformation path, the cyclic static solve, the harmonic and transient solves, the Newton driver and the case file
  (`rcfd-api`) keep the envelope; the case-file key for the choice is a follow-up.
- The heavy record's memory is the factor's arrays (`storage_bytes`) and the predicted bytes, not the process peak.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The sparse factor reuses `SkylineError` and `PivotRule` (one refusal type for both direct factors, so `EigenError` keeps its
   `source` type). Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. The retired X1 row: per the standing decision on retired scaffold tests (ADR 0291 pattern), `x1_modules_and_pending` becomes
   an `#[ignore]` record and `x1_modules_and_pending_after_landed` is its copy with `sparse_chol` `None`; the body of the old
   test is not changed. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
3. The default stays the envelope everywhere (rule 3); whether `SparseDirect` should become the default above some size is left
   open (it would change default results).
