# ADR 0503: F-529 registration: nested dissection and a supernodal L D L^T for the sparse direct solver, with the heavy record of 3e5 unknowns

Status: **REGISTERED 2026-11-14, written before any code of F-529 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-529 was written, compiled or run in Rust: the files under "What was read" were read;
the only arithmetic done beforehand is disclosed below (short Python probes in `w49i-work/f529-scratch`). Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-529 (deps F-431), structural solver, advanced tier; the follow-up proposed by
[ADR 0447](0447-f431-registration-sparse-ldlt-minimum-degree.md). Number 0503 given by the lead. Branch `f529-work` from
98e767f5, worktree `w49i-work/f529`, scratch `w49i-work/f529-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical (the new behaviour is reached only through new functions, a new ordering variant, a new factor type and a new
static solver variant); no edit of `baseline_hashes.rs`, of the hash tables or of any golden file; no existing test edited; no
new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally be
asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open
questions at the end.

## What F-529 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. A nested-dissection ordering and a supernodal numeric factorisation in rcfd_linalg::sparse_chol: on the 7-point 30^3 grid the factor entries at most 0.6 of minimum degree's (both recorded) and the residual below 1e-12 relative; the clamped Hex8 cube of 304 842 unknowns is solved with the relative residual below 1e-10 in less than 8 GB, time and memory recorded; a refusal names an equation whose elimination position differs from its index; the static solve reports which factor it built; factor and solution bit-identical for 1 and 8 threads; the default solver's results keep their bits.

Clauses: **c1** limits registered before the run (this document); **c2** a nested-dissection ordering in
`rcfd_linalg::sparse_chol`; **c3** a supernodal numeric factorisation there; **c4** 7-point 30^3: factor entries of nested
dissection at most 0.6 of minimum degree's, both recorded; **c5** 7-point 30^3: relative residual below 1e-12; **c6** the
clamped Hex8 cube of 304 842 unknowns solved with relative residual below 1e-10; **c7** in less than 8 GB; **c8** its time
and memory recorded; **c9** a refusal names an equation whose elimination position differs from its index; **c10** the static
solve reports which factor it built; **c11** factor and solution bit-identical for 1 and 8 threads; **c12** the default
solver's results keep their bits.

## What was read (base 98e767f5, read only)

1. `AGENT_RULES.md`; ADR 0447 (all of it: the design D1 to D7, the checks, amendment 1 and addendum 1, in particular the
   measured 30^3 minimum-degree count nnz(L) = 5 659 458, the heavy cube's minimum-degree count 848 257 887 (13.6 GB predicted,
   H1 not run for memory), and the surviving mutations MU6 (row equal to position in A6a's matrix) and MU7 (the solver name
   taken from the option, not from the factor built)); the roadmap entries F-431 and F-529.
2. Code: `crates/rcfd-linalg/src/sparse_chol.rs` (whole), `crates/rcfd-linalg/src/skyline.rs` (the error type, its `Display`,
   `validate`, `check_permutation`, `PivotRule`), `crates/rcfd-linalg/Cargo.toml` (rayon is a dependency);
   `crates/rcfd-fem/src/solve.rs` (whole), `crates/rcfd-fem/src/mpc.rs` lines 1240 to 1392 (the refusal of `SparseDirect` on
   the transformation path, its `SolveReport` literal and `solver_name`), the `StaticSolver` uses in `cyclic.rs` (line 975:
   every solver but `Direct` refused); `crates/rcfd-api/src/solid_case.rs` lines 160 to 230 and 750 to 860 (the case-file
   solver schema `method = direct | cg`; the JSON report writes the named fields of `SolveReport`, so a new field does not
   change a written byte); `crates/rcfd-core/src/vec3.rs` (`Vec3 = [f64; 3]`).
3. Tests read for set-ups: `crates/rcfd-linalg/tests/sa_sparse_chol.rs` lines 1 to 130 and 278 to 290 (grids, the LCG
   right-hand side, the rayon pools), `crates/rcfd-fem/tests/sa_sparse_solve.rs` (whole: `cube(n, density)`, H0, H1),
   `crates/rcfd-fem/tests/sa_sparse_bits.rs` (whole: the FNV-1a digest and the bit-record procedure).
4. **Not read:** any sparse-direct code or documentation outside this repository. The methods are cited from memory and
   derived in D1 to D4 (marked *unverified* where a detail is from memory): A. George, "Nested dissection of a regular finite
   element mesh", SIAM J. Numer. Anal. 10 (1973) 345-363 (dissection by separators, the separator numbered last); A. George and
   J. W. H. Liu, "An automatic nested dissection algorithm for irregular finite element problems", SIAM J. Numer. Anal. 15
   (1978) 1053-1069 (separators from a level structure rooted at a pseudo-peripheral node), and their book *Computer Solution of
   Large Sparse Positive Definite Systems*, Prentice-Hall 1981 (the pseudo-peripheral node finder, chapter 4, unverified
   details); J. W. H. Liu, "The multifrontal method for sparse matrix solution: theory and practice", SIAM Review 34 (1992)
   82-109, and I. S. Duff and J. K. Reid, "The multifrontal solution of indefinite sparse symmetric linear equations", ACM
   Trans. Math. Softw. 9 (1983) 302-325 (frontal matrices, extend-add, the update matrix passed to the parent); J. W. H. Liu,
   E. G. Ng and B. W. Peyton, "On finding supernodes for sparse matrix computations", SIAM J. Matrix Anal. Appl. 14 (1993)
   242-252 (fundamental supernodes); T. A. Davis, *Direct Methods for Sparse Linear Systems*, SIAM 2006 (elimination tree,
   column counts, the sparse triangular solves). None of these is a premise of a check: the checks are residuals, exact counts
   from the pattern, closed-form spectra, bit comparisons and the existing solvers.

Findings that shape the design:

1. `SkylineError::Pivot { row, position, .. }` already carries both the original equation and the elimination position, and
   `SparseLdlt` fills them correctly; ADR 0447's A6a could not see a swap because minimum degree left equation 137 at position
   137. F-529's refusal check must use an ordering that moves the equation, with the position asserted different.
2. `SolveReport` (`pub` fields, not `#[non_exhaustive]`) is constructed only in `rcfd-fem` (`solve.rs`, `mpc.rs`); `rcfd-api`
   reads named fields into the JSON report. A new field changes no written byte and no default result. The factor actually
   built must come from the function that built it (ADR 0447 MU7 survived because the name came from the option).
3. `StaticSolver` is `#[non_exhaustive]`; the transformation path maps every solver but CG to the envelope (`_ =>`) and refuses
   `SparseDirect` and `PreconditionedCg` explicitly: a new variant must be refused there too. Cyclic and refinement refuse
   every solver but `Direct` already.
4. `SparseOrdering` is matched only in `sparse_chol::order_of`; a new variant changes no other code. `SparseOptions` literals
   in other files list their fields, so the new factor takes the existing `SparseOptions` unchanged (no new field).
5. The F-431 heavy cube by minimum degree needs 13.6 GB for the factor alone: the 8 GB clause needs a better ordering, not only
   a better numeric kernel.

## Arithmetic before this registration (disclosed)

All Python, `python -I`, in `w49i-work/f529-scratch` (`nd_estimate.py`, `nd_levels.py`, `nd_bfs.py`, `nd_bfs2.py` to
`nd_bfs5.py`, `nd_lib.py`, `nd_cube.py`). The counts are exact strict-lower `nnz(L)` from the elimination tree and the row
subtrees of the node graph (checked: the natural order of the 5-point 20 x 20 grid gives 7 619, ADR 0447 A4'). They were made
to choose the design and to see whether c4 and c7 can be met; they are disclosed design probes, not registered runs.

| Probe | 7-point 30^3 nnz(L) | ratio to minimum degree's 5 659 458 |
| --- | --- | --- |
| geometric ND (median plane of the longest axis, leaves of 16 in index order) | 6 198 351 (leaf 1: 6 173 851; 64: 6 437 716) | 1.095 |
| BFS level structure from a pseudo-peripheral node, median level, one-sided separator | 3 952 653 | 0.698 |
| BFS levels, the smallest level in a balance window `min(below, above) >= alpha N`, alpha 0.4 / 0.3 / 0.2 | 3 899 750 / 3 582 657 / 3 841 432 | 0.689 / 0.633 / 0.679 |
| BFS levels, the level of least ratio `|S_l| / (|below| |above|)` (no window), leaves in index order | 3 402 665 | 0.6012 |
| the same with a window 0.1 / 0.2 / 0.3 | 3 424 403 / 3 495 994 / 3 678 882 | 0.605 / 0.618 / 0.650 |
| the cost `|S_l| / min(below, above)` | 3 696 904 | 0.653 |
| **least ratio, leaves of at most 16 ordered by exact minimum degree** (the registered design, D1) | **3 360 697** (leaves 64: 3 380 433; 200: 3 383 875) | **0.5938** |

Reading: plane separators are the wrong separators for the 7-point stencil at this size (a diagonal level set of a cube is
about 0.75 of an axis plane, and the planes' boundary faces dominate the fill, George 1973); the least-ratio level set is the
principled choice (it is the separator of least size per pair separated); the registered design is predicted at 0.594, inside
the limit 0.6 by 1 percent. The Rust leaves use the existing approximate minimum degree, not the exact one of the probe, and the
tie-breaks may differ, so c4 can miss; the limit stays 0.6 (the acceptance's).

5-point 100 x 100 (record only): least-ratio BFS ND 182 159, geometric 260 376; minimum degree 185 035 (ADR 0447).

**Heavy cube** (`cube(46)`: 46 x 47 x 47 free nodes, 27-point node coupling, three unknowns per node, 304 842 unknowns), node
graph counts scaled to the unknowns (`9 c + (2, 1, 0)` per node):

| Probe | nnz(L) | values (8 B) | multiply-adds `sum c_j^2 / 2` |
| --- | --- | --- | --- |
| geometric ND (median plane of the longest axis, leaves of 5 nodes) | 420 635 271 | 3.37 GB | 8.9e11 |
| least-ratio BFS ND, minimum-degree leaves | 504 360 309 | 4.03 GB | 1.3e12 |

On the 27-point stencil a BFS level is a shell of a cube (Chebyshev distance), so the coordinate planes are the better
separators there; the static solve has the node coordinates and uses the geometric variant (D1b). **Memory estimate** of the
heavy solve (c7): factor values 3.4 GB, row indices about 0.1 GB (one list per supernode), the full and reduced stiffness
2 x 0.38 GB (23.6e6 entries, 16 B each), the permuted lower triangle and its transpose about 0.4 GB, the model and the
assembly about 0.5 GB, the live fronts and pending update matrices about 0.5 to 1 GB (the top front 6 486 x 6 486 packed is
0.17 GB): about 5.5 to 6.5 GB, under 8 GB with a margin of about 20 percent. **Time estimate:** 1.8e12 flops; at 1 to 2 Gflop/s
per thread on the large fronts with 16 threads and the small fronts sequential: 2 to 15 minutes.

**Residual estimates.** 30^3: `|A| / lambda_min` = 390 (ADR 0447), backward-stable factor: about 1e-14 (ADR 0447 measured
2.9e-15). Supernodal against up-looking with the same order: both backward stable, difference about `kappa eps` = 390 x 2.2e-16
= 1e-13. Model S (cube(11)): ADR 0447 S1 measured 2.7e-14 against the envelope solve and residual 1.9e-13. Heavy cube: `kappa`
grows as `h^-2`: (46/11)^2 = 17 times S's, residual about 1e-12 to 1e-11, limit 1e-10.

## Design

### D1. Nested dissection by level structures (graph only): `nested_dissection_order(a)`

On the symmetric pattern of `a` (both triangles, diagonal excluded). Positions are assigned from the end of each part, so the
dissection runs on an explicit work list (no recursion, any depth): a task is a sorted vertex set `S` and the end position
`e` of its block `[e - |S|, e)`.

1. **Leaf.** If `|S| <= 16` (`ND_LEAF`), the vertices of `S` are ordered by `minimum_degree_order` (ADR 0447 D1) of the
   pattern induced on `S`, and take the block in that order.
2. **Components.** A breadth-first search from the least vertex of `S` (neighbours in increasing index order). If it does not
   reach all of `S`, `S` splits into the reached set `P1` and the rest `P2` with an empty separator.
3. **Pseudo-peripheral root** (George and Liu 1981, from memory, unverified details): from the least vertex, repeat: build the
   level structure; take as the next root the least-index vertex of the last level; stop when the eccentricity does not
   increase. The last level structure kept is that of the root of greatest eccentricity found: levels `L_0, ..., L_h`.
4. **Separator.** If `h < 2`, `S` is a leaf (step 1, whatever its size). Otherwise the separator is the level `L_l`,
   `1 <= l <= h - 1`, of least `|L_l| / (|below| |above|)` with `below` = the vertices of levels `< l` and `above` = those of
   levels `> l` (ties: the least `||below| - |above||`, then the least `l`); `P1` = below, `P2` = above. A level set separates
   because every edge joins equal or adjacent levels.
5. **Numbering.** `P1` takes the first `|P1|` positions of the block, `P2` the next, the separator the last `|S_sep|` in index
   order; `P1` and `P2` become tasks. Each task is strictly smaller than its parent, so the dissection ends.

`SparseOrdering::NestedDissection` (new variant) selects it in `SparseLdlt::factor_with` and in the supernodal factor.

### D1b. Nested dissection by coordinates: `nested_dissection_order_with_points(a, points)`

The same work list, leaves and components (steps 1, 2, 5), with step 3 and 4 replaced: the axis of largest extent of the
points of `S` (ties: the lower axis); the key of a vertex is its coordinate on that axis; `v` = the key of the vertex at index
`|S| / 2` of `S` sorted by (key, index); `A = {key < v}`, or `{key <= v}` if that is empty; if `A = S` the set is a leaf.
`B = S \ A`; the separator is the smaller (ties: the second) of `{a in A adjacent to B}` and `{b in B adjacent to A}`, with
`P1 = A`, `P2 = B` minus the separator, or `P1 = A` minus the separator, `P2 = B`. Every point must be finite (asserted). The
static solve gives the coordinates of each free equation's node, so the three unknowns of a node share a key and stay on one
side.

### D2. Supernodes and their row structures

On `C = P A P^T` (the existing `permuted_lower`, rows of the strict lower triangle) the existing elimination tree and column
counts (ADR 0447 D2), and the row counts `w_k` (the number of nodes of the row subtree of `k`, counted in the same pass).
**Fundamental supernodes** (Liu, Ng and Peyton 1993, from memory): column `j + 1` joins the supernode of `j` exactly when
`parent(j) = j + 1`, `count(j) = count(j + 1) + 1` and `j` is the only child of `j + 1`. No amalgamation and no width cap: the
stored entries are exactly `nnz(L)`, so `factor_entries()` equals `factor_count(a, order)`. A supernode `s` has the columns
`[f_s, l_s)` (`n_s = l_s - f_s`) and the rows `R_s` below them, the union of the rows `> l_s - 1` of the columns of `C` in `s`
and of the children's `R_c` above `l_s - 1`, sorted; `|R_s| = count(l_s - 1)` (asserted in debug builds). Its parent is the
supernode of `parent(l_s - 1)`.

### D3. Multifrontal numeric factorisation (Duff and Reid 1983, Liu 1992; derived)

Supernodes are processed **sequentially in increasing order** (a child has a smaller index than its parent). For supernode
`s` the front `F` has order `m = n_s + |R_s|` and holds its lower triangle packed by columns (column `c` holds rows `c..m`,
offset `c (2m - c + 1) / 2`); local index of a position `r`: `r - f_s` for `r < l_s`, else `n_s` + its index in `R_s`.

1. **Assembly.** `F = 0`; the diagonal `c_jj` and the column entries `c_ij` (`i > j`) of `C` for the columns of `s` are added;
   then the update matrix of each child, in increasing child order, is added entry by entry (extend-add) and freed.
2. **Partial factorisation** of the first `n_s` columns in panels of `NB = 32` columns. In a panel, for each column `k` in
   order: `d_k = F_kk`; the pivot test (below); `w_ik = F_ik` (kept) and `l_ik = w_ik / d_k` for every row `i > k`; for the
   later columns `j` of the panel and every row `i >= j`: `F_ij -= l_ik w_jk`. After the panel, every later column `j >= p1`
   of the front and every row `i >= j`: `F_ij -= T(i, j)` with `T(i, j) = sum_k l_ik w_jk` over the panel's columns `k` in a
   fixed order: four partial sums over `k` mod 4 for the leading multiple of 4, combined as `(s0 + s1) + (s2 + s3)`, then the
   remaining terms added in order.
3. **Hand-over.** The first `n_s` columns (trapezoid) are the factor's columns of `s`; the rest of `F` is the packed update
   matrix of order `|R_s|`, kept for the parent.

**Pivot test** of the supernodal factor: `theta_k = (w_k + 1) eps (|c_kk| + |c_kk - d_k|)`; `PivotRule::Positive` refuses
`d_k <= theta_k`, `NonZero` refuses `|d_k| <= theta_k`, a non-finite `d_k` is refused. (`|c_kk - d_k|` equals the up-looking
test's `sum |l_ki y_i|` in exact arithmetic when every earlier pivot is positive; the supernodal factor does not form the
individual terms.) The refusal is `SkylineError::Pivot { row: order[k], position: k, pivot, threshold }`: `row` the
**original** equation (c9). Positions are reached in increasing order, so the first refused pivot is reported. The input is
validated by `validate` (square, finite, symmetric within the tolerance), as both existing factors.

**Determinism (c11).** The orderings, the symbolic phase, the assembly, the extend-add, the panel factorisation and the solves
are sequential in a fixed order. The only parallel step is the update of the later columns after a panel: the columns are
disjoint slices of the front, each column is computed by one task, and every entry's value `F_ij - T(i, j)` has the fixed
summation order of step 2 whatever the split. The update is split over the rayon pool when `(m - p1) (m - p1 + 1) / 2 x NB >=
2^20` multiply-adds, else it runs in a loop; both compute the same expression. So the factor and every solve are bit-identical
for any number of threads. `NB`, the split rule and the summation order do not depend on the thread count.

**Solve.** `z = P b`; forward by columns in supernode order (`z_r -= l_rj z_j`); `z_j /= d_j`; backward by columns in reverse
(`z_j -= sum_r l_rj z_r` in stored order); `x = P^T z`. `solve_many` applies `solve` to each right-hand side. The inertia is the
count of negative `d_k` (Sylvester).

### D4. Public API (`crates/rcfd-linalg/src/sparse_chol.rs`)

- `SparseOrdering::NestedDissection` (D1); `pub const ND_LEAF: usize = 16`; `pub fn nested_dissection_order(a) -> Vec<usize>`;
  `pub fn nested_dissection_order_with_points(a, points: &[[f64; 3]]) -> Vec<usize>` (D1b).
- `pub struct SupernodalLdlt` with `factor(a)` (nested dissection, positive pivots, tolerance 1e-12), `factor_with(a,
  &SparseOptions)`, `n()`, `order()`, `pivots()`, `inertia()`, `factor_entries()` (strict lower), `supernodes()`,
  `max_front()`, `split_fronts()` (fronts whose later-column update was split over the pool), `peak_front_bytes()` (the most
  bytes of fronts and pending update matrices alive at once), `storage_bytes()` (values 8 B, row indices 8 B, pointers, `D`,
  order), `column(j) -> (&[usize], &[f64])`, `solve`, `solve_into`, `solve_many`.
- `SparseLdlt`, `minimum_degree_order`, `SparseOptions`'s default and every existing function keep their code paths
  (`order_of` gains one arm).

### D5. The static solve (c10)

- `rcfd_fem::solve::StaticSolver::SupernodalDirect`: the supernodal factor after `nested_dissection_order_with_points` of the
  reduced stiffness with the coordinates of each free equation's node; the pivot checks of `Direct` (a refusal mapped to
  `FemError::SingularStiffness` with node and component of `row`; then `d_i <= 1e-10 a_ii` refused the same way); solver name
  `"sparse-ldlt-supernodal"` (`SUPERNODAL_NAME`). Refused on the transformation path (`FemError::InvalidValue`, key `solver`),
  by the cyclic solve and with refinement (already, finding 3).
- `rcfd_fem::solve::DirectFactor { Envelope, SparseMinimumDegree, SupernodalNestedDissection }` (`#[non_exhaustive]`) and the
  field `SolveReport::factor: Option<DirectFactor>`: the factor the solve built, returned by the function that built it
  (`direct`, `direct_refined`, `sparse_direct`, `supernodal_direct` return it beside the solution); `None` for CG and for a
  model without free equations. The transformation path sets `Some(Envelope)` for its direct solve.

### D6. Files

`crates/rcfd-linalg/src/sparse_chol.rs`; `crates/rcfd-fem/src/solve.rs`; `crates/rcfd-fem/src/mpc.rs` (the refusal and the
`factor` field of its report literal); tests `crates/rcfd-linalg/tests/sa_supernodal.rs`, `crates/rcfd-fem/tests/
sa_supernodal_solve.rs`, `crates/rcfd-fem/tests/sa_nd_bits.rs`. Not touched: `rotor.rs`, shell and laminate files,
`docs/structural-solver.md` (other items edit them), any `rcfd-api` file, any golden.

## Checks (limits fixed now)

Release profile, each run once: `cargo test --release -p rcfd-linalg --test sa_supernodal -- --test-threads=1 --nocapture`,
`cargo test --release -p rcfd-fem --test sa_supernodal_solve --test sa_nd_bits -- --test-threads=1 --nocapture`; the heavy
record by name with `--ignored`, alone.

Matrices as ADR 0447: the 5-point Dirichlet Laplacian on `m x m` (diagonal 4), the 7-point on `m^3` (diagonal 6), the
tridiagonal (2, -1); right-hand side `b_i = 2 u_i - 1` from the LCG of ADR 0447 with the seed `0xF529_0503`. Model S: the
clamped Hex8 cube `cube(11, 7850)` of `sa_sparse_solve.rs` (4 752 unknowns); the heavy cube `cube(46, 0)` (304 842 unknowns).
Every aggregated error is asserted finite before it is compared.

**Bit record procedure (c12).** `sa_nd_bits.rs` uses only the API of base 98e767f5. Before any library code it is run once in
the worktree at base plus this ADR and the file itself; it prints one FNV-1a 64 digest (ADR 0291) per quantity; the printed
digests become its constants and the file is committed before the library code. After the change it runs unchanged and
compares. Cases: **D1** `solve_static` of S with `Direct` (displacements, reactions, the report's solver name bytes, iterations
and residual bits); **D2** the same with `SparseDirect`; **D3** `SparseLdlt::factor` of the 7-point 20^3 grid (order, parent,
every value of `L` by `column`, `D`) and its solve of `b`; **D4** `minimum_degree_order` of the 5-point 100 x 100 grid;
**D5** `SparseLdlt` with `Natural` and with `Given` (reversed) on the 5-point 30 x 30 grid: `D` and the solve. The existing
`sa_sparse_bits.rs` (B1 to B6 of ADR 0447) and `baseline_hashes` (rcfd-api) are run unchanged.

| Id | Clause | Check | Limit | Estimate / basis |
| --- | --- | --- | --- | --- |
| T1 | c2, c4 | 7-point 30^3: `factor_count(a, nested_dissection_order(a))` over `factor_count(a, minimum_degree_order(a))`; both printed | `<= 0.6` | 0.594 (probe) |
| T1r | c4 (record) | 30^3 with `nested_dissection_order_with_points` (grid points); 5-point 100 x 100: graph ND, geometric ND, minimum degree | record | 6.2e6; 182 159, 260 376, 185 035 |
| T2 | c3, c5 | 7-point 30^3, `SupernodalLdlt::factor`: `|b - A x| / |b|` | `< 1e-12` | about 1e-14 |
| T3 | D2, D3 | 30^3: `factor_entries()` = `factor_count(a, order)` = `SparseLdlt` (same order by `Given`) `factor_entries()`; `max |x_s - x_u| / max |x_u|` against that up-looking solve | equal, equal; `<= 1e-11` | exact; about 1e-13 |
| T4 | c3 | 5-point 100 x 100, `SupernodalLdlt::factor`: relative residual | `< 1e-12` | about 1e-13 |
| T5 | c9 | 5-point 20 x 20 with `a_137,137 = -1`: (a) `SupernodalLdlt::factor`: `Err(Pivot { row: 137, position: p })` with `p` the position of 137 in `nested_dissection_order(a)`, `p != 137`, pivot `< 0`, the message contains "row 137" and "elimination position {p}"; (b) `Given` of the reversed order: `row` 137, `position` 262; (c) `SparseLdlt` with `Given` of the ND order: `row` 137, `position` `p` | as stated | every earlier pivot is of an SPD principal submatrix |
| T6 | D3 | tridiagonal (2, -1), n = 200, `NonZero`, `SupernodalLdlt` with ND: negative pivots of `K - s I` at the shifts of ADR 0447 A5' (midpoints of `(lambda_10, lambda_11)`, `(lambda_95, lambda_96)`, `(lambda_190, lambda_191)`) | 10, 95, 190 | Sylvester |
| T7 | c11 | 7-point 20^3, `SupernodalLdlt::factor` and the solve of `b` inside rayon pools of 1 and of 8 threads: order, every `column(j)` (rows and values), `D`, `x`; and `split_fronts() > 0` | bit for bit; `> 0` | D3 determinism |
| T8 | D3 | refusals of `SupernodalLdlt::factor_with`: 2 x 3 (`NotSquare`), NaN at (3, 4) (`NonFinite { row: 3, col: 4 }`), `a_12 = -1, a_21 = -2` (`Unsymmetric { row: 1, col: 2 }`), `Given` with a repeated index (`BadOrdering`) | as stated | `validate` |
| T9 | D1, D1b | both orderings are permutations, and the supernodal solve has relative residual `< 1e-12`, on: two disconnected 5-point grids (10 x 10 and 7 x 7 in one matrix), a diagonal matrix of order 50 (isolated vertices), order 1, a dense SPD 30 x 30 (`30 I + 1 1^T`), the tridiagonal (2, -1) of order 500; and `nested_dissection_order_with_points` with all points equal (a leaf) | as stated | |
| T10 | D3 | `solve_many` of three right-hand sides equals three `solve`s | bit for bit | |
| F1 | c10 | S with `SupernodalDirect`: `report.factor == Some(SupernodalNestedDissection)`, solver `"sparse-ldlt-supernodal"`; `Direct`: `Some(Envelope)`; `SparseDirect`: `Some(SparseMinimumDegree)`; `ConjugateGradient`: `None`; `max |u_s - u_e| / max |u_e|` against `Direct` and the residual of the supernodal solve | as stated; `<= 1e-10`; `<= 1e-12` | 2.7e-14, 1.9e-13 (ADR 0447 S1) |
| F2 | c11 | S with `SupernodalDirect` and `threads(1)` inside a rayon pool of 1 thread, `threads(8)` inside a pool of 8: displacements and reactions | bit for bit | |
| F3 | D5 | `SupernodalDirect`: with a rigid set (transformation path) `FemError::InvalidValue { key: "solver" }`; with `refinement(1)` refused (`FemError::Solver`); the hinge mechanism of `sa_sparse_solve.rs` S5 `FemError::SingularStiffness` | as stated | |
| H1 | c6, c7, c8 | (heavy, `#[ignore]`) `cube(46, 0)` with `SupernodalDirect`: relative residual; the peak working set of the test process; wall time of `solve_static_report`; and, after the solve, `nnz(L)` by `factor_count` with the same order (symbolic only) | residual `<= 1e-10`; peak working set `< 8.0e9` bytes; the rest recorded | about 1e-12; 5.5 to 6.5 GB; 2 to 15 min |
| B | c12 | `sa_nd_bits.rs` D1 to D5 digests equal to base; `sa_sparse_bits.rs` passes; `cargo test -p rcfd-api --release --test baseline_hashes` passes | equal; pass; pass | |

**H1's memory.** The test binary is built first; the test is started alone, as its own process, by a PowerShell script
(`f529-scratch/heavy.ps1`) that polls the process's `PeakWorkingSet64` every second until exit and records the largest value
seen and the wall time. It is started only when the free physical memory (`Win32_OperatingSystem.FreePhysicalMemory`) is at
least 10 GB, waiting otherwise; if that never happens on the day it is recorded as not run with the estimate. "8 GB" is read as
8.0e9 bytes (the stricter reading).

### Negative controls

| Id | Control | Must hold |
| --- | --- | --- |
| N1 | T1's ratio with the `Natural` order of 30^3 | `> 0.6` (the check can fail) |
| N2 | T3: the supernodal `L` values on 30^3 are not all bit-equal to the up-looking factor's with the same order | at least one differs (the supernodal arithmetic ran) |
| N3 | T5's matrix with `SparseOrdering::Natural`: the refusal has `row = position = 137` | holds (so T5's position test can fail) |
| N4 | F1: the `SupernodalDirect` displacements are not bit-equal to `Direct`'s | holds |

### Planned mutations

On a `git archive` copy of the final tree in `w49i-work/f529-scratch/mut` with its own target directory; each alone, by exact
text replacement; the named test files run once; the file restored.

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | `nested_dissection_order` returns `minimum_degree_order(a)` | T1 |
| M2 | the separator cost is `|L_l|` alone (no balance) | T1 |
| M3 | the later-column update adds `T(i, j)` instead of subtracting | T2 |
| M4 | the extend-add of the children is skipped | T2 |
| M5 | the supernodal refusal reports the position as `row` | T5 |
| M6 | the panel width is `16 * rayon::current_num_threads()` (the arithmetic depends on the pool) | T7 |
| M7 | the `SupernodalDirect` arm calls the up-looking `sparse_direct` | F1 |
| M8 | the transformation-path refusal of `SupernodalDirect` removed | F3 |
| M9 | `order_of` maps `MinimumDegree` to `nested_dissection_order` | B (D2 or D3) |
| M10 | the supernodal `inertia` counts `d > 0` | T6 |
| M11 | the fundamental-supernode test drops the single-child condition | T2 or T3 (or a panic in them) |
| M12 | the supernodal backward substitution is skipped | T2 |

## Decision rule

**Done** if every check meets its limit on its registered run (H1 included), every negative control holds and every mutation
is caught by a check named for it on the first run; **partial** if a check misses or is not run, a mutation survives or is
caught only by another check, or a check is met only after an amendment made after its run; **todo** if the module does not
build or a default bit changes and cannot be restored without changing a default.

## Limits (not covered)

- No relaxed amalgamation of supernodes, no width cap, no tree-level parallelism (subtrees run one after another; only the
  later-column update of large fronts is parallel), no blocked dense kernels beyond the panel of 32 and the four-way sums.
- The graph-only ordering is level-structure dissection, not a multilevel partitioner; its separators can be poor on
  irregular graphs. The geometric variant needs coordinates and suits meshes; the separator is one-sided, not a minimum vertex
  cover.
- The supernodal factor is not used by the modal, buckling or harmonic solves, the transformation path, the cyclic solve, the
  refinement of ADR 0397 or the Newton driver (follow-up); the case file has no key for it (open question 1).
- Symmetric matrices only, no pivoting for indefinite matrices (the `NonZero` rule refuses a tiny pivot).
- H1's memory is the process peak working set polled once a second; a peak shorter than a second between polls could be
  missed (the factor and the stiffness, which dominate, live for minutes).

## Open questions

1. **Case-file key.** Not added (it would change the schema golden). Options: (a) `solver.method = "sparse_nd"` selecting
   `SupernodalDirect` (and optionally `"sparse_md"` for `SparseDirect`); (b) a separate `solver.ordering` key; (c) none.
   Recommendation: (a) with the single value `"sparse_nd"`, after the maintainer approves the golden change.
2. Whether `SupernodalDirect` should become the default above some size changes default results: not taken (rule 6), open.
3. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): the supernodal factor reuses
   `SparseOptions`, `PivotRule` and `SkylineError` (one refusal type for the three direct factors); the factor kind is reported
   by a new `SolveReport` field rather than by the solver name.

## Addendum 1: results (2026-11-14)

Registered runs at 93f3f582, release profile, each run once: `sa_supernodal.rs` (log `f529-scratch/run_sa_supernodal.log`),
`sa_supernodal_solve.rs` and `sa_nd_bits.rs` (`run_fem.log`), H1 alone through `f529-scratch/heavy.ps1` (`run_h1.log`,
`run_h1_memory.log`; free memory at the start 10.26e9 bytes). The bit record `sa_nd_bits.rs` was run at 2b01d96a (base 98e767f5 plus
this ADR) plus the file (`bits_base_run.log`), its constants filled in and committed at 3927ed17 before any library line; d46f5891
changed only the clippy form of one closure in it (`map_or(usize::MAX, |v| v)` to `unwrap_or(usize::MAX)`, the same values).
No limit was changed and no amendment was needed.

Disclosed: before the registered runs one throwaway development probe (not committed, copy
`f529-scratch/dev_f529_tmp.rs`) compared the supernodal and up-looking solves on four small grids (5-point 5^2 and 13^2,
7-point 6^3 and 9^3, diagonals 4.5 and 6.5) with the three orderings: differences at most 2.7e-15, equal counts. No
registered matrix was used and nothing was changed after it.

### Checks

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| T1 (c2, c4) | `<= 0.6` | nnz(L) nested dissection 3 360 870, minimum degree 5 659 458: 0.5939 (probe 0.5938); ordering 0.04 s | yes |
| T1r | record | 30^3 geometric ND 6 180 043 (1.09 of minimum degree); 5-point 100 x 100: level-structure ND 172 416, geometric 257 050, minimum degree 185 035 | record |
| T2 (c3, c5) | `< 1e-12` | 1.185e-15; factor 0.29 s (up-looking with the same order 0.73 s), 18 629 supernodes, largest front 1 059, 51 fronts split | yes |
| T3 | equal, equal; `<= 1e-11` | 3 360 870 = 3 360 870 = 3 360 870; 1.048e-14 | yes |
| T4 | `< 1e-12` | 2.761e-15 | yes |
| T5 (c9) | as stated | (a) row 137 at elimination position 270, pivot -2.31, message "row 137 (elimination position 270)"; (b) row 137, position 262; (c) up-looking row 137, position 270 | yes |
| T6 | 10, 95, 190 | 10, 95, 190 | yes |
| T7 (c11) | bit for bit; `> 0` | order, L (591 826 values), D, x equal; 13 fronts split | yes |
| T8 | as stated | NotSquare, NonFinite (3, 4), Unsymmetric (1, 2), BadOrdering | yes |
| T9 | permutations; `< 1e-12` | all 15 cases; largest residual 8.4e-14 (tridiagonal 500) | yes |
| T10 | bit for bit | equal | yes |
| F1 (c10) | as stated; `<= 1e-10`; `<= 1e-12` | factors Supernodal / Envelope / SparseMinimumDegree / None, name "sparse-ldlt-supernodal"; 7.147e-15; 1.168e-13 (0.45 s against the envelope's 0.30 s) | yes |
| F2 (c11) | bit for bit | equal | yes |
| F3 | as stated | transformation path `InvalidValue(solver)`; refinement refused; hinge `SingularStiffness` at node 13 component 0 | yes |
| H1 (c6, c7, c8) | residual `<= 1e-10`; peak working set `< 8.0e9` B | residual 2.224e-12; peak working set 5.044e9 B; `solve_static_report` 80.8 s (process 87.3 s with the symbolic recount); nnz(L) 421 295 142 (3.37 GB of values; probe 420 635 271), nnz(K) 23 648 904 | yes |
| B (c12) | equal; pass; pass | 17 digests equal to base; see "Other checks run" | yes |
| N1 | `> 0.6` | 23 516 129 / 5 659 458 = 4.155 | holds |
| N2 | some differ | 3 224 040 of 3 360 870 values differ | holds |
| N3 | row = position = 137 | 137, 137 | holds |
| N4 | not bit-equal | not equal | holds |

H1 against ADR 0447: minimum degree needed 848 257 887 entries (13.6 GB predicted, not run); nested dissection by the node
coordinates halves the fill (0.497) and the supernodal factor solves the cube in 81 s at 5.0 GB peak, inside the estimate of
5.5 to 6.5 GB and 2 to 15 minutes.

### Mutations (copy `f529-scratch/mut/tree` of 93f3f582, own target `target-f529-mut`, `f529-scratch/mutate.py`, each alone; log `mutations.log`)

| Id | Caught by | Registered | Result |
| --- | --- | --- | --- |
| M1 | T1 (ratio 1), T5 (position equals row) | T1 | caught |
| M2 | T1 (ratio 2.36), T7 (no front large enough to split) | T1 | caught |
| M3 | T2, T4, T5, T6, T9 | T2 | caught |
| M4 | T2 and others | T2 | caught |
| M5 | T5 (row) | T5 | caught |
| M6 | T7 (values of column 774) | T7 | caught |
| M7 | F1 (`factor`) | F1 | caught |
| M8 | F3 (the transformation path solved) | F3 | caught |
| M9 | B (D2, D3 digests) | B | caught |
| M10 | T6 | T6 | caught |
| M11 | nothing | T2 or T3 | **survived** |
| M12 | T2, T4, T9 | T2 | caught |

M11 is an equivalent mutation, diagnosed after the run: when `parent(j) = j + 1` and `count(j) = count(j + 1) + 1`, the
structure of column `j` is `{j + 1}` plus that of `j + 1`, so merging `j` into the supernode of `j + 1` stores no extra entry
even when `j + 1` has other children. Their update matrices have rows `>= j + 1` only, so the extend-add (D3 step 1) never
touches column `j`. The single-child condition belongs to the definition of a fundamental supernode, not to the correctness of
the multifrontal method, so no check can tell the two partitions apart by value. The registration should have planned a
mutation that changes the result (for example one that also drops `count(j) = count(j + 1) + 1`).

### Other checks run

`cargo test --release -p rcfd-linalg` (all binaries, `run_linalg_all.log`) and `cargo test --release -p rcfd-fem` (160 binaries,
726 tests, `run_fem_all.log`): all pass, 0 failed; `sa_sparse_bits.rs` (B1 to B6 of ADR 0447) among them.
`cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored (`run_baseline.log`).
`cargo test -p rcfd-cli --release --test docs_site`: 2 passed. `cargo fmt -p rcfd-linalg --check`, `cargo fmt -p rcfd-fem
--check`: clean. `cargo clippy --release -p rcfd-linalg -p rcfd-fem --all-targets -- -D warnings`: clean. Shared solver files
changed: `crates/rcfd-fem/src/solve.rs` (the new variant, `DirectFactor`, the `factor` field, the four direct arms and
`supernodal_direct`) and `crates/rcfd-fem/src/mpc.rs` (one refusal, one field of the report literal).

### Not run

Nothing registered was left out. The modal, buckling and harmonic solves do not use the supernodal factor (a limit of the
registration, not a check).

### Decision by the registered rule

**Partial**: every check met its limit on its registered run (H1 included) and every negative control held, but mutation M11
survived (equivalent, diagnosed above). Every clause of the acceptance was met.

### Remaining questions

1. M11. Options: (a) accept it as equivalent and the item as done; (b) a follow-up registration with a mutation that changes
   values (for example dropping the count condition as well). Recommendation: (a); the maintainer decides.
2. Case-file key (open question 1 of the registration). Recommendation: `solver.method = "sparse_nd"` selecting
   `SupernodalDirect`, after the maintainer approves the schema golden change.
3. Default above a size (open question 2): changes default results; not taken, open.
4. The supernodal factor in the modal, buckling and harmonic solves and on the transformation path. Recommendation: a
   follow-up item (below); the heavy cube shows it is about 1.5 times slower than the envelope on 4 752 unknowns but the only
   direct factor that fits at 3e5.

### For the lead: roadmap entries

Proposed status of F-529: **partial** (done if the maintainer accepts M11 as equivalent).

> Structural solver, advanced tier. Follow-up of F-431 (ADR 0447). ADR 0503, 2026-11-14: rcfd_linalg::sparse_chol::nested_dissection_order (level structures from a pseudo-peripheral vertex, the level of least |S| / (|below| |above|) as the separator, minimum-degree leaves of at most 16) and nested_dissection_order_with_points (the median plane of the longest axis, the smaller one-sided boundary as the separator), SparseOrdering::NestedDissection, and SupernodalLdlt (fundamental supernodes, multifrontal extend-add, partial factorisation in panels of 32 with the later-column update split over the rayon pool in a fixed summation order); rcfd_fem::solve::StaticSolver::SupernodalDirect ("sparse-ldlt-supernodal", nested dissection by the node coordinates, refused on the transformation path and with refinement) and SolveReport::factor (DirectFactor, returned by the function that built the factor). 7-point 30^3: nnz(L) 3 360 870 against minimum degree's 5 659 458 (0.594, limit 0.6), relative residual 1.2e-15 (limit 1e-12). Clamped Hex8 cube of 304 842 unknowns: solved in 81 s with relative residual 2.2e-12 (limit 1e-10) at a peak working set of 5.04 GB (limit 8 GB), nnz(L) 421 295 142 (3.37 GB; minimum degree needed 848 257 887). A refusal names equation 137 at elimination position 270; the static report names the factor built (supernodal, envelope, minimum degree, none for CG); factor and solution bit-identical for 1 and 8 threads (grid and solid); 17 digests of the default and existing sparse paths equal to base; baseline_hashes and every rcfd-linalg and rcfd-fem test pass. Partial by the registered rule only because mutation M11 (the single-child condition of the supernode test dropped) survived; diagnosed as equivalent (the merged supernode stores no extra entry and the extend-add never reaches the merged column). Done is the maintainer's call.

Follow-up item, title "Supernodal L D L^T in the modal, buckling and harmonic solves and on the transformation path", deps
F-529, acceptance:

> Every limit not stated here is registered in the ADR before the run. The supernodal factor after nested dissection is selectable in the subspace and buckling eigensolvers, the direct harmonic solve and the transformation path: on the clamped Hex8 cube of 4 752 unknowns the modal frequencies and buckling factors agree with the envelope's within 1e-9 and the harmonic response within 1e-8; a model with a rigid set solved on the transformation path equals the envelope solve within 1e-10; a modal analysis of a clamped cube of about 1e5 unknowns completes with time and memory recorded; results bit-identical for 1 and 8 threads; the default solver's results keep their bits.
