# ADR 0509: F-594 registration: the supernodal L D L^T in the modal, buckling and harmonic solves and on the transformation path

Status: **REGISTERED 2026-11-14, written before any code of F-594 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-594 was written, compiled or run: the files under "What was read" were read; the
only arithmetic done beforehand is disclosed below (by hand, from the measured numbers of ADR 0447 and ADR 0503; no probe was
run). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-594 (deps F-529), structural solver, advanced tier; the follow-up proposed by
[ADR 0503](0503-f529-registration-nested-dissection-and-supernodal-ldlt.md) (its addendum, "For the lead: roadmap
entries"). Number 0509 given by the lead. Branch `f594-work` from 624909b9, worktree `w49i-work/f594`, scratch
`w49i-work/f594-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical (the new behaviour is reached only through a new `Factorisation` variant, a new option field of the direct
harmonic solve whose default is the envelope, and the `SupernodalDirect` static solver that the transformation path refused
until now); no edit of `baseline_hashes.rs`, of the hash tables or of any golden file; no new dependency, no unsafe code, the
crate graph unchanged; clean-room. Every choice below that the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-594 asks

The acceptance, binding (copied verbatim from ADR 0503's addendum):

> Every limit not stated here is registered in the ADR before the run. The supernodal factor after nested dissection is selectable in the subspace and buckling eigensolvers, the direct harmonic solve and the transformation path: on the clamped Hex8 cube of 4 752 unknowns the modal frequencies and buckling factors agree with the envelope's within 1e-9 and the harmonic response within 1e-8; a model with a rigid set solved on the transformation path equals the envelope solve within 1e-10; a modal analysis of a clamped cube of about 1e5 unknowns completes with time and memory recorded; results bit-identical for 1 and 8 threads; the default solver's results keep their bits.

Clauses: **c1** limits registered before the run (this document); **c2** the supernodal factor after nested dissection
selectable in the subspace eigensolver; **c3** in the buckling eigensolver; **c4** in the direct harmonic solve; **c5** on the
transformation path; **c6** on the clamped Hex8 cube of 4 752 unknowns the modal frequencies agree with the envelope's within
1e-9; **c7** its buckling factors within 1e-9; **c8** its harmonic response within 1e-8; **c9** a model with a rigid set solved
on the transformation path equals the envelope solve within 1e-10; **c10** a modal analysis of a clamped cube of about 1e5
unknowns completes, time and memory recorded; **c11** results bit-identical for 1 and 8 threads; **c12** the default
solver's results keep their bits.

## What was read (base 624909b9, read only)

1. `AGENT_RULES.md`; ADR 0503 (all: D1 to D6, the checks, addendum 1 with H1 at 5.04 GB and 81 s, the surviving equivalent
   mutation M11, the follow-up text); ADR 0447 rows S2, S3, S6, H0 and their measured values (S2 9.253e-15 in 23 iterations,
   S3 1.601e-14 with the double factor 239.6152).
2. Code: `crates/rcfd-linalg/src/sparse_chol.rs` (whole: `Permuted`, `permuted_lower`, `symbolic_rows`, `columns_of`,
   `partial_factor`, `SupernodalLdlt`, the orderings, `adjacency` reads the pattern only); `crates/rcfd-linalg/src/eigen.rs`
   lines 1 to 20 and 440 to 1130 (`Factorisation { Envelope, Sparse }`, not `#[non_exhaustive]`; the private `Factor` enum and
   `factor_by`, the single point where the eigensolvers build a factor; `factor_shifted`, `count_unchecked`,
   `buckling_count`, `lowest_eigenpairs`, `lowest_buckling_factors`; no rayon in the eigensolvers);
   `crates/rcfd-linalg/src/cskyline.rs` (whole: `ComplexCsr`, the complex symmetric envelope factor without pivoting, the
   pivot test `|d_i| <= max(theta_i, tau s_i)`, `validate` private); `crates/rcfd-linalg/src/complex.rs` (the operators;
   Smith's division, which for a zero imaginary part divides the real parts exactly as `f64`);
   `crates/rcfd-fem/src/solve.rs` (whole); `crates/rcfd-fem/src/modal.rs` lines 1 to 245; `crates/rcfd-fem/src/buckling.rs`
   lines 1 to 240 (the factorisation passed through `eigen_options` on both paths); `crates/rcfd-fem/src/harmonic_direct.rs`
   (whole: one `ComplexSkylineLdlt` per frequency in reverse Cuthill-McKee order, the resonance test when the matrix is real,
   the transformation path refused); `crates/rcfd-fem/src/mpc.rs` lines 1239 to 1650 (`solve_static_mpc` with its refusals of
   `SparseDirect` and `SupernodalDirect`, `direct`, `singular`, `zero_shift_refused`, `modal_analysis_mpc`, which already
   passes `options.factorisation`).
3. Tests read for set-ups: `crates/rcfd-fem/tests/sa_supernodal_solve.rs` (whole; F3a asserts the transformation-path
   refusal that F-594 removes), `sa_sparse_solve.rs` lines 1 to 140 (S2, S3), `sa_nd_bits.rs` (whole: the digest),
   `sa_harmonic_direct.rs` lines 60 to 140 and 455 to 540 (H8 resonance, H9 threads).
4. **Not read:** any sparse-direct or eigensolver code or documentation outside this repository. The complex symmetric
   factorisation without conjugation is that of ADR 0448 D1 (derived there); the multifrontal method is ADR 0503 D3 (Duff and
   Reid 1983, Liu 1992, cited there from memory). Nothing new is cited.

Findings that shape the design:

1. **The eigensolvers are reached through one function.** `factor_by` builds every factor of `lowest_eigenpairs` (the shifted
   factor and the Sturm counts) and of `lowest_buckling_factors`; `rcfd-fem` modal and buckling pass their
   `factorisation` option through on the elimination path and on the transformation path. A new `Factorisation` variant makes
   c2 and c3 selectable without touching `modal.rs`, `buckling.rs` or any geometric file. The eigensolvers see only matrices,
   so the ordering there is nested dissection by level structures (graph only).
2. **`Factorisation` is not `#[non_exhaustive]`;** it is matched only in `eigen.rs` in the workspace. A new variant changes no
   other code (an external exhaustive match would need an arm; noted as a limit).
3. **The harmonic solve is complex.** `SupernodalLdlt` is real only and there is no complex variant. A real-equivalent form
   `[[A_r, -A_i], [-A_i, -A_r]]` is symmetric but always indefinite with `n` negative eigenvalues; an unpivoted `L D L^T` of it
   can meet a zero pivot where the complex form cannot (for `A = K' + i B`, `B` positive definite, every leading principal
   submatrix of any symmetric permutation is non-singular, ADR 0448 D1), and it stores twice the bytes. So F-594 adds a
   complex symmetric supernodal factor (no conjugation, no pivoting), with the same numeric kernel as the real one.
4. **The transformation path refuses `SupernodalDirect`** (ADR 0503 D5) and the F-529 test `f3_refusals` asserts that refusal
   (F3a). c5 needs the solve there, so the refusal is replaced. That test is kept as an ignored record (attribute only; its
   other two parts, F3b and F3c, are repeated unchanged in this item's test file), the procedure of ADR 0448 for
   `r1_to_r11_refusals`. This is the one edit of an existing test file; it is listed for the maintainer (open question 2).
5. The mechanism checks of the transformation path (`zero_shift_refused`, `zero_shift_failing_pivot`) factor `T^T K T` by the
   envelope. They decide a refusal, not the result; they stay envelope (a limit).

## Arithmetic before this registration (disclosed)

By hand, from measured numbers in the repository; no code run.

- **Agreement estimates (c6 to c9).** ADR 0447 S2 measured 9.3e-15 (frequencies, minimum degree against envelope) and S3
  1.6e-14 (buckling factors) on S; ADR 0503 F1 measured 7.1e-15 (static displacements, supernodal against envelope). Two
  backward-stable factors of the same matrix give eigenvalues that differ by about `kappa eps` times the residual tolerance's
  effect on the converged subspace; the frequencies are converged to a residual of 1e-10, whose eigenvalue error is of order
  its square, so the difference is dominated by rounding: about 1e-14. Harmonic: the dynamic stiffness at a damped
  resonance with loss factor `eta` has a condition number about `(lambda_max / lambda_1) / eta`; `lambda_max / lambda_1` of S
  is about `(2 c / h)^2 / omega_1^2` with `c = sqrt(E / rho) = 5 050 m/s`, `h = 1/11`, `omega_1` of a clamped unit cube of
  order `2 pi 1.5e3`: about 1.3e2 to 1e3, so `kappa` about 1e4 to 5e4 and the difference about `kappa eps` = 1e-12 to 1e-11.
  Transformation path: as F1, about 1e-14.
- **Heavy record (c10).** The cube `cube(32, 7850)`: 32 x 33 x 33 free nodes, 104 544 unknowns (about 1e5). ADR 0503's probe
  of the level-structure dissection on `cube(46)` gave nnz(L) 504 360 309 (304 842 unknowns); nnz(L) of nested dissection
  scales as `N^(4/3)`: `(104 544 / 304 842)^(4/3) = 0.240`, about 1.2e8 entries, 0.97 GB of values per factor. The subspace
  iteration keeps its factor while the Sturm count builds a second one: about 2 GB of factors, plus `K`, `M`, `K - s M` and
  the subspace (about 0.6 GB): **about 3 GB peak**. Time: factor work `1.3e12 x 0.343^2 = 1.5e11` multiply-adds, at ADR
  0503 H1's rate (1.8e12 in about 70 s) about 6 s per factor; the solves `4 nnz(L)` flops each, 14 vectors per iteration,
  about 25 iterations (S took 23): 350 solves x 4.8e8 = 1.7e11 flops, sequential and memory bound at about 1 to 2 Gflop/s:
  1.5 to 3 minutes. **About 2 to 6 minutes in all.** It is run alone, after the free memory is at least 10 GB.
- **Bits across threads (c11).** The only parallel step of both supernodal factors is the later-column update of large
  fronts (ADR 0503 D3), whose summation order does not depend on the split; the eigensolvers, the harmonic assembly of the
  reduced matrix and the solves are sequential; the assembly of `K` and `M` takes the thread count and is bit-identical for
  any (ADR 0275). S's largest front is about `3 x 12 x 12 = 432` rows, above the split threshold (`tail (tail + 1) / 2 x 32
  >= 2^20` from `tail` about 256), so the parallel path runs on S.

## Design

### D1. One numeric kernel for both scalars (`crates/rcfd-linalg/src/sparse_chol.rs`)

A private trait `FrontScalar` (`Copy`, `Send`, `Sync`, the four operators and their assignments, `ZERO`, `modulus`,
`finite`) implemented for `f64` and `C64`. The supernodal code of ADR 0503 after the ordering (`permuted_lower`,
`symbolic_rows`, `columns_of`, the fundamental supernodes, the row structures, the assembly with extend-add, the panel
factorisation `partial_factor` with its split rule and the four-way `dot4`, the hand-over, the solve) becomes generic over
the scalar, with the **same expressions in the same order** for `f64` (so `SupernodalLdlt`'s factor and solves keep their
bits; guarded by digests G7, G8 recorded at base, check B). The pivot test is a closure given by the caller: it receives
`a_kk`, `d_k` and `w_k` and returns the threshold when it refuses. `SupernodalLdlt`'s closure is ADR 0503 D3's test verbatim:
`theta = (w + 1) eps (|a_kk| + |a_kk - d_k|)`, refused by `PivotRule::Positive` (`d <= theta`) or `NonZero` (`|d| <= theta`) or
if `d` is not finite. Byte counts use `size_of::<T>()` (8 for `f64`, as before). `SparseLdlt`, `factor_count`,
`elimination_tree` and the orderings read the real `Permuted` as before.

### D2. The complex symmetric supernodal factor: `ComplexSupernodalLdlt`

`pub struct ComplexSupernodalLdlt` in `sparse_chol.rs`, `pub struct ComplexSupernodalOptions { ordering: SparseOrdering,
symmetry_tolerance: f64, min_relative_pivot: f64 }` (default nested dissection by level structures, 1e-12, 0). `factor(a:
&ComplexCsr)` and `factor_with(a, &options)`:

1. Validation by `cskyline`'s `validate` (made `pub(crate)`; finite, complex symmetric `|a_rc - a_cr| <= delta sqrt(|a_rr|
   |a_cc|)`, no conjugation); `tau` finite and `>= 0` (else `Malformed`); the ordering computed on the pattern (a real
   `CsrMatrix` of the same arrays with every value 1: the orderings read the pattern only), `Given` checked by
   `check_permutation` (refusal mapped to `ComplexSkylineError::BadOrdering`).
2. The kernel of D1 in `C64`: `A = L D L^T`, `L` unit lower, `D` complex diagonal, no conjugation anywhere.
3. **Pivot test**, the complex form of D1's: `s_k = |a_kk| + |a_kk - d_k|` (moduli), `theta_k = max((w_k + 1) eps s_k, tau
   s_k)`; refused when `|d_k| <= theta_k` or `d_k` is not finite, as `ComplexSkylineError::Pivot { row: order[k], position: k,
   pivot, threshold: theta_k, scale: s_k }` (`row` the original equation). The envelope's scale `|a_ii| + sum |u_ij l_ij|` is
   at least `|a_ii| + |a_ii - d_i|`, so for the same pivot the supernodal test with `tau > 0` refuses no more than the
   envelope's; pivots differ anyway because the order differs. Every refusal type is `ComplexSkylineError`, so the callers'
   error mapping is shared.
4. Solve: `z = P b`, forward by columns in supernode order, `z_j /= d_j` (complex division), backward in reverse, `x = P^T
   z`. Accessors `n`, `order`, `pivots` (`&[C64]`), `column(j)`, `factor_entries`, `supernodes`, `max_front`,
   `split_fronts`, `solve`, `solve_into`.
5. **For a real matrix** (zero imaginary parts) every operation reduces to the real one: products `re re - 0 0`, sums of zero
   imaginary parts, Smith's division with `d = 0` (`r = 0`, `den = c`, `(a + b r) / den = a / c`). So the real parts equal
   `SupernodalLdlt`'s values in the same order (compared with `==`, which identifies `-0` and `+0`) and every imaginary part
   is zero (check L2).

### D3. The eigensolvers (`crates/rcfd-linalg/src/eigen.rs`)

`Factorisation::Supernodal` (new variant): "the supernodal multifrontal `L D L^T` after nested dissection by level
structures (ADR 0509); the options' `ordering` is not used". `Factor` gains `Supernodal(SupernodalLdlt)` with the four
methods; `factor_by` builds `SupernodalLdlt::factor_with(a, &SparseOptions { ordering: NestedDissection, pivots:
opts.pivots, symmetry_tolerance: opts.symmetry_tolerance })`. The pivot rule is the caller's (positive for the shifted factor
and for `K`, non-zero for the Sturm counts), so the inertia of an indefinite count is read from `D` (Sylvester). Selectable
by `SubspaceOptions::factorisation` (c2) and `BucklingEigenOptions::factorisation` (c3), and therefore in `rcfd_fem` by
`ModalOptions::factorisation` and `BucklingOptions::factorisation` on the elimination and the transformation path. The
`Envelope` and `Sparse` arms are unchanged.

### D4. The direct harmonic solve (`crates/rcfd-fem/src/harmonic_direct.rs`)

`DirectHarmonicOptions` (already `#[non_exhaustive]`) gains `pub factor: DirectFactor` (default `DirectFactor::Envelope`,
the code of ADR 0448 as it is) and the builder `factor(DirectFactor)`. With `DirectFactor::SupernodalNestedDissection`,
`DirectHarmonic::new` computes once the order `nested_dissection_order_with_points` of the reduced free-free pattern (the
mesh pattern plus the dashpot entries, as assembled at every frequency) with the coordinates of each free equation's node
(the static solve's choice, ADR 0503 D1b), and each frequency factors the same reduced complex matrix by
`ComplexSupernodalLdlt::factor_with` with that `Given` order and the same `tau` rule (the resonance threshold when the matrix
is real, 0 when damped). A refusal is mapped by the existing code (same key, same message). `DirectFactor::SparseMinimumDegree`
is refused in `new` (`FemError::InvalidValue`, key `"solver"`). Nothing else of the module changes.

### D5. The transformation path (`crates/rcfd-fem/src/mpc.rs`)

In `solve_static_mpc`, the refusal of `SupernodalDirect` is replaced by a solve: `SupernodalLdlt::factor` of `K_r = T^T K T`
(nested dissection by level structures of `K_r`'s graph: the reference unknowns have no node), the pivot checks of `direct`
(a refusal or `d_i <= 1e-10 a_ii` mapped by `singular`), solver name `SUPERNODAL_NAME`, `report.factor =
Some(SupernodalNestedDissection)`; `solver_name` gains the arm. `SparseDirect` and the preconditioned CG stay refused there.
The modal analysis and the buckling of the transformation path take `Factorisation::Supernodal` through D3 already.

### D6. Files

`crates/rcfd-linalg/src/sparse_chol.rs`, `crates/rcfd-linalg/src/cskyline.rs` (`validate` made `pub(crate)`, nothing else),
`crates/rcfd-linalg/src/eigen.rs`, `crates/rcfd-fem/src/harmonic_direct.rs`, `crates/rcfd-fem/src/mpc.rs` (the refusal and the
arm); tests `crates/rcfd-linalg/tests/sa_supernodal_complex.rs`, `crates/rcfd-fem/tests/sa_supernodal_modes.rs`,
`crates/rcfd-fem/tests/sa_f594_bits.rs`; the `#[ignore]` attribute on `f3_refusals` in
`crates/rcfd-fem/tests/sa_supernodal_solve.rs` (finding 4). Not touched: `modal.rs`, `buckling.rs`, `geometric.rs`,
`geometric_struct.rs`, `nonlinear.rs`, `nl_transient.rs`, `hex8x.rs`, any contact file, any `rcfd-api` file, any golden.

## Checks (limits fixed now)

Release profile, each run once: `cargo test --release -p rcfd-linalg --test sa_supernodal_complex -- --test-threads=1
--nocapture`, `cargo test --release -p rcfd-fem --test sa_supernodal_modes --test sa_f594_bits -- --test-threads=1
--nocapture`; the heavy record by name with `--ignored`, alone.

Model **S**: the clamped Hex8 cube `cube(11, 7850)` of `sa_supernodal_solve.rs` (4 752 unknowns; E = 2e11, nu = 0.3, `xmin`
clamped, traction (0, 0, -1e6) on `xmax`). Model **R**: S with `Constraint::rigid_set("xmax", [1.0, 0.5, 0.5])` (the set-up
of F3a). Its eigenvalues `lambda_j` and `omega_j = sqrt(lambda_j)` are those of the envelope modal analysis of S in the same
test. Matrices of the linalg tests: the 7-point Laplacian on `m^3` (diagonal 6), the 5-point on `m^2` (diagonal 4), the
tridiagonal (2, -1); complex right-hand sides `b_i = (2 u - 1) + i (2 u' - 1)` from the LCG of ADR 0447 with the seed
`0xF594_0509`. Differences of complex vectors are `max_i |x_i - y_i| / max_i |y_i|` with complex moduli. **Every aggregated
error is asserted finite before it is compared.**

**Bit record procedure (c12).** `sa_f594_bits.rs` uses only the API of base 624909b9. Before any library code it is run once
in the worktree at base plus this ADR and the file itself; it prints one FNV-1a 64 digest (ADR 0291) per quantity; the
printed digests become its constants and the file is committed before the library code. After the change it runs unchanged
and compares. Cases: **G1** `modal_analysis` of S, 6 modes, default options (frequencies, every mode, residuals, iterations
and the Sturm record); **G2** the same with `Factorisation::Sparse` (frequencies, modes); **G3** `buckling::factors` of S with
the uniform pre-stress `[-1e8, 0, 0, 0, 0, 0]`, 2 factors, default options (factors, modes); **G4** `DirectHarmonic` of S with
the loss factor 0.02 on `all`, default options, the load `HarmonicLoad::from_model(S)`, at `omega = 1.5 omega_1` with `omega_1`
from G1 (re, im); **G5** `solve_static_report` of R with `Direct` (displacements, reactions, the report's solver bytes and
residual); **G6** `modal_analysis` of R, 6 modes, default options (frequencies, modes); **G7** `SupernodalLdlt::factor` of the
7-point 20^3 grid (order, every `column(j)` row and value, `D`) and its solve of the real LCG vector; **G8**
`solve_static_report` of S with `SupernodalDirect` (displacements, reactions, report). `sa_nd_bits.rs`, `sa_sparse_bits.rs`
and `baseline_hashes` (rcfd-api) are run unchanged.

| Id | Clause | Check | Limit | Estimate / basis |
| --- | --- | --- | --- | --- |
| L1 | c4 (kernel) | 7-point 20^3, (a) `A = L7 + 0.5 i I`, (b) `A = L7 - 3 I + 0.5 i I` (real part indefinite): `ComplexSupernodalLdlt::factor`: `|b - A x| / |b|`; and against `ComplexSkylineLdlt::factor`'s solution | `< 1e-12`; `<= 1e-11` | `kappa` <= 30: about 1e-15 |
| L2 | D2 item 5 | the 7-point 20^3 grid (real) as `ComplexCsr::from_real`, nested dissection: every `L` value's real part, `D`'s real parts and the solution's real parts `==` `SupernodalLdlt` with the same order; every imaginary part `== 0` | as stated | D2 item 5 |
| L3 | c11 | L1 (b) factored and solved inside rayon pools of 1 and 8 threads: order, every `column(j)` (rows; re and im bits), `D`, `x` bit for bit; `split_fronts() > 0` | bit for bit; `> 0` | D1 |
| L4 | D2 | refusals of `ComplexSupernodalLdlt::factor_with`: (a) `[[1, 1], [1, 1]]` with `Given([1, 0])`: `Pivot { row: 0, position: 1 }`; (b) the 5-point 20^2 grid with `min_relative_pivot = 1`: `Pivot` at position 0, `row = order[0]`, threshold equal to its scale; (c) NaN at (3, 4): `NonFinite { row: 3, col: 4 }`; (d) `a_12 = -1, a_21 = -2`: `Unsymmetric { row: 1, col: 2 }`; (e) `Given` with a repeated index: `BadOrdering`; (f) `min_relative_pivot = -1`: `Malformed` | as stated | (b): `|d| <= s` always |
| L5 | c2, c3 | tridiagonal (2, -1), n = 200: (a) `lowest_eigenpairs(K, I, 5)` with `Factorisation::Supernodal`: `max_j |lambda_j - (2 - 2 cos(j pi / 201))| / lambda_j`, and `sturm.counted == sturm.expected`; (b) `lowest_buckling_factors(K, -I, 3)` with `Factorisation::Supernodal`: the same closed form | `<= 1e-9`; equal; `<= 1e-9` | rounding `eps ||K|| / lambda_1` = 4e-12 |
| E1 | c2, c6 | S, `modal_analysis`, 6 modes, `Factorisation::Supernodal` against the default: `max_j |f_s - f_e| / f_e`; every supernodal residual `<= 1e-10` and its Sturm `counted == expected` | `<= 1e-9`; as stated | about 1e-14 (S2: 9.3e-15) |
| E2 | c3, c7 | S, `buckling::factors`, G3's pre-stress, 2 factors, `Supernodal` against the default: `max_j |lambda_s - lambda_e| / lambda_e`; 2 factors found | `<= 1e-9`; 2 | about 1e-14 (S3: 1.6e-14) |
| E3 | c4, c8 | S, `DirectHarmonic` with `factor(SupernodalNestedDissection)` against the default, load `HarmonicLoad::from_model(S)`: (a) loss factor 0.02 on `all` at `omega_1` (damped resonance); (b) undamped at `0.5 omega_1`; (c) Rayleigh `(0.02 omega_1, 0.02 / omega_1)` at `2 omega_1`: the complex difference of the full response, each case | `<= 1e-8` each | 1e-12 to 1e-11 |
| E4 | c4 | S undamped with the supernodal factor: (a) at `omega_1` refused, `FemError::InvalidValue` key `"frequency"`, the message contains "resonance"; (b) at `1.001 omega_1` accepted, finite; (c) `factor(SparseMinimumDegree)` refused in `new` with key `"solver"` | as stated | the last pivot of a singular matrix is of order `1e-14` relative |
| E5 | c5, c9 | R, `solve_static_report` with `SupernodalDirect` against `Direct`: displacements `max |u_s - u_e| / max |u_e|`; reference translations and rotations each `max |r_s - r_e| / max |r_e|` over their three components; `report.factor == Some(SupernodalNestedDissection)` and solver `"sparse-ldlt-supernodal"` (and `Direct`'s `Some(Envelope)`); the supernodal relative residual | `<= 1e-10`; `<= 1e-10`; as stated; `<= 1e-12` | about 1e-14 (F1: 7.1e-15, 1.2e-13) |
| E6 | c2, c5 | R, `modal_analysis` (transformation path), 6 modes, `Supernodal` against the default: `max_j |f_s - f_e| / f_e` | `<= 1e-9` | about 1e-14 |
| E7 | c11 | inside rayon pools of 1 and of 8 threads, the options' threads 1 and 8: E1's supernodal frequencies and modes, E2's factors and modes, E3 (a)'s response (re, im), E5's displacements and reactions | bit for bit | D1, ADR 0275 |
| E8 | D5 | `SupernodalDirect`: with `refinement(1)` refused (`FemError::Solver`); the hinge mechanism of `sa_supernodal_solve.rs` F3c `FemError::SingularStiffness` (F3b and F3c of ADR 0503, repeated) | as stated | |
| H1 | c10 | (heavy, `#[ignore]`) `cube(32, 7850)`, 104 544 unknowns, `modal_analysis`, 6 modes, `Factorisation::Supernodal`: completes; the 6 frequencies finite, positive, ascending; every residual `<= 1e-10`; Sturm `counted == expected`; recorded: wall time of `modal_analysis`, the peak working set of the test process, iterations, and after it (symbolic only) nnz(L) of `K_FF` by `nested_dissection_order` and by `nested_dissection_order_with_points` | as stated; time and memory recorded | 2 to 6 min, about 3 GB, nnz(L) about 1.2e8 |
| B | c12 | `sa_f594_bits.rs` G1 to G8 digests equal to base; `sa_nd_bits.rs` and `sa_sparse_bits.rs` pass; `cargo test -p rcfd-api --release --test baseline_hashes` passes | equal; pass; pass | |

**H1's memory.** The test binary is built first; the test is started alone, as its own process, by a PowerShell script
(`f594-scratch/heavy.ps1`, ADR 0503's script with the names changed) that polls the process's `PeakWorkingSet64` every second
until exit and records the largest value seen and the wall time. It is started only when the free physical memory is at least
10 GB, waiting otherwise; if that never happens on the day it is recorded as not run with the estimate. No memory limit is
stated by the acceptance; none is added (record).

### Negative controls

| Id | Control | Must hold |
| --- | --- | --- |
| N1 | E1: the supernodal frequencies are bit-equal neither to the envelope's nor to those of `Factorisation::Sparse` | holds (the supernodal factor ran) |
| N2 | E3 (a): the supernodal response is not bit-equal to the envelope's | holds |
| N3 | E5: the supernodal displacements of R are not bit-equal to `Direct`'s | holds |
| N4 | L1 (b): the supernodal solution is not bit-equal to the complex envelope's | holds |

### Planned mutations

On a `git archive` copy of the final tree in `w49i-work/f594-scratch/mut` with its own target directory; each alone, by exact
text replacement; the named test files run once; the file restored.

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | `factor_by`'s `Supernodal` arm builds the minimum-degree `SparseLdlt` | N1 |
| M2 | `factor_by`'s `Supernodal` arm uses `PivotRule::Positive` whatever the caller asks | E1 (the Sturm count of the indefinite `K - s M` refused) and L5 |
| M3 | the complex assembly takes the real part of each entry of `A` only | L1 |
| M4 | the complex update conjugates `l` (`x -= conj(l) w`) | L1 |
| M5 | the complex pivot test ignores `tau` | L4 (b) and E4 (a) |
| M6 | the panel width is `16 * rayon::current_num_threads()` | L3 |
| M7 | the backward substitution of the solve is skipped | L1 |
| M8 | `DirectHarmonic` ignores the `factor` option (always the envelope) | N2 |
| M9 | the transformation path's `SupernodalDirect` arm calls the envelope `direct` (reporting the supernodal factor) | N3 |
| M10 | `factor_by`'s `Envelope` arm builds the supernodal factor | B (G1) |
| M11 | the generic `dot4` combines `(s0 + s2) + (s1 + s3)` | B (G7 or G8) |

## Decision rule

**Done** if every check meets its limit on its registered run (H1 included), every negative control holds and every mutation
is caught by a check named for it on the first run; **partial** if a check misses or is not run, a mutation survives or is
caught only by another check, or a check is met only after an amendment made after its run; **todo** if the modules do not
build or a default bit changes and cannot be restored without changing a default.

## Limits (not covered)

- The eigensolvers and the transformation path order by level structures (graph only); the coordinate dissection, about 1.2
  times less fill on the 27-point stencil (ADR 0503's probe), is used only by the static elimination path and the direct
  harmonic solve.
- The mechanism checks of the transformation path (`zero_shift_refused`, `zero_shift_failing_pivot`) and the `rigid_free`
  flag of its modal analysis with a given shift factor by the envelope; the cyclic solve, the refinement of ADR 0397, the
  Newton driver, the modal-superposition harmonic solve, CMS, the follower, geometric and cyclic eigen calls (which build
  their own `SubspaceOptions` with the default) are not changed.
- The complex factor has no pivoting (as the envelope's): a matrix with a singular leading principal submatrix in the chosen
  order is refused, not solved.
- `Factorisation` is not `#[non_exhaustive]`: a new variant would break an exhaustive match outside the workspace (none
  exists inside).
- No case-file key (open question 1).
- H1's memory is the process peak working set polled once a second.

## Open questions

1. **Case-file key.** Not added (it would change the schema golden). Options: (a) `modal.factorisation = "supernodal"`, the
   same for `buckling` and `harmonic`, and `solver.method = "sparse_nd"` (ADR 0503's proposal) for the static solve; (b) one
   key `solver.factor = "envelope" | "supernodal"` read by every analysis; (c) none. Recommendation: (b), a single key for all
   analyses, after the maintainer approves the golden change.
2. **The edit of `f3_refusals` (ADR 0503).** Its part F3a asserts the transformation-path refusal that the acceptance of F-594
   removes; it gets `#[ignore]` with the reason, and F3b and F3c are repeated as E8. Options: (a) as done; (b) keep the
   refusal and select the transformation-path factor by a separate option (a second selector for one solver). Taken on the
   maintainer's standing instruction of 2026-11-11 (take the recommendation): (a).
3. Whether the supernodal factor becomes the default of any analysis above some size changes default results: not taken,
   open.
4. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): the harmonic option reuses
   `rcfd_fem::solve::DirectFactor` (one enum names the factor everywhere) and refuses its minimum-degree value; the complex
   factor reuses `ComplexSkylineError`.

## Addendum 1: results (2026-11-14)

Registered runs at 7f36dcb5, release profile, each run once: `sa_supernodal_complex.rs` (log `f594-scratch/run_linalg.log`),
`sa_supernodal_modes.rs` and `sa_f594_bits.rs` (`run_fem.log`), H1 alone through `f594-scratch/heavy.ps1` (`run_h1.log`,
`run_h1_memory.log`; free memory at the start 16.49e9 bytes). The bit record `sa_f594_bits.rs` was run at 0186cf62 (base
624909b9 plus this ADR) plus the file (`bits_base_run.log`), its 22 constants filled in and committed at b0dadb37 before any
library line; 7f36dcb5 changed only its `cargo fmt` layout (the same values). No probe or development run was made before the
registered runs. No limit was changed and no amendment was needed.

### Checks

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| L1 (c4) | `< 1e-12`; `<= 1e-11` | (a) residual 7.659e-16, against the envelope 2.090e-15; (b) 1.255e-15, 2.015e-15; 5 510 supernodes, largest front 469, 13 fronts split | yes |
| L2 | `==`; imaginary parts 0 | 591 826 values of L, D and the solution equal to the real factor's; every imaginary part 0 | yes |
| L3 (c11) | bit for bit; `> 0` | order, L, D, x equal for 1 and 8 threads; 13 fronts split | yes |
| L4 | as stated | (a) row 0 at position 1, pivot 0; (b) row 46 at position 0, threshold = scale = 4; (c) NonFinite (3, 4); (d) Unsymmetric (1, 2); (e) BadOrdering; (f) Malformed | yes |
| L5 (c2, c3) | `<= 1e-9`; Sturm equal; `<= 1e-9` | (a) 6.125e-14, Sturm 5 = 5, 12 iterations; (b) 6.192e-14, Sturm 3 = 3 | yes |
| E1 (c2, c6) | `<= 1e-9`; residuals `<= 1e-10`; Sturm | 1.472e-15; largest residual 4.563e-11; Sturm 6 = 6; 23 iterations each; 0.73 s against the envelope's 0.69 s | yes |
| E2 (c3, c7) | `<= 1e-9`; 2 factors | 1.779e-15 (239.61515575162, double) | yes |
| E3 (c4, c8) | `<= 1e-8` each | (a) 1.932e-13; (b) 1.086e-14; (c) 8.705e-15; 0.083 s per frequency against the envelope's 0.32 s | yes |
| E4 | as stated | (a) refused at omega_1 = 3 396.878 rad/s, key "frequency", "a resonance without damping", pivot modulus 1.77e-3 at scale 1.71e10; (b) 1.001 omega_1 accepted; (c) `SparseMinimumDegree` refused, key "solver" | yes |
| E5 (c5, c9) | `<= 1e-10` (three); factor and name; `<= 1e-12` | displacements 4.082e-15, reference translation 2.857e-15, rotation 1.894e-15 (4 326 reduced unknowns); Supernodal / Envelope, "sparse-ldlt-supernodal"; residual 2.345e-14 | yes |
| E6 (c2, c5) | `<= 1e-9` | 2.310e-15 | yes |
| E7 (c11) | bit for bit | modal frequencies and modes, buckling factors and modes, harmonic re and im, transformation-path displacements and reactions equal for 1 and 8 threads | yes |
| E8 | as stated | refinement refused (`Solver`); hinge `SingularStiffness` at node 13 component 0 | yes |
| H1 (c10) | completes; finite, positive, ascending; residuals `<= 1e-10`; Sturm; record | 104 544 unknowns; `modal_analysis` 77.7 s (process 81.1 s), 22 iterations, peak working set 2.945e9 B; frequencies 537.05 (double), 729.96, 1 282.81, 1 420.87 (double) Hz; largest residual 6.284e-11; Sturm 6 = 6; nnz(K_FF) 7 960 014, nnz(L) by level structures 110 872 668 (0.89 GB of values), by coordinates 92 953 458 (0.838 of it) | yes |
| B (c12) | equal; pass; pass | 22 digests equal to base; see "Other checks run" | yes |
| N1 | not bit-equal (both) | false, false | holds |
| N2 | not bit-equal | false | holds |
| N3 | not bit-equal | false | holds |
| N4 | not bit-equal | false | holds |

H1 against the estimates: 2.9 GB (estimate about 3 GB), 78 s (estimate 2 to 6 minutes; the solves ran faster than the
memory-bound guess), nnz(L) 1.11e8 (estimate 1.2e8). The coordinate dissection would store 16 percent fewer entries
(remaining question 5).

### Mutations (copy `f594-scratch/mut/tree` of 7f36dcb5, own target `target-f594-mut`, `f594-scratch/mutate.py`, each alone; log `mutations.log`)

| Id | Caught by | Registered | Result |
| --- | --- | --- | --- |
| M1 | N1 (bit-equal to the minimum degree's) | N1 | caught |
| M2 | L5, E1, E2, E6, E7 (the Sturm count of an indefinite matrix refused) | E1 and L5 | caught |
| M3 | L1 (residual), L3 | L1 | caught |
| M4 | L1 (residual) | L1 | caught |
| M5 | L4 (b) only; `sa_supernodal_modes` passed | L4 (b) and E4 (a) | **caught by L4 (b), not by E4 (a)** |
| M6 | L3 (values) | L3 | caught |
| M7 | L1, L5 | L1 | caught |
| M8 | N2 | N2 | caught |
| M9 | N3 | N3 | caught |
| M10 | B | B (G1) | caught |
| M11 | B | B (G7 or G8) | caught |

M5 diagnosed after the run: at the undamped resonance of S the refused pivot (modulus 1.77e-3 at scale 1.71e10, relative
1.0e-13) is below the rounding threshold `(w + 1) eps s` as well as below `tau s`, so removing `tau` leaves E4 (a)'s refusal in
place. E4 (a) shows that the refusal exists, not that the `tau` term makes it; L4 (b) is the check that isolates `tau`. The
registration should have named L4 (b) alone, or added a frequency whose pivot lies between the two thresholds.

### Other checks run

`cargo test --release -p rcfd-linalg` (17 binaries, 115 passed, 0 failed, 6 ignored; `run_linalg_all.log`) and `cargo test
--release -p rcfd-fem` (174 binaries, 777 passed, 0 failed, 204 ignored; `run_fem_all.log`), `sa_nd_bits.rs` and
`sa_sparse_bits.rs` among them. `cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored
(`run_baseline.log`). `cargo test -p rcfd-cli --release --test docs_site`: 2 passed. `cargo fmt -p rcfd-linalg --check`,
`cargo fmt -p rcfd-fem --check`: clean. `cargo clippy --release -p rcfd-linalg -p rcfd-fem --all-targets -- -D warnings`:
clean.

Shared solver files changed: `crates/rcfd-linalg/src/sparse_chol.rs` (the supernodal section made generic, the complex
factor added, `permuted_lower` split into a generic part), `crates/rcfd-linalg/src/eigen.rs` (one variant and one `Factor`
arm in each of five places), `crates/rcfd-linalg/src/cskyline.rs` (`validate` made `pub(crate)`),
`crates/rcfd-fem/src/harmonic_direct.rs` (the option, the order in `new`, the factor choice in `solve`),
`crates/rcfd-fem/src/mpc.rs` (the refusal removed, the arm, the report's factor, `supernodal`), `crates/rcfd-fem/src/solve.rs`
(one doc comment). Test file edited: the `#[ignore]` attribute on `f3_refusals` in `sa_supernodal_solve.rs` (finding 4).

### Not run

Nothing registered was left out.

### Decision by the registered rule

**Partial**: every check met its limit on its registered run (H1 included) and every negative control held, but mutation M5
was caught only by L4 (b), not also by E4 (a) as registered (diagnosed above). Every clause of the acceptance was met.

### Remaining questions

1. M5. Options: (a) accept L4 (b) as the check of the `tau` term and the item as done; (b) a follow-up check at a frequency
   whose pivot lies between the rounding and the `tau` thresholds. Recommendation: (a); the maintainer decides.
2. Case-file key (open question 1). Recommendation: one key `solver.factor = "envelope" | "supernodal"` read by every
   analysis, after the maintainer approves the schema golden change.
3. The `#[ignore]` on ADR 0503's `f3_refusals` (open question 2). Recommendation: accept it (the precedent of ADR 0448).
4. Default above a size (open question 3): changes default results; not taken, open.
5. The coordinate dissection in the eigensolvers (16 percent less fill on the 1e5 cube). Options: (a) a points option of
   `SubspaceOptions` and `BucklingEigenOptions` filled by `rcfd-fem`; (b) leave the graph order. Recommendation: (a), as a
   small follow-up when larger modal models need it.

### For the lead: roadmap entries

Proposed status of F-594: **partial** (done if the maintainer accepts M5 as caught by L4 (b)).

> Structural solver, advanced tier. Follow-up of F-529 (ADR 0503). ADR 0509, 2026-11-14: rcfd_linalg::eigen::Factorisation::Supernodal (the supernodal L D L^T after nested dissection by level structures in the subspace and buckling eigensolvers, so in modal_analysis and buckling::factors on the elimination and the transformation path), rcfd_linalg::sparse_chol::ComplexSupernodalLdlt (the same multifrontal kernel made generic over f64 and C64, complex symmetric without conjugation or pivoting; for a real matrix the real factor's values), DirectHarmonicOptions::factor (DirectFactor::SupernodalNestedDissection, the order by the node coordinates computed once per solver), and StaticSolver::SupernodalDirect on the transformation path. Clamped Hex8 cube of 4 752 unknowns: modal frequencies 1.5e-15 and buckling factors 1.8e-15 from the envelope's (limit 1e-9); harmonic response 1.9e-13 at the damped resonance, 1.1e-14 undamped, 8.7e-15 with Rayleigh damping (limit 1e-8), about 4 times faster than the envelope per frequency; with a rigid set the transformation-path static solve 4.1e-15 from the envelope's (limit 1e-10) and its modes 2.3e-15. Clamped cube of 104 544 unknowns: modal analysis of 6 modes in 78 s at a peak working set of 2.9 GB. Results bit-identical for 1 and 8 threads; 22 digests of the default paths equal to base; baseline_hashes passes. ADR 0503's F3a (the transformation-path refusal) kept as an ignored record. Partial by the registered rule only because mutation M5 (the tau term of the complex pivot test removed) was caught by L4 (b) but not also by E4 (a), whose resonance pivot is below the rounding threshold too. Done is the maintainer's call.

Follow-up item (optional), title "Coordinate nested dissection in the eigensolvers", deps F-594, acceptance:

> Every limit not stated here is registered in the ADR before the run. The subspace and buckling eigensolvers take optional point coordinates for the supernodal factor's nested dissection and rcfd-fem modal and buckling pass the node coordinates of the free unknowns: on the clamped cube of 104 544 unknowns nnz(L) at most 0.85 of the level-structure order's (both recorded) and the frequencies within 1e-9 of the level-structure run; results bit-identical for 1 and 8 threads; the default solver's results keep their bits.
