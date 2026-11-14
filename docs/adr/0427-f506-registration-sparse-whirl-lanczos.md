# ADR 0427: F-506 registration: a sparse whirl solver for large rotating parts (shift-invert skew Lanczos on the state form of ADR 0411 in the `M*` inner product, one sparse factor of `K_eff` per speed, deflated restarts for multiple frequencies, opt-in)

Status: **REGISTERED 2026-11-14, written before any code of F-506 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-506 was written, compiled or run: the code listed under "What was read" was read and
the arithmetic below was done by hand (no script, no build, no run). The only numbers below that came from a run are those of the
registered run of ADR 0411 as its addendum prints them (disclosed under "Arithmetic"). Results go in the addendum at the end;
nothing above the addendum is changed after a run.

Parent: [ADR 0411](0411-f499-registration-coriolis-gyroscopic-whirl-modes.md) (F-499: the Coriolis matrix `G = 2 Omega C`, the
state form `M* x_t + G* x = 0`, `M* = blockdiag(M, K_eff)`, `G* = [[G, K_eff], [-K_eff, 0]]`, the Hermitian `H = i A`, `A =
L^-1 G* L^-T`, the dense solver `solve_dense` with its cap `MAX_WHIRL_UNKNOWNS = 1500`, the whirl number `kappa`; its remaining
question 2 (b) and its follow-up item "Sparse whirl solver for large rotating parts"). Not changed: [ADR
0416](0416-f505-registration-gyroscopic-stabilisation.md) (the state solver above the loss of stiffness, `whirl_state`), [ADR
0420](0420-f511-registration-sector-whirl-stability.md) (the sector state solver), [ADR
0424](0424-f510-registration-odd-index-instability.md) (`with_nodal_springs`). Roadmap item F-506 (dep F-499). Number 0427 given
by the lead. Branch `f506-sparse-whirl` (base `17addb94`), worktree `w49i-work/f506`, scratch `w49i-work/f506-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, the hash tables or a golden file; no new dependency, no unsafe code; clean-room.

## What F-506 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The lowest whirl modes of (K_eff - omega^2 M + i omega G)
> phi = 0 by a shift-invert Lanczos on the Hermitian form of ADR 0411 with sparse factors of K_eff and M, without the dense cap;
> on the cube and the disc sector of ADR 0411 the frequencies equal the dense solver's within 1e-9 relative and the whirl numbers
> within 1e-8; a part of at least 20 000 reduced unknowns is solved in under two minutes in release.

Clauses: (c1) every limit not stated there registered here before the run; (c2) the lowest whirl modes by a shift-invert Lanczos
on the Hermitian form of ADR 0411 with sparse factors of `K_eff` and `M`, without the dense cap; (c3) on the cube CB of ADR 0411
the frequencies equal the dense solver's within 1e-9 relative and the whirl numbers within 1e-8; (c4) the same on the disc sector
DS of ADR 0411; (c5) a part of at least 20 000 reduced unknowns solved in under two minutes in release.

Readings (registered now).

- "Within 1e-9 relative" (frequencies): `|omega_sparse / omega_dense - 1| <= 1e-9`, mode by mode in ascending order over the
  returned modes. "The whirl numbers within 1e-8": `|kappa_sparse - kappa_dense| <= 1e-8` (absolute; `kappa` lies in `[-1, 1]`),
  mode by mode, over every returned mode whose relative gap to its neighbours in the dense spectrum (`frequencies_all`) exceeds
  1e-6. The whirl number of a mode is not defined by the problem inside a cluster of (near-)equal frequencies (any combination is
  a mode), so such modes are excluded; the number excluded is printed, and if it is not zero the clause is reported as met on
  the separated modes only.
- "The disc sector" (c4): the harmonic indices `k = 0 .. 3` of DS at both comparison speeds of ADR 0411 (`0.4` and `0.8
  omega_ref`), with the Coriolis matrix.
- "Solved in under two minutes" (c5): the wall time of `RotatingPart::new` (the static solve of the pre-stress and the assembly
  of the four matrices) plus `RotatingPart::whirl_modes` at one speed with the Coriolis matrix, the model built before the clock
  starts; release profile; measured once, on the shared machine as it is (other agents build at the same time; disclosed).
- "Sparse factors of `K_eff` and `M`" (c2): see D3; `K_eff` is factored and used for every operator application; `M` is factored
  as the positive-definiteness test of the inner product `M*` (the role of the dense Cholesky factor of `M`), and the Lanczos runs
  in the `M*` inner product, which is the Hermitian form of ADR 0411 written in the coordinates `X = L^-T y` (D2), so the factor
  of `M` is never applied.

## What was read (branch base 17addb94, read only)

- `AGENT_RULES.md`; the brief; ADR 0411 (all, with the addendum); ADR 0416, 0420, 0424 (heads and designs, to see what they
  touch).
- `crates/rcfd-fem/src/rotating/whirl.rs` (all: `solve_dense`, `skew_tridiagonal`, `tql`, `quadratic_residual`, `whirl_number`,
  `inertia`, `check_size`, `RotatingPart::whirl_at`, `RotatingSector::whirl_at`), `rotating/whirl/state.rs` (the `use` list and
  the function heads: it calls `check_size`, `inertia`, `dense_of`, not `whirl_at`), `state/sector.rs`, `state/springs.rs`
  (heads), `crates/rcfd-fem/src/rotating.rs` (lines 1 to 330, 940 to 1010: `RotatingOptions`, `Path`, `RotatingPart`,
  `RotatingSector::new`).
- `crates/rcfd-linalg/src/skyline.rs` (lines 1 to 300: `SkylineLdlt::factor_with`, `PivotRule`, `SkylineOptions`, the
  symmetry validation), `crates/rcfd-linalg/src/eigen.rs` (lines 600 to 720: `count_below`, `factor_shifted`,
  `lowest_eigenpairs` head), `crates/rcfd-linalg/src/matrix.rs` (the function list: `transpose`, `matvec`, `diag`),
  `crates/rcfd-fem/src/modal.rs` (heads). There is no Lanczos solver in the repository (grep `lanczos` over `crates`: none);
  the modal solver is a subspace iteration on the skyline factor (ADR 0283). The Lanczos of D3 is written here; it reuses the
  skyline factor and the QL routine `tql` of ADR 0411.
- Tests: `crates/rcfd-fem/tests/si_rotating_whirl.rs` (lines 1 to 140 and 480 to 720: `Checks`, CB, DS, `full_of`,
  `ds_options`, `omega_ref`, `union`, S1 to S4, Q2).
- grep of `rotating::`, `RotatingOptions`, `whirl` outside `crates/rcfd-fem`: only `crates/rcfd-post/src/rotordyn.rs` and its
  test (`RotatingOptions` is `#[non_exhaustive]`, built there through `new`), so a new field of `RotatingOptions` does not break
  it (checked by `cargo check -p rcfd-post --tests`, R3).

## Design

Notation of ADR 0411: `K = K_eff(Omega)` reduced, symmetric positive definite (tested by the inertia of ADR 0403 D3 first, as
the dense path does); `M` reduced, symmetric positive definite; `G = 2 Omega C` reduced, skew; `n` the reduced unknowns.

### D1. The operator to iterate with: the inverse of the state pencil (first principles)

ADR 0411 D2: with `x = [u_t; u] = X e^{s t}`, `s M* X = -G* X`, and the free whirl `s = i omega`. `G*` is invertible when `K`
is (`det G* = det(K)^2`, block elimination). So

`W X = -(1/s) X = (i / omega) X`, `W = G*^-1 M*`.

The lowest frequencies are the largest `|1/omega|`: the spectrum of `W` is the shift-invert transform at the shift 0 (Ericsson
and Ruhe 1980, "The spectral transformation Lanczos method for the numerical solution of large sparse generalized symmetric
eigenvalue problems", Math. Comp. 35; cited from memory; only the idea of iterating with the inverse of the shifted pencil is
taken, the operator here is derived). `G*^-1 = [[0, -K^-1], [K^-1, K^-1 G K^-1]]` (check: `G* G*^-1 = [[K K^-1, -G K^-1 + K
K^-1 G K^-1], [0, K K^-1]] = I`). With `X = [a; b]`, `M* X = [M a; K b]`, and `K^-1 (K b) = b` exactly:

`W [a; b] = [-b; K^-1 (M a + G b)]`:

**one solve with `K` per application**, one product with `M` and one with `G`; `M` is never solved with.

### D2. Why this is the Hermitian form of ADR 0411 (the equivalence)

`M*` is symmetric positive definite, so `<x, y>_* = x^T M* y = a_x^T M a_y + b_x^T K b_y` is an inner product. `W` is skew in it:
`M* W = M* G*^-1 M*` and `(M* G*^-1 M*)^T = M* G*^-T M* = -M* G*^-1 M*` (`G*` skew). With `M* = L L^T` (`L = blockdiag(L_m,
L_k)`, ADR 0411) and `y = L^T X`: `L^T W L^-T = L^T G*^-1 L = A^-1`, `A = L^-1 G* L^-T` the real skew matrix of ADR 0411, and
`<X_1, X_2>_* = y_1^T y_2`. So a Lanczos process on `W` in the `M*` inner product is, vector by vector, the Lanczos process on
`A^-1` (equivalently on the Hermitian `(i A)^-1 = -i A^-1`) in the Euclidean product, carried in the coordinates `X = L^-T y`;
the factor `L` cancels and need not be applied. The positivity of `M*` (and so of `M`) is what makes `<., .>_*` an inner
product; it is tested by factoring `M` (D4).

### D3. The skew Lanczos process and its Ritz pairs (first principles; Ward and Gray 1978, ACM TOMS 4, for the skew tridiagonal form, cited from memory, derived here)

For a real operator `W` skew in `<., .>_*`, a Lanczos basis `q_1, q_2, ...` (`*`-orthonormal) has `<q_j, W q_j>_* = 0` and
`<q_{j-1}, W q_j>_* = -<W q_{j-1}, q_j>_* = -beta_{j-1}`, so

`W q_j = beta_j q_{j+1} - beta_{j-1} q_{j-1}`, `beta_j = ||W q_j + beta_{j-1} q_{j-1}||_*`,

and `W Q_m = Q_m T_m + beta_m q_{m+1} e_m^T`, `T_m` skew tridiagonal with zero diagonal, `(T_m)_{j+1, j} = beta_j`: the form of
ADR 0411 D2 step 1 (`e_j = beta_j`). As there, `S = D^-1 (i T_m) D`, `D = diag(i^j)` (`j` from 0), is real symmetric tridiagonal
with zero diagonal and off-diagonal `beta_j`, solved by `tql` of ADR 0411 (the same routine, eigenvectors `w`). From `S w = mu w`:
`T_m (D w) = -i mu (D w)`, so the Ritz value of `W` is `-i mu`; with `W X = (i/omega) X`, **`mu = -1/omega`: every negative `mu`
gives a frequency `omega = -1/mu > 0`** (the positive `mu` are the conjugate pairs) and the Ritz vector `X = Q_m D w`: its real
part `sum_j c_j q_j` with `c_j = w_j, 0, -w_j, 0` for `j = 0, 1, 2, 3 (mod 4)` and its imaginary part with `0, w_j, 0, -w_j`
(the bookkeeping of `solve_dense`). The mode is the displacement block: `phi = b` of `X` (ADR 0411 has `phi = L_k^-T y_bottom`,
which is the bottom block of `X = L^-T y`). Residual estimate: `||W X - (i/omega) X||_* = beta_m |w_{m-1}|` (`||X||_* = ||w|| =
1`); the pair is **converged** when `beta_m |w_{m-1}| <= TOL |mu|`, `TOL = 1e-12` (fixed; D6).

Each step: `v = W q_j` (D1); `v` is `*`-orthogonalised against the locked vectors `Z` (D5) and all of `Q_j` by classical
Gram-Schmidt applied twice (full reorthogonalisation; the three-term recurrence alone loses orthogonality once a Ritz pair
converges, Paige 1971, cited from memory); `beta_j = ||v||_*`. The coefficients of the reorthogonalisation are discarded (`T_m`
keeps the exact skew form, the coefficient on `q_{j-1}` being `-beta_{j-1}` in exact arithmetic). **Breakdown**: if `beta_j <=
1e-13 max_{i <= j} beta_i` the Krylov space is invariant to rounding; the run stops and its Ritz pairs are exact (`beta_m := 0`).

Start vector of run `r`: entries `u - 1/2` of a fixed linear congruential sequence (`state = state * 6364136223846793005 +
1442695040888963407`, `u = (state >> 11) / 2^53`, seed `0x5eed_0506 + r`), `*`-orthogonalised against `Z` twice and normalised.
Deterministic: the result does not depend on threads or on the run.

**A run** iterates until the `count` largest `-mu` (lowest `omega`) of `S` are converged (checked after steps `2 count + 2`,
then every 8 steps), or breakdown, or `MAX_STEPS = min(2 n - dim Z, 20 count + 100)` steps (then the converged ones among the
`count` lowest are kept and the run is marked short). It returns its converged Ritz pairs among the `count` lowest.

### D4. Factors

Per speed, after the inertia test of the dense path (`inertia(kr, mr)`, unchanged, which decides `LossOfStiffness` exactly as
before): `K` by `SkylineLdlt::factor_with` (reverse Cuthill-McKee, positive pivots, symmetry tolerance 1e-12), refused as
`"stiffness"` if it fails; `M` by the same, refused as `"mass"`; `G` as the exactly skew `G_s = Omega (C - C^T)` (sparse, by
`CsrMatrix::transpose`; the dense path also discards the symmetric part of `G`). The whirl number of a returned mode is computed
as the dense path does, `kappa = 2 r^T C s / (r^T M r + s^T M s)` with the reduced `C`, by sparse products.

### D5. Multiple frequencies: deflated restarts with locking

A single-vector Krylov space sees only one vector of a multiple eigenvalue (on a real eigenspace of `W` of dimension 4, the real
form of a double frequency, `W^2 = -theta^2 I`, so the Krylov space of `v` is `span{v, W v}`). Multiple frequencies occur here:
the full rotor with the Coriolis matrix off (each pair index `k` double), a sector pair index with it off, the cube at rest
(handled by the zero-speed shortcut). Remedy (fixed now): **runs with locking**. The converged pairs of a run are locked: the real
and imaginary parts of each `X` are `*`-orthonormalised (Gram-Schmidt twice) into `Z`; since `W` is `*`-normal the
`*`-orthogonal complement of the invariant span of `Z` is invariant, so the next run (started orthogonal to `Z` and kept
orthogonal to it at every step) iterates on the deflated operator and finds the missing copies. **Stopping rule**: after run `r
>= 1`, stop when the run locked no frequency below `omega_count,prev` (the `count`-th lowest locked frequency before that run,
`infinity` while fewer than `count` are locked) and at least `count` are locked; also stop when `dim Z = 2 n`. At most
`MAX_RUNS = 8` runs; otherwise `FemError::Solver` (`"sparse_whirl"`). The result: the `count` lowest locked frequencies with
their modes, ascending. So every result costs at least two runs (the second confirms the first).

Then as `solve_dense` does: normalisation `phi^H M phi = 1`, the entry of largest modulus real and positive (the first on a
tie); the residual `||(K - omega^2 M + i omega G) phi|| / (||K phi|| + omega^2 ||M phi|| + omega ||G phi||)` by sparse products.

### D6. Interfaces and selection (opt-in; defaults bit-identical)

- `pub enum WhirlSolver { Dense, Sparse, Auto }`, `#[default] Dense`. New field `RotatingOptions::whirl_solver` (default
  `Dense`) with the builder `whirl_solver(WhirlSolver)`. `Dense`: exactly the code of ADR 0411 (the refusal above the cap
  included). `Sparse`: D1 to D5 at every speed `> 0`. `Auto`: dense at `n <= MAX_WHIRL_UNKNOWNS`, sparse above. The zero-speed
  shortcut (the modes of `modes(0)`) and the inertia test come before the choice and are unchanged for all three.
- `RotatingPart::whirl_modes` and `RotatingSector::whirl_modes` (and `whirl_campbell`, through `whirl_at`) follow the option.
  The new branch is inserted after the inertia test of each `whirl_at`; the dense branch is the existing lines, unmoved in
  effect. `whirl_state` and everything of ADR 0416, 0420, 0424 never read the option.
- With the sparse solver `WhirlModes::frequencies_all` holds the frequencies of the returned modes only (it cannot hold all `n`);
  documented on the field.
- `pub fn gyroscopic_eigen_sparse(k: &CsrMatrix, m: &CsrMatrix, g: &CsrMatrix, count: usize) -> Result<SparseWhirl>` (the
  solver on given sparse matrices; `g` skew, its symmetric part discarded) and `gyroscopic_eigen_sparse_with(.., &LanczosOptions)`
  with `LanczosOptions { tolerance, max_runs }` (defaults `1e-12`, `8`; the negative control N1 uses `max_runs = 1`).
  `SparseWhirl { omegas, re, im, residuals, runs, steps }`. Refusals: unequal or non-square sizes, a non-finite entry
  (`"matrix"`), `count == 0` or `count > n` (`"modes"`), `K` not positive definite (`"stiffness"`), `M` not (`"mass"`).
- Code: a new file `crates/rcfd-fem/src/rotating/whirl/sparse.rs` (child of `whirl`, it reads `tql`, `dot` and friends);
  in `whirl.rs`: `mod sparse;`, a `pub use`, the branch in each `whirl_at`, the `frequencies_all` doc; in `rotating.rs`: the
  field, its default, the builder, the re-export. No other file of the library changes; `rcfd-linalg` is not touched.

## Set-up (fixed now)

Material, axis, CB and DS exactly as ADR 0411 (the test file copies them): CB the cube on springs, `modes = 3`, speeds `0.1, 0.3,
0.5, 0.7, 0.9 omega_0`; DS the `N = 6` Hex20 disc sector, `ds_options` (`modes = 4`, tolerance 1e-9, 400 iterations), `omega_ref`
the lowest rest sector frequency over `k = 0 .. 3`, speeds `0.4, 0.8 omega_ref`; DF the full rotor of DS (252 unknowns, below
the cap), both speeds (an extra, not a clause).

**MP (medium part, above the cap, fast):** the sector `hex_box([0.25, 0, -0.1], [0.75, alpha, 0.2], [4, 1, 2])` mapped to the
annulus and upgraded to Hex20 as `ds_mesh`, `alpha = 2 pi / N`, `N = 16`, `xmin` clamped, cyclic `ymin`/`ymax`; the full rotor
by `full_rotor`, `xmin` clamped (`full_of`). Per sector of the full rotor (arithmetic 1) 132 free unknowns: **2112** reduced
unknowns (above 1500). Options `RotatingOptions::new(10).modal(ModalOptions::new(10).tolerance(1e-9), 400 iterations).coriolis(true)`.
`omega_mp` = the lowest rest frequency of the full rotor (`whirl_modes(0)`); speed `0.4 omega_mp`.

**LP (large part, heavy record):** the same with `N = 152`: **20 064** reduced unknowns (arithmetic 1), same options, speed `0.4
omega_lp`, `omega_lp` the lowest rest frequency (`whirl_modes(0)`, the subspace modal analysis, outside the timed span).

## Arithmetic (done before the code, by hand; disclosed)

1. Unknowns of a Hex20 sector grid `(n_r, n_t, n_z)` in the full rotor (one `theta` face per sector): corner nodes `(n_r + 1) n_t
   (n_z + 1)`, mid-edge nodes on `r` edges `n_r n_t (n_z + 1)`, on `t` edges `(n_r + 1) n_t (n_z + 1)`, on `z` edges `(n_r + 1)
   n_t n_z`; the hub face `r` index 0 clamped removes `n_t (n_z + 1) + n_t (n_z + 1) + n_t n_z`. Check on DS `(2, 1, 1)`: `6 + 4 +
   6 + 3 = 19` nodes per sector, `5` clamped, `14 x 3 = 42` per sector, `x 6 = 252`: the measured count of ADR 0411 (S1, 252).
   MP/LP `(4, 1, 2)`: `15 + 12 + 15 + 10 = 52`, clamped `3 + 3 + 2 = 8`, `44 x 3 = 132` per sector; `N = 16`: 2112; `N = 152`:
   20 064 (>= 20 000). A count below 20 000 at the run is a defective set-up (amendment before the timed run).
2. Cost of LP. Reverse Cuthill-McKee on a ring of 152 slices of about 132 unknowns: level sets advance both ways round the ring,
   envelope width about `2 x 132` to `3 x 132`, so the envelope is `~20 064 x 400 = 8e6` entries (64 MB per factor; three factors
   at a time at most: the inertia test's, which is dropped, `K` and `M`). Factor `~n b^2 / 2 ~ 2e9` flops, about 1 to 3 s each.
   A Lanczos step: one solve `~4 x 8e6 = 3e7` flops, three `*`-products (`K` and `M`, `~5e6` stored entries each: `~2e7` flops),
   reorthogonalisation `4 j (2 n)` flops, `~1.6e7` at `j = 100`; `~0.1 s` per step at most; `count = 10` needs `~60 .. 100` steps
   per run, two runs: `~20 s`. `RotatingPart::new`: one static solve (direct, the same envelope) and five assemblies: seconds.
   Estimate for the timed span: `10 .. 40 s` against 120 s. Lanczos basis `<= 300` vectors of `2 n = 40 128` entries: `<= 96 MB`.
3. Accuracy. Each Ritz pair converges to `TOL = 1e-12` in `W` (relative to its `|mu|`); the frequency is a Rayleigh-type quantity
   of a normal operator, error `<= TOL |mu|` at first order and `~TOL^2 / gap` once separated: `<= 1e-12` relative against the
   dense solver's own error `~1e-13` (ADR 0411 Q2: 4e-13): estimate `<= 1e-11` against 1e-9. The vector error is `~TOL / gap_rel`;
   `kappa` is a quadratic form, its error at most twice the vector error times `|C|/|M| <= 1`; with `gap_rel >= 1e-3` (the cube:
   `omega_0 -/+ Omega` and the torsion at `sqrt 3 omega_0`; DS: S4 of ADR 0411 measured a split of 0.355 for the in-plane pair,
   the bending pairs split `~1e-2`) the estimate is `<= 2e-9` against 1e-8; a pair split below `1e-4` would bring it to `~2e-8`
   (a possible miss, diagnosed if it occurs).
4. The quadratic residual of a converged pair: `~TOL` relative times the ratio of norms (`~omega_max / omega` of the `K`
   solve), estimate `<= 1e-10`; limit `1e-9` as ADR 0411 Q1.

## Checks (limits fixed now)

| # | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| A1 | c1 | every limit of this table and of the controls fixed in this file, committed alone before any code | holds | the commit order |
| A2 | c2 | MP with `Dense` (the default) is refused with the key `"unknowns"`; with `Sparse` it returns 10 modes; neither the sparse path of `whirl_at` nor `gyroscopic_eigen_sparse` calls `check_size` or `dense_of` (reviewed, and the LP run itself, `n = 20 064`, forms no dense matrix: its peak would be `2n x 2n x 8 B = 12.9 GB`) | each | D6 |
| E1 | c3 | CB, Coriolis on, the five speeds, `Sparse` against `Dense`: `max |omega_s/omega_d - 1|` over the 3 modes | `<= 1e-9` | `<= 1e-11` (arithmetic 3) |
| E2 | c3 | the same: `max |kappa_s - kappa_d|` over the separated modes (reading); the count of excluded modes printed | `<= 1e-8` | `<= 2e-9` |
| E3 | c4 | DS, Coriolis on, `k = 0 .. 3`, both speeds: frequencies, `Sparse` against `Dense` (`4 x multiplicity` modes per index) | `<= 1e-9` | `<= 1e-11` |
| E4 | c4 | the same: whirl numbers over the separated modes | `<= 1e-8` | `<= 2e-9` |
| E5 | extra | DF (full rotor of DS, 252 unknowns, 4 modes), both speeds: frequencies and whirl numbers against `Dense` | `<= 1e-9`; `<= 1e-8` | as E3, E4 |
| E6 | D5 | Coriolis off (`coriolis(false)`, every frequency of a pair index double): DS index 1 and DF at `0.4 omega_ref`, frequencies `Sparse` against `Dense` | `<= 1e-9` | the deflated restarts find both copies |
| Q1 | D5 | the stored residual of every sparse mode (CB, DS, DF, MP, LP, with and without Coriolis) | `<= 1e-9` | `<= 1e-10` (arithmetic 4) |
| U1 | D6 | `gyroscopic_eigen_sparse` against `gyroscopic_eigen_dense` on random dense matrices converted to CSR (`K`, `M` positive definite, `G` skew; orders 1, 2, 5, 12, 30; all `n` frequencies for `n <= 12`, 8 for 30): frequencies | `<= 1e-9` relative | `~1e-13` |
| M1 | c2 | MP: `Auto` gives the bits of `Sparse` (every `omega`, `whirl`, `re`, `im`) | identical | D6 |
| M2 | c2 | MP with Coriolis: the 10 sparse frequencies against the lowest 10 of the union over `k = 0 .. 8` of the dense sector solutions (`RotatingSector` of the MP sector, `whirl_modes(k, .)`, `Dense`, 4 modes per index); valid if every index's highest returned frequency is at or above the union's 10th (else a defective set-up, printed) | `<= 1e-8` relative | `~1e-12` (ADR 0411 S1: 3e-13) |
| M3 | c2 | the same: whirl numbers of the separated modes against the matched sector modes | `<= 1e-6` | `~1e-9` (the identity of ADR 0411 D5) |
| M4 | D5 | MP Coriolis off: the 10 sparse frequencies against `sqrt(lambda)` of `modes(0.4 omega_mp)` (the subspace solution of F-396, every pair double) | `<= 1e-8` | `~1e-12` (ADR 0411 Q2) |
| T1 | c5 | LP: unknowns `>= 20 000` and the wall time of `new + whirl_modes(0.4 omega_lp)` with `Sparse` and Coriolis, release, measured once | `< 120 s` | `10 .. 40 s` (arithmetic 2) |
| T2 | c5 | LP: the stored residuals (Q1) and the 10 frequencies against the lowest 10 of the dense sector union over `k = 0 .. 76` (as M2, validity as M2) | `<= 1e-8` relative | as M2 |
| T3 | c5 | LP Coriolis off: against `modes(0.4 omega_lp)` (as M4) | `<= 1e-8` | as M4 |
| R1 | D6 | refusals of `gyroscopic_eigen_sparse`: wrong size (`"matrix"`), `count = 0` and `count = n + 1` (`"modes"`), indefinite `K` (`"stiffness"`), indefinite `M` (`"mass"`) | each refused with its key | D6 |
| R2 | rule 3 | `RotatingOptions::new(m).whirl_solver == WhirlSolver::Dense`; on CB at `0.5 omega_0` and DS `k = 1` at `0.4 omega_ref`, the default part and a part with `whirl_solver(Dense)` give the same bits; `Auto` below the cap gives the bits of `Dense` (CB, DS `k = 1`); the existing tests of `rcfd-fem` pass unchanged (release) | each | the default branch is the existing code |
| R3 | rule 3 | `cargo check -p rcfd-post --tests` | passes | D6 |

T1 is the heavy record: the LP test is `#[ignore]` (its references T2, T3 add a subspace modal solve and 77 dense sector indices
to the timed span's work) and run once by me; MP is the fast test of the same code path (rule 7).

Negative controls (expected to fail the property):

| # | Control | Expected |
|---|---|---|
| N1 | `gyroscopic_eigen_sparse_with(max_runs = 1)` on a diagonal `K = diag(1, 1, 4, 4, 9, 9, ...)`, `M = I`, `G = 0`, `n = 20`, `count = 6`: one run cannot see the second copies | the second frequency deviates from the dense one by `> 0.1` relative (1 against 2) |
| N2 | E1 with the sparse solver at Coriolis off against the dense one at Coriolis on (CB, `0.5 omega_0`) | `> 1e-2` |

Planned mutations (on a `git archive` copy in `f506-scratch`, each run once against the new test file's fast tests; what catches
each is recorded):

1. `W` with `M a - G b` (the sign of `G`): E2, E4 (the whirl numbers change sign; the frequency set of CB is symmetric).
2. No restart (`max_runs` forced to 1 in the default): E6, M4.
3. Reorthogonalisation dropped (only the three-term recurrence): E3, E5, U1 (spurious copies).
4. `TOL = 1e-6`: E2, E4 (vector accuracy).
5. `omega` from the positive `mu` (the conjugate) with the same vector: Q1, E2.
6. The phase matrix `D` dropped (`X = Q w` real): Q1, E1.
7. `Auto` with the comparison reversed (sparse below the cap): R2.
8. Locked vectors not kept orthogonal in later runs (the projection on `Z` removed): E6, M4 (copies found twice).

## Decision rule

- **done**: A1 to R3 met and N1, N2 as expected, every planned mutation caught by a registered check.
- **partial**: E1 to E4 and T1 met (the clauses c2 to c5) with one other check missed and its cause diagnosed (a `*_known_miss`
  record), or a mutation not caught, or a check met only after an amendment made after its run, or E2/E4 met on the separated
  modes only with modes excluded.
- **todo**: E1, E3 or T1 missed, or a returned frequency non-finite.

## Limits (what is not covered)

Speeds above the loss of stiffness (`K_eff` indefinite: `M*` is not an inner product; the state solver of ADR 0416 stays dense);
a shift other than 0 (frequencies far above the lowest); the sector state stability (ADR 0420) and the stationary frequencies of
large sectors (they call the dense solvers); threads (the Lanczos is sequential; the factor is the sequential skyline); the
envelope factor's memory grows with the bandwidth, so a part of 20 000 unknowns with a wide envelope (a compact 3D block) costs
far more than the ring of LP; no case-file or CLI switch for the option.

## Amendment 1 (2026-11-14, after the registered run of the fast tests, before any run of the corrected control)

The registered run of the four fast tests (log `f506-scratch/run1.log`) met every check of `e1_e2_n2_r2_cube`,
`e3_e4_e5_e6_r2_sector` and `a2_m1_m2_m3_m4_medium_part`, and U1 and R1 of `u1_r1_n1_algebraic`, but **N1 was not as
expected**: one run on `K = diag(1, 1, 4, 4, ..., 100, 100)`, `M = I`, `G = 0` returned `1, 1, 2, 2, 3, 3` (deviation 0 against
the dense solver, expected `> 0.1`). Diagnosis (a development print of `beta_j` on a scratch copy, not a registered run): the
premise of N1 was wrong. In exact arithmetic the Krylov space has dimension 20 (two per distinct frequency) and breaks down at
step 20; in floating point the components of the later Lanczos vectors along the high frequencies (eigenvalues `1/j` of `W` down
to 0.1) are small, their rounding is large relative to them, and at step 20 `beta_20 = 9.6e-4 max beta` (not `~1e-16`): the run
did not break down, continued into the complement and found the second copies by rounding, up to step 40 (the whole space). So a
single run is not blind to multiple frequencies when the spectrum is spread; it is blind when the copies are exact and the space
is reached without amplification.

The record: `u1_r1_n1_algebraic` becomes the ignored record `u1_r1_n1_algebraic_known_miss` (unchanged, its limit intact). The
corrected control (registered now, run once):

| # | Control | Expected |
|---|---|---|
| N1b | `gyroscopic_eigen_sparse_with(max_runs = 1)` on `K = M = I`, `G = 0`, `n = 20`, `count = 2` (one frequency 1 of multiplicity 20: `W [a; b] = [-b; a]` exactly, `W^2 = -I`, the Krylov space is `span{v, W v}` and breaks down at step 2 with `beta_2` at rounding) | refused (`FemError::Solver`, fewer than `count` pairs converged); with the default options both frequencies `1` within `1e-12` |

The corrected test `u1_r1_n1b_algebraic` holds U1 and R1 unchanged (the same code and limits; deterministic, so their values are
those of the record, compared in the addendum) and N1b in place of N1. The decision rule is unchanged; by it the item is at most
**partial** (a control met only after an amendment made after its run).
