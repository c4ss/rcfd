# ADR 0442: F-520 registration: accurate far-mode vectors of the sparse whirl state solver (each returned mode refined by inverse iteration on `Q(s) = K_eff + s G + s^2 M` at its own eigenvalue, with a complex band LU factor with partial pivoting and Gram-Schmidt within clusters of equal eigenvalues; opt-in)

Status: **REGISTERED 2026-11-14, written before any code of F-520 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-520 was written, compiled or run: the code and logs listed under "What was read" were
read and the arithmetic below was done by hand (no script, no build, no run). The only numbers below that came from a run are
those of the registered run 1 of ADR 0428 and its diagnostic print, as `f519-scratch/run1.log` and
`f519-scratch/diag1_residuals.log` show them (disclosed under "Arithmetic"). Results go in the addendum at the end; nothing above
the addendum is changed after a run.

Parents: [ADR 0428](0428-f519-registration-sparse-whirl-state.md) (F-519: the sparse whirl state solver, `SparseStateOptions`,
`gyroscopic_state_eigen_sparse`, `RotatingPart::whirl_state_sparse`, `RotatingSector::whirl_state_sparse` in
`crates/rcfd-fem/src/rotating/whirl/state/arnoldi.rs`; its D3 smoothing step; its amendment 2, Q1 of the cube missed, 8.466e-8
against 1e-8, the three far elastic modes near 1.07e5 .. 1.10e5 rad/s; its remaining question 1, option (a), taken here), [ADR
0416](0416-f505-registration-gyroscopic-stabilisation.md) (F-505: the dense state solver, the cube CBI / CBA, the flag rule, and
its D4: the mode vector of an eigenvalue `s` by three inverse iterations on `Q(s)` with a pivoted LU, a pivot below `eps
max|Q|` replaced by that value). Roadmap item F-520 (deps F-519). Size S. Number 0442 given by the lead. Branch
`f520-far-mode-vectors` (base `b032cbf0`), worktree `w49i-work/f520`, scratch `w49i-work/f520-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, the hash tables or a golden file; no new dependency, no unsafe code; clean-room. The ignored record
`e1_f1_l1_w1_q1_i1_n2_n3_cube_known_miss` of ADR 0428 and every other test of `si_rotating_whirl_state_sparse.rs` stay unedited.

Library edits, inside `crates/rcfd-fem/src/rotating` only (another agent edits other `rcfd-fem` modules in parallel):
`rotating/whirl/state/arnoldi.rs` (one option field and its builder, one call in `solve`, `residual` split into a wrapper over a
matrix-level function with the same arithmetic, `mod refine;`) and a new child module `rotating/whirl/state/arnoldi/refine.rs`
(the band LU, the clustering, the inverse iteration, and the unit test K2). Tests: a new file
`crates/rcfd-fem/tests/si_rotating_whirl_state_refine.rs`. No existing test is edited or retired.

## What F-520 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Each mode returned by the sparse whirl state solver of ADR
> 0428 is refined by inverse iteration at its own eigenvalue, so that on the cube of ADR 0416 at both registered shifts with count
> 6 every stored mode residual is at most 1e-8, while the eigenvalues still equal the dense state solver's within 1e-9 relative
> and the stability flags agree.

Clauses: (c1) every limit not stated there registered here before the run; (c2) each returned mode refined by inverse iteration
at its own eigenvalue; (c3) on the cube of ADR 0416 (CBI at 1.2, 1.5, 2.0 `omega_0`, CBA at 1.05, 1.1, 1.18, 1.3 `omega_0`, as
ADR 0428's set-up), at `sigma_a` and `sigma_b`, count 6, every stored mode residual `<= 1e-8`; (c4) the eigenvalues equal the
dense state solver's within 1e-9 relative (by ADR 0428's reading: the returned set against the dense set ranked by `|nu|`,
greedy one-to-one matching, relative to each value); (c5) the stability flags agree. The brief adds: (c6) derive how `Q(s)` at a
complex `s` is factored; (c7) how clustered modes are kept apart, with a registered check on a cluster; (c8) whether refinement
is always on or opt-in, the F-519 outputs (eigenvalues) bit-identical; (c9) every other F-519 check passes unedited.

## What was read (branch base b032cbf0, read only)

- `AGENT_RULES.md`; the brief; ADR 0428 (all, with both amendments, the addendum and the remaining questions); ADR 0416 D4 (the
  inverse iteration) through the code.
- `crates/rcfd-fem/src/rotating/whirl/state/arnoldi.rs` (all), `crates/rcfd-fem/src/rotating/whirl/state.rs` (lines 1 to 40,
  380 to 604: `cmul`, `cdiv`, `pencil`, `null_vector`, `normalise`, `state_residual`, `state_solve`,
  `INVERSE_ITERATIONS = 3`), `crates/rcfd-linalg/src/reorder.rs` (the signatures of `rcm_order`, `inverse_permutation`),
  `crates/rcfd-linalg/src/skyline.rs` (the public function list).
- `crates/rcfd-fem/tests/si_rotating_whirl_state_sparse.rs` (lines 1 to 245 and 425 to 630: `Checks`, `compare`, `matching`,
  `whirl_dev`, `cube`, the known-miss record and its amended copy, N2, N3).
- `f519-scratch/run1.log` (the lines of N2 and N3) and `f519-scratch/diag1_residuals.log` (the mode residuals of every cube
  solve of ADR 0428 run 1).

## Design

Notation of ADR 0416 and 0428: `K` (= `K_eff`) reduced, symmetric, any inertia; `M` symmetric positive definite; `G` the skew
part used by ADR 0428 (`op.g`); `Q(s) = K + s G + s^2 M`; a returned mode `(rho, phi)` with `rho` the canonical root of ADR 0428
D2 and `phi` its D3 mode (smoothed Ritz vector, `u + v / rho`, normalised).

### D1. Why refine at the mode's own eigenvalue (c2; derived here)

ADR 0428 amendment 2 diagnosed the far-mode defect: the smoothing step is one inverse iteration **at the shift**; for a mode far
from it, it amplifies the near-shift contamination by `|nu_near / nu_far|` (up to `~5e5` on the cube). Inverse iteration at the
mode's own eigenvalue `rho` (Wielandt; Wilkinson, *The Algebraic Eigenvalue Problem*, 1965, chapter 9; Peters and Wilkinson
1979, "Inverse iteration, ill-conditioned equations and Newton's method", SIAM Rev. 21; all cited from memory, only the classical
argument below is used) removes it: write the start vector `x = sum_k a_k phi_k` over the eigenvectors of the quadratic problem
(the state form of ADR 0416, `x_k = [phi_k; s_k phi_k]`, is diagonalisable for simple `s_k`; the `n`-vector solve
`Q(rho)^-1 b` is the first block of `(C - rho)^-1` applied to a state vector built from `b`, by the block elimination of ADR
0428 D1, so its components along `phi_k` are multiplied by `~1 / (s_k - rho)` times a factor bounded by the eigenvector
condition). With `|rho - s_j| = epsilon_j` (the eigenvalue error of the returned `rho`, `<= 1.6e-10 |rho|` on the cube by ADR
0428 E1) and the distance `d_k = |s_k - rho|` to every other eigenvalue, one solve multiplies the contamination ratio
`a_k / a_j` by `~epsilon_j / d_k`. On the cube's far cluster (relative gaps `>= 1.0e-2`, arithmetic 1) one solve gives `<=
1.6e-8`, three solves `<= 4e-24`; the near-shift modes (`d_k ~ |rho|`) are removed after one solve.

**The residual floor.** The stored eigenvalue is not changed (D4), so the residual of the refined vector is limited by the
eigenvalue error: for the exact eigenvector `phi_j`, `Q(rho) phi_j = Q(rho) phi_j - Q(s_j) phi_j = (rho - s_j) ((rho + s_j) M +
G) phi_j`, and with the residual of ADR 0428 D3, `||Q(rho) phi|| / (||K phi|| + |rho|^2 ||M phi|| + |rho| ||G phi||)`, at an
eigenvector `||K phi|| ~ |rho|^2 ||M phi||` for the elastic modes, so the floor is `~ epsilon_j |2 rho| ||M phi|| / (2 |rho|^2
||M phi||) = epsilon_j / |rho|`, the relative eigenvalue error (`<= 1.6e-10`). The rounding of the solve adds `~ n eps g_LU`
(Wilkinson's argument: a backward-stable solve of a nearly singular system yields a vector of small relative residual whatever
the conditioning; `g_LU` the growth factor of the pivoted factor), `~ 16 x 2.2e-16 x 10 ~ 4e-14` on the cube.

### D2. The factor of `Q(rho)` at a complex `rho` (c6)

`Q(rho)` is complex. It is Hermitian only for an imaginary `rho` (`Q(i w)^H = K - i w G^T - w^2 M = Q(i w)`, since `G^T = -G`);
for a real `rho` (a divergence of CBA) it is real and unsymmetric (`Q(rho)^T = Q(-rho)`), and for a complex `rho` (a flutter
quartet, the RND problems of ADR 0428) neither. ADR 0428 D1 turned the shift into a real symmetric `R` only because its shift is
imaginary; at a general `rho` no symmetric real form exists (the real form `[[Re Q, -Im Q], [Im Q, Re Q]]` has the skew parts
`Re(rho) G` on its diagonal blocks), so the skyline `L D L^T` of `rcfd-linalg` does not apply, and an unpivoted factor would
also be at its weakest here, at a matrix singular to the eigenvalue error. Taken instead (written in `refine.rs`, in `rcfd-fem`
so that no shared linear-algebra crate is edited): **a complex LU with partial pivoting on the band of the reverse Cuthill-McKee
order** (the band form of Gaussian elimination with row interchanges, as in LAPACK `xGBTRF`; Golub and Van Loan, *Matrix
Computations*, section 4.3, cited from memory; derived here):

- The pattern is the union of the patterns of `K`, `M` and `G` (symmetric, since `K` and `M` are symmetric and `G` exactly skew),
  ordered by `rcfd_linalg::rcm_order` on its face list (pairs `r < c`); `p` the lower = upper bandwidth in that order.
- Each row is stored as a window of columns `[lo, hi]`. At step `j` the pivot is the entry of largest modulus `|re| hypot |im|`
  in column `j` among the rows `j .. min(n, j + p + 1)` (the only rows that can have an entry there: a row `i` of the band starts
  at `i - p` or later, and row interchanges only exchange rows of that window; the first such row on a tie). Rows are swapped
  whole. A pivot of modulus below `eps max|Q|` is replaced by `eps max|Q|` (exactly ADR 0416 D4's rule, so that the factor at
  an eigenvalue exists; the solve is then dominated by the wanted null direction, which is the purpose). The multipliers of step
  `j` are kept with their row indices; eliminating extends a row's window to the pivot row's `hi`, so the upper part has width
  at most `2 p` (the standard fill bound of banded partial pivoting).
- The solve applies the same interchanges and multipliers to the right-hand side in the order of the factor, then back
  substitution on the upper part, then the inverse of the RCM order.

Cost: `~ n p (2 p)` complex multiply-adds per factor, one factor per returned mode (each at its own `rho`), three solves each. The
cube: `n = 16`, `p <= 15`: microseconds.

### D3. The iteration and the clusters (c2, c7)

**Clusters.** Two returned modes are in one cluster if `|rho_a - rho_b| <= r_c max(|rho_a|, |rho_b|)`, `r_c = 1e-9`, closed
transitively; the members of a cluster are processed in the order of the returned modes before the sort of ADR 0428 (the Ritz
selection order; deterministic).

Why `r_c = 1e-9` (derived): single-vector inverse iteration fails in a cluster in two ways. (i) Two returned modes of one
eigenvalue (a double beyond `+/- s`, such as the C4-symmetric cube without Coriolis, arithmetic 2) are found by ADR 0428 with
Ritz vectors computed by `null_vector` on `H_m - nu I` from the same start vector at nearly the same `nu`, so they are nearly the
same vector, and inverse iteration at nearly the same `rho` keeps them so: two copies of one direction, each a valid residual,
the eigenspace not spanned. (ii) Two eigenvalues closer than the eigenvalue error `epsilon` cannot be separated by the shift.
Both need `|rho_a - rho_b| <~ 1e3 epsilon` (below that, the per-solve ratio `epsilon / d` is above `1e-3`); with `epsilon <=
1.6e-10 |rho|`, the radius `1e-9` covers both (a pair at relative distance above `1e-9` separates by `<= 0.16` per solve, `<=
4e-3` after three). Within a cluster, **Gram-Schmidt in the `M` product** against the members already refined, applied twice
after every solve (Peters and Wilkinson's remedy for close eigenvalues, as in LAPACK `xSTEIN`, which reorthogonalises within
clusters; cited from memory). The cost of the remedy on the residual: a member made `M`-orthogonal is `phi_b - alpha phi_a`,
whose residual at `rho_b` is `|alpha| ||Q(rho_b) phi_a||`, `~ |alpha| 2 |rho_b - rho_a| / |rho| <= 2 r_c = 2e-9` relative for
`|alpha| <= 1` (M-normalised vectors) -- under the 1e-8 limit, which is why `r_c` is not larger. For an exact double it is 0
(any vector of the eigenspace is an eigenvector).

**The iteration** (for each returned mode, in the Ritz selection order): start from the F-519 mode vector `phi` (re, im) as
computed by ADR 0428 D3 (normalised); factor `Q(rho)` (D2); three times (`INVERSE_ITERATIONS = 3` of ADR 0416 D4, the same
count): `y = Q(rho)^-1 x` (the ADR 0416 iteration: the previous iterate as the right-hand side), the cluster Gram-Schmidt
(`y -= (phi_c^H M y) phi_c` for each earlier refined member `phi_c` of the cluster, the pass applied twice), `x = y / ||y||_2`.
Then the normalisation of ADR 0428 D3 (`phi^H M phi = 1`, the entry of largest modulus real and positive) and the stored
residual of ADR 0428 D3 at the unchanged `rho`. The whirl number of a part's or sector's mode is computed from the refined
vector by the unchanged code of ADR 0428. A refined vector with a non-finite entry refuses the solve (`FemError::Solver`,
`"sparse_state_whirl"`, "the refinement of the mode at s"); it cannot occur with the pivot rule above unless a value overflows.

### D4. Opt-in, and what stays bit-identical (c8)

`SparseStateOptions` gets the field `refine: bool` (default `false`) and the builder `.refine(on)`. With `refine = false`
nothing new runs and every output of ADR 0428 is computed by the same code as before (the only edit on that path is the split of
`residual` into a wrapper over a matrix-level function with the identical sequence of floating-point operations). With `refine =
true` the Arnoldi run, the eigenvalues, their order, `stability`, `real_defect`, `steps`, `isotropy`, `loss` and `unknowns` are
computed before and independently of the refinement and are therefore bit-identical to `refine = false`; only the mode vectors,
their residuals and (on parts and sectors) their whirl numbers and directions change. The refinement does not update the
eigenvalues (no Rayleigh quotient): (c4) and (c5) then hold exactly as in ADR 0428's run. Opt-in is taken by house rule 3 (new
behaviour opt-in) and on the maintainer's standing instruction of 2026-11-11 (take the recommendation); whether it should become
the default is a remaining question for the addendum.

## Set-up (fixed now)

The cube exactly as ADR 0428 (copied: `cube`, `RotatingOptions::new(12).coriolis(true)`, CBI `W0 = W0`, CBA `wy = 1.2 W0`;
speeds and shifts `sigma_a = (0.25 omega_0)^2`, `sigma_b = (2.7 omega_0)^2`, count 6), each solve with
`SparseStateOptions::new(sigma, 6).refine(true)` and once more with `refine(false)` (for B1); the dense reference
`part.whirl_state(w)`. **N3C** (the cluster set-up): CBI at `1.5 omega_0` with `coriolis(false)` (ADR 0428 N3), both shifts,
count 6, `refine(true)`. **PM / RND** exactly as ADR 0428 (PMI 1.5, PMA 1.1 `omega_0` at `sigma_a`, count 2; RND `n = 6, 12,
30, 60`, counts `2, 3, 5, 8`, seed 19, `sigma = 1`), `refine(true)` and `refine(false)`. **K2** (unit test in `refine.rs`): a
`4 x 4` problem from a linear congruential sequence (`K1` symmetric indefinite: diagonal `2 u + 0.3`, off-diagonals `u` with `u`
uniform in `[-0.5, 0.5]`; `M1` diagonal `1 + 0.2 u`, off-diagonal `0.1 u`; `G1` skew with entries `2 u`; seed `0x5eed_0520`),
its four reported dense eigenvalues `s` by `gyroscopic_state_eigen_dense`; **K2a** the `8 x 8` block-diagonal `blockdiag(P1, P1)`
(every eigenvalue exactly double): for each `s`, two modes at the same `rho = s` given the same start vector (entries
`1 + ((7 i + 3) mod 11) / 11 + i (((5 i + 1) mod 13) / 13 - 1/2)`, ADR 0416's start); **K2b** `blockdiag(P1, P1')` with `K1' =
(1 + 1e-3) K1` (the same `M1`, `G1`): for each `s` of block 1, the nearest dense eigenvalue `s'` of block 2 (relative distance
of order `5e-4`, printed), two modes at `s` and `s'` from the same start vector.

## Arithmetic (done before the code, by hand; disclosed)

1. **The cube's far cluster** (from `diag1_residuals.log`, ADR 0428 run 1): CBI 1.2 `sigma_a`: the three elastic modes at
   107256.47, 108478.81, 109656.47 rad/s (relative gaps 1.13e-2 and 1.09e-2), residuals 8.27e-8, 1.45e-8, 8.47e-8; at `sigma_b`
   the same modes with residuals 6.7e-11, 1.7e-11, 2.3e-11. CBA 1.05 `sigma_a`: 107408.57, 108488.25, 109508.58 (gaps 1.0e-2,
   0.94e-2), residuals 6.3e-8, 7.9e-9, 6.5e-8. Every near-shift mode `<= 1.1e-10` (the divergence 59.77 at CBA 1.18 `sigma_b`,
   1.10e-10). The returned far modes are the three lowest elastic modes; the dense solver reports the lowest 12 of the 16
   modes with `Im s > 0` (`RotatingOptions::new(12)`), so they have dense counterparts (K1).
2. **The cluster set-up N3C** (from `run1.log`, ADR 0428 N3, CBI 1.5 `omega_0` without Coriolis, `sigma_a`): the returned modes
   `(1118.033988749727, 0)`, `(1118.0339887497862, 0)` (relative difference 5.3e-14), `(0, 1732.0177)`, `(0, 108448.33620838329)`,
   `(0, 108448.3362083863)` (2.8e-14), `(0, 108486.3656)`: two exact doubles (the C4 symmetry of the cube with isotropic springs
   and no Coriolis), each a cluster by `r_c = 1e-9`; the other two modes are singletons (gap 3.5e-4 between 108448 and 108486,
   above `r_c`, so single-vector separation is exercised there too). `sigma_b` was not printed; the doubles are a property of
   the problem, so they are expected there too if the count brings them in (the number of clusters is printed; K3 requires at
   least one cluster over the two shifts).
3. **Estimates.** Q1R: the floor of D1, `max epsilon_j / |rho| <= 1.6e-10` (ADR 0428 E1 bounds it, since the dense solver's own
   error is smaller, `<= 2e-12` by ADR 0428 arithmetic 3), plus rounding `~4e-14`: estimate `<= 2e-10` against `1e-8`. K1 (the
   angle to the dense vector): `sin theta <~ (residual_sparse + residual_dense) / relative gap <= (2e-10 + 1e-11) / 1e-2 =
   2e-8`, `1 - |cos theta| ~ theta^2 / 2 <= 2e-16`; limit `1e-8`, which a vector of the neighbouring mode fails by `~1` (distinct
   eigenvectors of the cube are not parallel). K2a: Gram-Schmidt twice leaves `|phi_a^H M phi_b| ~ eps`; limit `1e-6`; residuals
   rounding, `~1e-14` (the dense eigenvalue of a `4 x 4` is accurate to `~1e-15` relative). K2b: own-block contamination after
   three solves `(1e-15 / 5e-4)^3`, the cross product `|phi^H M phi'|` at rounding; limit `1e-6`. W1R: as ADR 0428 W1 (`<=
   1e-6`), the refined vectors only closer to the dense ones.
4. **Cost.** Cube, N3C, PM: microseconds per factor. RND `n = 60` with bandwidth `<= 59` (dense): `60 x 59 x 118 ~ 4e5`
   complex operations per factor, 16 modes: negligible. The whole new test file well under 10 s in release.

## Checks (limits fixed now)

| # | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| A1 | c1 | every limit of this table and of the controls fixed in this file, committed alone before any code | holds | the commit order |
| Q1R | c2, c3 | cube, CBI and CBA at their 7 speeds, `sigma_a` and `sigma_b`, count 6, `refine(true)`: the stored residual of every returned mode | `<= 1e-8` | `<= 2e-10` (arithmetic 3) |
| E1R | c4 | the same 14 solves: the returned eigenvalues against the dense state solver's by ADR 0428's reading (boundary valid, one-to-one, relative to each value) | `<= 1e-9` | `1.59e-10` (ADR 0428 E1, identical by B1) |
| F1R | c5 | the same 14 solves: the sparse flag equals the dense flag, and equals ADR 0416 AN3's (`Stable` at CBI 1.2, 1.5, 2.0 and CBA 1.3; `Unstable` at CBA 1.05, 1.1, 1.18) | equal, each | B1 |
| B1 | c8 | every solve of Q1R, N3C, PM and RND: `eigenvalues`, `stability`, `real_defect`, `steps`, `isotropy` (and on the cube `loss`, `unknowns`) of `refine(true)` bitwise equal to `refine(false)` | equal, each | D4 |
| B2 | c8, c9 | default unchanged: the five tests of `si_rotating_whirl_state_sparse.rs` that are not ignored pass unedited; the ignored record `e1_f1_l1_w1_q1_i1_n2_n3_cube_known_miss`, run once with `--ignored`, still prints `Q1 cube residuals: 8.466e-8` and fails as recorded | pass; the same printed value | D4 |
| K1 | c2, c7 | each refined cube mode of the 14 solves whose eigenvalue matches a dense reported mode within `1e-8` relative and whose dense eigenvalue has a relative gap `> 1e-6` to every other dense eigenvalue: `1 - |phi_s^H phi_d| / (||phi_s|| ||phi_d||)` (Euclidean, on the model's equations); the number compared printed, and the three far modes of every solve among them | `<= 1e-8`; `>= 3` far modes per solve | `<= 2e-16` |
| W1R | D3 | the whirl numbers of the refined cube modes against the dense ones by ADR 0428's W1 rule (match `1e-8`, gap `> 1e-3`), the number compared printed | `<= 1e-6` | `<= 2e-11` |
| K2a | c7 | K2a: for each of the four doubles, both refined residuals, and `|phi_a^H M phi_b|` (both M-normalised) | `<= 1e-8`; `<= 1e-6` | `~1e-14`; `~eps` |
| K2b | c7 | K2b: for each of the four near pairs (no Gram-Schmidt: relative distance `> r_c`), both residuals and `|phi^H M phi'|` | `<= 1e-8`; `<= 1e-6` | rounding |
| K3 | c7 | N3C at both shifts: every stored residual; for every pair of a cluster of two or more returned modes (by `r_c`), `|phi_a^H phi_b| / (||phi_a|| ||phi_b||)` (Euclidean, the full mode vectors); the number of clusters printed | `<= 1e-8`; `<= 1 - 1e-3`; at least 1 cluster over the two shifts | arithmetic 2; below |
| Q2 | c2 | PM (PMI, PMA) and RND (4 problems, real, imaginary and complex `rho`) with `refine(true)`: every stored residual | `<= 1e-8` | `~1e-13` |
| R2 | c9 | `cargo test -p rcfd-fem --release --test si_rotating_whirl_state_sparse --test si_rotating_whirl_state_refine` and the `rotating::whirl::state` unit tests pass; `cargo check -p rcfd-post --tests` | pass | additive edits |

K3's product: the integration test has the modes on the model's equations and not the reduced mass, and the Gram-Schmidt of D3
makes the reduced modes `M`-orthogonal, not Euclidean-orthogonal. So K3 measures independence: two `M`-orthogonal vectors have a
Euclidean cosine at most `(kappa - 1) / (kappa + 1)` with `kappa` the condition of `M` (Wielandt's inequality, cited from
memory; `kappa` of a consistent Hex8 mass is a small number, `< 10`, giving `< 0.82`), while ADR 0428's unrefined copies, from
one Ritz direction, are expected near `1`. Two vectors that both have residual `<= 1e-8` at one double eigenvalue and are not
parallel span its eigenspace. K2a covers the `M`-orthogonality itself. Also printed (not asserted): the same cosine for the
unrefined modes of the same solves.

Negative controls (expected to fail the property, each with a limit):

| # | Control | Expected |
|---|---|---|
| N1R | the cube solves of Q1R with `refine(false)` (the F-519 vectors) | max residual `> 1e-8` (ADR 0428 run 1: 8.466e-8) |
| N2R | CBI 1.2 `sigma_a`, `refine(true)`: K1's cosine of each of the three far refined modes against the dense vector of each of the other two far modes (its cluster neighbours) | `1 - |cos| > 1e-3` for each of the 6 pairs (K1 can tell a neighbour's vector from the own one) |

Planned mutations (on a `git archive` copy in `f520-scratch`, each run once against the new test file and the unit test; what
catches each is recorded):

1. The refinement skipped (`refine(true)` does nothing): Q1R (8.466e-8), K3 (the unrefined copies parallel).
2. The cluster Gram-Schmidt removed: K2a (the two copies identical, `|phi_a^H M phi_b| ~ 1`), K3.
3. The factor at `-rho` instead of `rho`: Q1R, Q2, K1, K2a, K2b.
4. The `s G` term dropped from `Q(rho)` in the refinement: Q1R (Coriolis on), Q2, K1, K2a, K2b.
5. The Gram-Schmidt coefficient `phi_c^T M y` (no conjugate): K2a (complex modes of the random `P1`).
6. `refine` forced on whatever the option says: B2 (the record's printed value changes), B1 is unaffected.

## Decision rule

- **done**: A1 to R2 met, N1R and N2R as expected, every planned mutation caught by a registered check, no amendment after a run.
- **partial**: Q1R, E1R and F1R met (the clauses c3 to c5) with another check missed and its cause diagnosed (a
  `*_known_miss` record), or a control not as expected, or a mutation not caught, or a check met only after an amendment made
  after its run.
- **todo**: Q1R, E1R or F1R missed.

## Limits (what is not covered)

The refinement is opt-in; `refine(false)` keeps the F-519 vectors. It is checked on the cube, the point masses, the random
problems and the unit problems only: the disc sector, its full rotor and the ring MP of ADR 0428 are not re-run with
`refine(true)` (the same code path `solve` serves them; their F-519 residuals were already `<= 4.9e-11`). The band LU costs
`~2 n p^2` per mode: on the ring MP (`n = 2112`, an RCM bandwidth of a few hundred) about a second per mode, not measured. The
eigenvalues are not improved by the refinement (no Rayleigh quotient), so the residual floor is the Arnoldi eigenvalue error.
Clusters wider than `r_c = 1e-9` and narrower than the separation of three solves (`~1e-9 .. 1e-8` relative to the eigenvalue
error) are not specially treated. Eigenvalue multiplicities that ADR 0428 does not return (a second copy not found by its Krylov
space) cannot be refined into existence.

## Addendum 1: results (2026-11-14)

**Code.** New: `crates/rcfd-fem/src/rotating/whirl/state/arnoldi/refine.rs` (the band LU with partial pivoting, the clusters,
the inverse iteration, the unit test K2) and `crates/rcfd-fem/tests/si_rotating_whirl_state_refine.rs` (three tests; set-ups
and helpers copied from `si_rotating_whirl_state_sparse.rs`). Edited: `rotating/whirl/state/arnoldi.rs` (the field `refine`,
default `false`, and the builder `.refine(on)`; `mod refine;`; `residual` now a wrapper over `residual_of` with the same
operations; one `if opts.refine` call after the modes are formed). No other file of `rcfd-fem` is touched; no new crate, no
`unsafe`, no hash table or golden edited.

**Runs.** Commit `ca2059ac`, release. K2 (`rotating::whirl::state::arnoldi::refine::tests::k2a_k2b_clusters`) was first run
inside `cargo test --lib rotating::whirl::state` without `--nocapture` (passed, nothing printed), then once more with
`--nocapture` for its numbers (`f520-scratch/run_k2.log`; deterministic, the same code). The integration file once
(`f520-scratch/run1.log`). B2: the F-519 file (`b2_f519_suite.log`) and its ignored record once with `--ignored`
(`b2_known_miss.log`). R2: `r2_lib.log`, `r2_post.log`.

| Check | Limit | Measured | Met |
|---|---|---|---|
| A1 registration first | holds | `2e9325cd` alone before any code | yes |
| **Q1R** cube residuals, refined (14 solves, 84 modes) | `<= 1e-8` | **2.24e-10** (the divergence 99.589 of CBA 1.1 at `sigma_b`; the far elastic modes `<= 8.0e-13`) | yes |
| E1R eigenvalues against the dense state solver | `<= 1e-9` | 1.59e-10 (as ADR 0428 E1, by B1) | yes |
| F1R flags | equal, each; as AN3 | equal; as AN3 | yes |
| B1 bitwise equality but for the modes (cube, N3C, PM, RND) | equal, each | equal in every solve | yes |
| B2 F-519 tests unedited | pass; record prints 8.466e-8 | 4 passed, 1 ignored; the record prints `Q1 cube residuals: 8.466e-8` and fails as recorded | yes |
| K1 own vector (84 modes, 3 far per solve) | `<= 1e-8`; `>= 3` far | 3.3e-16; 3 far in each of the 14 solves | yes |
| W1R whirl numbers (84 modes) | `<= 1e-6` | 1.5e-12 | yes |
| K2a exact doubles (4) | `<= 1e-8`; `<= 1e-6` | residual 1.4e-15; `|phi_a^H M phi_b|` 1.1e-16 | yes |
| K2b near pairs (smallest relative distance 2.8e-4) | `<= 1e-8`; `<= 1e-6` | 1.8e-15; 1.1e-33 | yes |
| K3 N3C clusters | `<= 1e-8`; `<= 1 - 1e-3`; `>= 1` cluster | 6.5e-13; `|cos|` 4.1e-17; 4 clusters (the divergence double and the elastic double at each shift) | yes |
| Q2 PM and RND residuals, refined | `<= 1e-8` | 1.8e-15 | yes |
| R2 the two test files, the `rotating::whirl::state` unit tests; `cargo check -p rcfd-post --tests` | pass | pass (3 + 4 + 4 tests; check clean) | yes |
| N1R unrefined cube residuals | `> 1e-8` | 8.466e-8 | as expected |
| N2R far modes against the neighbours' dense vectors (6 pairs) | `1 - |cos| > 1e-3` | 1.0 (smallest over the 6) | as expected |

**Mutations** (`f520-scratch/mutate.py` on the `git archive` copy `f520-scratch/mut` of `ca2059ac`, each run once,
`mut_1..6.log`, summary `mutations_run.log`): 6 of 6 caught.

| # | Mutation | Caught by |
|---|---|---|
| 1 | refinement skipped | Q1R (8.466e-8) |
| 2 | cluster Gram-Schmidt removed | K2a (`|phi_a^H M phi_b|` 1.0) |
| 3 | factor at `-rho` | Q1R (1.0), K1 (1.0), W1R (2.0), Q2 (1.0), K2a (residual), N2R (0, not as expected) |
| 4 | `s G` dropped from `Q(rho)` | Q1R (0.93), K1 (0.66), W1R (0.82), Q2 (0.93), K2a (residual 0.55) |
| 5 | Gram-Schmidt coefficient without conjugate | K2a (`|phi_a^H M phi_b|` 1.0) |
| 6 | refinement forced on | B2 (the record prints 2.236e-10 and passes), N1R (2.2e-10, not as expected) |

`cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean (both before the run; one
`type` alias for clippy's complex-type lint and the rustfmt layout were applied before commit `ca2059ac`). `baseline_hashes` was
not run: the only shared-path edit is in the F-519 solver, which no case file, CLI or hash-table path calls, and with
`refine(false)` its bytes are unchanged (B2).

### Deviations

- B2's registered text says "the five tests ... that are not ignored"; the file has five tests of which four are not ignored
  and one is the record. All four pass and the record behaves as recorded; the wording was wrong, not the limit.
- The registration expected ADR 0428's unrefined copies in N3C to be nearly parallel (an information print, not asserted):
  they were not (`|cos|` 1.3e-3 and 1.2e-3 for the divergence doubles, 0.72 and 0.22 for the elastic doubles). On the cube the
  Gram-Schmidt is therefore not what keeps them independent; K2a (the same start vector) is where it is exercised. So mutations
  1 and 2 were not caught by K3 as planned; they were caught by Q1R and K2a, both registered.
- Q1R measured 2.24e-10, slightly above the estimate `<= 2e-10`: the floor of D1 was derived for elastic modes (`||K phi|| ~
  |rho|^2 ||M phi||`); for the divergence the denominator is smaller relative to `|(2 rho M + G) phi|`. Within the limit by 45x.
- The RND problems' returned modes are all imaginary at their shift (printed), so a non-imaginary complex `rho` is exercised
  only by K2 (the quartet `+/- 0.646 + 0.135 i`), a real `rho` by PMA and the CBA divergences.
- K2 was run twice (see Runs); nothing was changed between the runs.

### What was not run

The workspace suite (house rules); `baseline_hashes` (reason above); the disc sector, its full rotor and the ring MP with
`refine(true)` (limits); no timing check.

### Decision

**done**, by the decision rule: A1 to R2 met, N1R and N2R as expected, 6 of 6 mutations caught by registered checks, no
amendment. Every refined mode residual on the cube is at most 2.24e-10 (the far elastic modes at most 8.0e-13, against
8.466e-8 unrefined); the eigenvalues and flags are bitwise those of ADR 0428 (E1 1.59e-10, flags as AN3).

### Remaining questions

1. **Default.** Should `refine` become the default of `SparseStateOptions`? Options: (a) keep opt-in; (b) default on (changes
   the default mode vectors of F-519; no hash table or golden is affected). Recommendation: (b) once the maintainer agrees,
   since the refined vectors are the accurate ones and the cost is one band factor per mode; until then (a), as the house rule
   on defaults requires. Not taken: it changes a default result.
2. **Large parts.** The band LU on the ring MP (2112 unknowns) is not measured. Options: measure it in the item that routes
   `WhirlSolver::Auto` to the sparse state solver; add a check now. Recommendation: measure it there.

## For the lead: roadmap entries

**F-520**, proposed status: done.

> ADR 0442, 2026-11-14: opt-in refinement (SparseStateOptions::refine) of every sparse whirl state mode by three inverse iterations on Q(s) at its own eigenvalue, with a complex band LU with partial pivoting on the RCM order and M-Gram-Schmidt within clusters of eigenvalues equal to 1e-9; on the cube at both shifts with count 6 every mode residual at most 2.2e-10 (far elastic modes 8.0e-13, against 8.5e-8 unrefined), eigenvalues and flags bitwise those of ADR 0428 (1.6e-10 against the dense solver, flags equal); exact doubles kept apart; 6 of 6 mutations caught.

**F-519**, status unchanged (partial); new note:

> ADR 0428, 2026-11-14: sparse whirl state solver (isotropic shift-invert Arnoldi on (C^2 + sigma)^-1, one skyline factor with K_eff - sigma M blocks); eigenvalues against the dense state solver within 1.6e-10 on the cube and 4.4e-12 on the disc sector, flags equal; the 2112-unknown ring above the cap matches the dense sector union to 3.3e-13; Q1 of the cube missed (8.5e-8 against 1e-8, far elastic mode vectors, diagnosed); 8 of 8 mutations caught. The far-mode vectors are refined by the opt-in option of F-520 (ADR 0442, 2026-11-14).

No follow-up items.
