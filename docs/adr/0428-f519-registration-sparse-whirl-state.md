# ADR 0428: F-519 registration: a sparse whirl state solver above the loss of stiffness (an isotropic shift-invert Arnoldi process on the real operator `(C^2 + sigma I)^-1` of the state matrix of ADR 0416, one sparse indefinite factor of the real form of `Q(i sqrt(sigma))`, whose diagonal blocks are `K_eff - sigma M`, at a registered shift `sigma > 0`; opt-in)

Status: **REGISTERED 2026-11-14, written before any code of F-519 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-519 was written, compiled or run: the code listed under "What was read" was read and
the arithmetic below was done by hand (no script, no build, no run). The only numbers below that came from a run are those of the
registered runs of ADR 0416, 0420 and 0424 as their addenda print them (disclosed under "Arithmetic"). Results go in the addendum
at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0416](0416-f505-registration-gyroscopic-stabilisation.md) (F-505: the state matrix `A = [[0, c I], [-K^/c,
-G^]]` of `M u_tt + G u_t + K_eff u = 0`, its eigenvalues `s` those of `Q(s) = s^2 M + s G + K_eff`, the spectrum symmetric
about both axes (its D3), the flag `Stable` iff `max|Re s| <= STABILITY_TOLERANCE max|s|` (`1e-8`), otherwise `Unstable {
growth: max Re s }`, the cube on springs CBI / CBA; the dense cap `MAX_WHIRL_UNKNOWNS = 1500` of `whirl_state`), [ADR
0420](0420-f511-registration-sector-whirl-stability.md) (F-511: `RotatingSector::whirl_state` per harmonic index on its real
cos/sin problem, the disc sector DS and its test speed `Omega_T` above the first loss of stiffness by the rule of its D3), [ADR
0427](0427-f506-registration-sparse-whirl-lanczos.md) (F-506: the sparse whirl solver below the loss, the skyline factor reuse,
`WhirlSolver`, the medium ring MP; its amendment 1: a negative control whose premise was wrong, because a single Krylov run on a
spread spectrum finds the second copies of a multiple eigenvalue by rounding; its remaining question 2 (b) and the follow-up item
taken here). Not changed: [ADR 0424](0424-f510-registration-odd-index-instability.md) (`with_nodal_springs`). Roadmap item F-519
(deps F-506, F-505). Number 0428 given by the lead. Branch `f519-sparse-whirl-state` (base `0ef93493`), worktree
`w49i-work/f519`, scratch `w49i-work/f519-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, the hash tables or a golden file; no new dependency, no unsafe code; clean-room.

Library edits, additive only. A new file `crates/rcfd-fem/src/rotating/whirl/state/arnoldi.rs`, a child module of
`rotating::whirl::state` (so that it reads the private `hessenberg_eigenvalues`, `null_vector`, `Stability`,
`STABILITY_TOLERANCE`, `StateWhirlMode` of `state.rs`, the private helpers `inertia`, `dot`, `doubled`, `speed_refused`,
`mass_refused` of `whirl.rs`, and the private fields of `RotatingPart` and `RotatingSector`). In `state.rs`: one line `mod
arnoldi;` and one `pub use arnoldi::{...};`. In `whirl.rs`: one `pub use state::{...}` line. In `rotating.rs`: one `pub use
whirl::{...}` line and one paragraph of the module comment. No existing function changes what it computes: `whirl_state`,
`whirl_stability`, `whirl_modes`, `WhirlSolver` and everything of ADR 0411 to 0427 are untouched and do not call the new code;
the new solver is reached only through the new functions (opt-in by calling them). Tests: a new file
`crates/rcfd-fem/tests/si_rotating_whirl_state_sparse.rs`. No existing test is edited or retired.

## What F-519 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Above the loss of stiffness, the whirl state of a part
> above the dense cap by a shift-invert Arnoldi process on the state matrix of ADR 0416 with a sparse factor of K_eff - sigma M
> at a registered shift; on the cube of ADR 0416 and the disc sector of ADR 0420 the eigenvalues equal the dense state solver's
> within 1e-9 relative and the stability flags agree.

Clauses: (c1) every limit not stated there registered here before the run; (c2) above the loss of stiffness, the whirl state of a
part above the dense cap (more than 1500 reduced unknowns) by a shift-invert Arnoldi process on the state matrix of ADR 0416;
(c3) with a sparse factor of `K_eff - sigma M` at a registered shift; (c4) on the cube of ADR 0416 the eigenvalues equal the
dense state solver's within 1e-9 relative and the stability flags agree; (c5) the same on the disc sector of ADR 0420. The brief
adds: (c6) derive the shift-invert of the quadratic pencil using only solves with `K_eff - sigma M` or the quadratic shift
`Q(sigma)`, cite the Arnoldi and implicit-restart sources; (c7) `K_eff - sigma M` may be indefinite: register how it is factored
and what happens when the factor without pivoting fails; (c8) defaults bit-identical (`Dense` stays the default), the flag rule
identical to ADR 0416.

Readings (registered now).

- "The eigenvalues" (c4, c5). A sparse solver returns the eigenvalues nearest its shift, not all `2 n`. The clause is read on the
  returned set: the solver returns the `2 p` eigenvalues `s` (`p >= count` Ritz values, each giving `+/- s`, D2) with the largest
  `|nu(s)|`, `nu(s) = 1 / (s^2 + sigma)`; the dense state solver's `2 n` eigenvalues are ranked by the same `|nu|` and its first
  `2 p` form the reference set; the two sets are matched one to one by the global greedy rule of ADR 0420 (all distances in
  increasing order, ties by index), and "within 1e-9 relative" is `max |s_sparse - s_dense| / |s_dense| <= 1e-9` over the matched
  pairs (relative to each value, the stricter reading of ADR 0416 and 0420). The comparison is valid only when the dense ranking
  has a gap at its boundary, `|nu_(2p)| > (1 + 1e-6) |nu_(2p+1)|` (a near tie would make the reference set ambiguous); an invalid
  boundary is a defective set-up (printed, an amendment before any corrected run).
- "The stability flags agree" (c4, c5): the sparse flag is the rule of ADR 0416 D3 applied to the returned set (`Stable` iff
  `max |Re s| <= STABILITY_TOLERANCE max |s|` over it, otherwise `Unstable { growth: max Re s }`, the same constant and code
  shape), and it must equal the dense flag (on all `2 n`): the same variant, and if unstable the growth rates within 1e-9
  relative (covered by the eigenvalue check, printed separately). The sparse flag certifies only the returned set (limits); the
  set-ups below are chosen, by arithmetic 2, so that every eigenvalue with `Re s != 0` is in it.
- "Above the loss of stiffness" (c2): every registered speed of the cube, the sector and the ring has `K_eff` not positive
  definite on at least one of the solved problems (the inertia test of ADR 0403 D3, printed as `loss`), except the speeds where
  the set-up says otherwise (none of the checks below).
- "A part above the dense cap" (c2): the ring MP of ADR 0427 (2112 reduced unknowns) at a speed above its first loss of stiffness,
  checked against the union of the dense state solutions of its sector indices (ADR 0420 D2, which showed the union equal to the
  full rotor within 6.7e-14).

## What was read (branch base 0ef93493, read only)

- `AGENT_RULES.md`; the brief; ADR 0416 (all, with amendment 1 and the addendum), ADR 0420 (all), ADR 0427 (all, with amendment
  1), ADR 0424 (set-up and addendum table).
- `crates/rcfd-fem/src/rotating/whirl/state.rs` (all: `hessenberg`, `hessenberg_eigenvalues`, `francis`, `eig2`,
  `null_vector`, `normalise`, `state_residual`, `state_solve` with its sign convention of `G^`, `gyroscopic_state_eigen_dense`,
  `whirl_state_at`), `state/sector.rs` (all), `state/springs.rs` (all), `crates/rcfd-fem/src/rotating/whirl/sparse.rs` (all),
  `crates/rcfd-fem/src/rotating/whirl.rs` (lines 1 to 60 and the function list: `dot`, `inertia`, `doubled`, `check_size`,
  `dense_of`, `whirl_number`), `crates/rcfd-fem/src/rotating.rs` (lines 1 to 70).
- `crates/rcfd-linalg/src/skyline.rs` (lines 1 to 60 and 280 to 445: `PivotRule::{Positive, NonZero}`, the pivot test `theta_i =
  (w + 1) eps (|a_ii| + sum |u l|)`, `factor_with`, `solve`), `crates/rcfd-linalg/src/matrix.rs` (`CsrMatrix`, `from_triplets`
  sums duplicates in input order, `transpose`, `matvec`, `diag`). There is no unsymmetric sparse factor and no complex type in the
  repository.
- Tests: `si_rotating_whirl_state.rs` (`Checks`, `cube`, `options`, `closed`, `point_mass`, `an3_anisotropic_cube_band`),
  `si_rotating_whirl_sector_state.rs` (lines 1 to 420: DS, `omega_ref`, `lost`, `matching`, `flag_of`, the D3 rule of ADR 0420),
  `si_rotating_whirl_sparse.rs` (the ring MP: `sector_model`, `full_of`, `modal`, `RING_DIV`).

## Design

Notation of ADR 0416: `K = K_eff(Omega)` reduced, symmetric, of any inertia; `M` reduced, symmetric positive definite; `G = 2
Omega C` reduced, skew (its skew part `G_s = Omega (C - C^T)` is used, as ADR 0427 D4); `n` the reduced unknowns; `M u_tt + G u_t
+ K u = 0`, `u = phi e^{s t}`, `Q(s) phi = 0`, `Q(s) = s^2 M + s G + K`.

### D1. Why the shift is put on the imaginary axis, and the real operator that needs only one symmetric factor (c3, c6; derived here)

The state form of ADR 0416 in the original coordinates (no Cholesky factor of `M`): `x = [u; v]`, `v = u_t`, `x_t = C x`, `C =
[[0, I], [-M^-1 K, -M^-1 G]]` (the matrix `A` of ADR 0416 is `C` in the coordinates `y = L^T u`, scaled; same eigenvalues). A
shift-invert at a point `s_0` needs a solve with `C - s_0 I`, which by block elimination (the second block row: `-M^-1 K p - M^-1
G q - s_0 q`, with `q = f + s_0 p` from the first) is one solve with `Q(s_0) = K + s_0 G + s_0^2 M`. For a real `s_0 != 0`
this matrix is real and **not symmetric** (`s_0 G` is its skew part), and the repository has no unsymmetric sparse factor. On
the imaginary axis, `s_0 = i w_0`, `Q(i w_0) = (K - w_0^2 M) + i w_0 G` is **complex Hermitian** (`i G` is Hermitian for real
skew `G`): its diagonal part is `K - sigma M` with `sigma = w_0^2`, which is the "sparse factor of `K_eff - sigma M` at a
registered shift" of the acceptance, generalised to `G != 0` (at `G = 0` it is exactly that factor, D1 (iii)).

The complex shift is turned into a real operator by pairing it with its conjugate (Parlett and Saad 1987, "Complex shift and
invert strategies for real matrices", Linear Algebra Appl. 88/89; cited from memory, only the idea that `(C - s_0)^-1 (C - conj
s_0)^-1` is real is taken):

`P = (C - i w_0)^-1 (C + i w_0)^-1 = (C^2 + sigma I)^-1`, `sigma = w_0^2 > 0`,

real, with the eigenvectors of `C` and the eigenvalues **`nu(s) = 1 / (s^2 + sigma)`**. The largest `|nu|` belong to the `s`
nearest `+/- i sqrt(sigma)`; the divergence `s = +/- gamma` (real) has `nu = 1/(gamma^2 + sigma) > 0`, a stable `s = +/- i omega`
has `nu = 1/(sigma - omega^2)`.

(i) **One solve per application (derived).** `(C^2 + sigma) [p; q] = [f; g]`: with `r = -M^-1 (K p + G q)` (the second block of
`C [p; q]`), `C^2 [p; q] = [r; -M^-1 (K q + G r)]`. The first block row `r + sigma p = f` gives `(K - sigma M) p + G q = -M f`.
The second, `-M^-1 (K q + G r) + sigma q = g` with `r = f - sigma p`, gives `(K - sigma M) q - sigma G p = -M g - G f`. So

`[[K - sigma M, G], [-sigma G, K - sigma M]] [p; q] = [-M f; -M g - G f]`,

no solve with `M`. (ii) **Scaled, it is symmetric.** In the scaled state `x~ = [c u; v]`, `c = sqrt(sigma)` (D3), `P~ = D P D^-1`,
`D = blockdiag(c I, I)`: with `x~ = [f~; g~]` (`f = f~/c`, `g = g~`) and the result `[c p; q]`, multiplying the first row by `c`
and substituting:

`R [c p; q] = [-M f~; -M g~ - G f~ / c]`, `R = [[K - sigma M, c G], [-c G, K - sigma M]]` (`c = sqrt(sigma)`).

Check of the second row: `-c G (c p) + (K - sigma M) q = -sigma G p + (K - sigma M) q = -M g - G f = -M g~ - G f~/c`. `R` is
**real symmetric** (`(c G)^T = -c G`), order `2 n`, and it is the real form of `Q(i sqrt(sigma))` (for `z = a + i b`,
`Q(i w_0) z = 0` iff `[[K - sigma M, -w_0 G], [w_0 G, K - sigma M]] [a; b] = 0`, the same matrix with `b -> -b`), so **`R` is
singular iff `+/- i sqrt(sigma)` is an eigenvalue** (D4). (iii) At `G = 0`, `R = blockdiag(K - sigma M, K - sigma M)` and
`P~ [f~; g~] = -[(K - sigma M)^-1 M f~; (K - sigma M)^-1 M g~]`: the classical shift-invert `(K - sigma M)^-1 M` of Ericsson
and Ruhe (1980, Math. Comp. 35, cited from memory) on each block, with `nu = 1 / (sigma - lambda)`, `lambda = -s^2 = omega^2`.

`sigma = 0` is not offered (refused, key `"shift"`): `R` would be `blockdiag(K, K)`, singular exactly at the loss of stiffness
(`s = 0`), the region this solver is for, and the scale `c` of D3 would vanish.

### D2. The duplicate `+/- s` and the isotropic Arnoldi process (Mehrmann and Watkins 2001; derived here)

`nu(s) = nu(-s)`: every eigenvalue of `P` is (at least) **double**, its eigenspace spanned by `x_s` and `x_-s` (the spectrum is
symmetric, ADR 0416 D3). A Krylov space of a real start vector sees one vector of each such plane in exact arithmetic; in floating
point the second vector enters by rounding and, on a spread spectrum, is amplified until a duplicate Ritz value appears (the
mechanism ADR 0427 amendment 1 observed for genuine copies). Remedy, after the SHIRA method of Mehrmann and Watkins (2001,
"Structure-preserving methods for computing eigenpairs of large sparse skew-Hamiltonian/Hamiltonian pencils", SIAM J. Sci.
Comput. 22; cited from memory, the form below is derived):

- **The form.** `Omega = [[G, M], [-M, 0]]` is skew and invertible (`det = det(M)^2`), and `Omega C = [[-K, 0], [0, -M]]`
  (`[[G, M], [-M, 0]] [[0, I], [-M^-1 K, -M^-1 G]]`: first row `[-K, G - G]`, second `[0, -M]`) is symmetric. With `w(x, y) = x^T
  Omega y`: `(Omega C)^T = C^T Omega^T = -C^T Omega`, and `Omega C` symmetric gives `C^T Omega = -Omega C`, so `w(C x, y) = x^T
  C^T Omega y = -x^T Omega C y = -w(x, C y)`: `C` is Hamiltonian for `w`, and `C^2` and `P = (C^2 + sigma)^-1` are
  skew-Hamiltonian: `w(P x, y) = w(x, P y)`.
- **Isotropy.** For skew-Hamiltonian `P`, `w(P^i v, P^j v) = w(v, P^(i+j) v) = w(P^(i+j) v, v) = -w(v, P^(i+j) v) = 0`: every
  Krylov space of `P` is isotropic. Eigenvectors: `w(C x_s, x_t) = -w(x_s, C x_t)` gives `(s + t) w(x_s, x_t) = 0`, so `x_s` is
  `w`-orthogonal to every eigenvector except those of `-s`, and since `Omega` is invertible `w(x_s, x_-s) != 0` for a simple `s`:
  **an isotropic space contains at most one direction of each plane `span{x_s, x_-s}`**, and its dimension is at most `n`.
- **Enforced.** In the scaled coordinates the form is `Omega~ = D^-1 Omega D^-1`, used as `Omega^ = c Omega~ = [[G / c, M], [-M,
  0]]` (`Omega^ x~ = [G x~_u / c + M x~_v; -M x~_u]`). At each step `j` the vector `Omega^ q_j` is Gram-Schmidt orthogonalised
  (twice) against the Arnoldi basis `Q` and the set `Y` and, if its norm is above `1e-12` of its norm before, normalised into
  `Y`; the new Arnoldi vector is then orthogonalised against `Q` and `Y`. Since `w(q_i, v) = q_i^T Omega^ v = -(Omega^ q_i)^T v`,
  orthogonality to `Omega^ Q` is `w`-orthogonality to `Q`; in exact arithmetic the Arnoldi vector already has it (isotropy), so
  the projection on `Y` removes rounding only and its coefficients are discarded. The diagnostic `isotropy = max_ij |q_i^T Omega^
  q_j| / max_j ||Omega^ q_j||` is returned (check I1).

**The Arnoldi process** (Arnoldi 1951, Quart. Appl. Math. 9; the shift-invert Arnoldi and its Ritz estimate as in Saad,
*Numerical Methods for Large Eigenvalue Problems*, 1992, chapter 6; cited from memory): `q_1` the normalised start vector (the
linear congruential sequence of ADR 0427 D3, seed `0x5eed_0519`, entries `u - 1/2`, in `x~`); step `j`: `w = P~ q_j` (one solve with
`R`), `Omega^ q_j` into `Y`, `w` orthogonalised against `Q` by classical Gram-Schmidt applied twice with **both passes'
coefficients summed into `H[., j]`**, then against `Y` (twice, discarded); `h_{j+1,j} = ||w||`. **Breakdown** if `h_{j+1,j} <=
1e-13 ||P~ q_j||` (an invariant subspace to rounding: the Ritz pairs are exact). At most `max_steps = min(n, 20 count + 100)`
steps (`n`, the largest isotropic dimension; ADR 0427's `20 count + 100` for the basis size). **No restart.** The implicit restart
of Sorensen (1992, SIAM J. Matrix Anal. Appl. 13; cited from memory) is the standard way to bound the basis; it is not written
here: with `count <= 20` the basis is at most 500 vectors of length `2 n` (at `n = 2112`, 17 MB), and a single run with full
reorthogonalisation is the simplest process whose result does not depend on restart choices (limits).

**Ritz values.** At the check steps (`m = min(2 count + 2, max_steps)`, then every 8, at `max_steps` and at breakdown) the
eigenvalues `nu` of the `m x m` upper Hessenberg `H_m` by `hessenberg_eigenvalues` of ADR 0416 D2 (the Francis double-shift QR,
reused unchanged), ranked by `|nu|` descending (ties: larger `Im nu` first, so that a conjugate pair is adjacent); the first
`count` are selected, and if the last selected has `Im nu > 0` its conjugate (the next) is added (`p` = the number selected). The
eigenvector `y` of `H_m` for each selected `nu` by `null_vector` of ADR 0416 D4 on the pencil `H_m - nu I` (passed as `k = H_m`,
`m = 0`, `g = -I`; three inverse iterations, unit 2-norm). The Ritz estimate `r = h_{m+1,m} |y_m|` (0 at breakdown) is carried to
the eigenvalue `s` by first order (`s^2 = 1/nu - sigma`, `|d s / s| = |d nu| |s^2 + sigma| / (2 |nu| |s|^2)`): the pair is
**converged** when `r |s^2 + sigma| / (2 |nu| |s|^2) <= TOL`, `TOL = 1e-12` (fixed; `|s| = 0` is never converged). The run ends
when all `p` selected are converged; at breakdown or `max_steps` with any selected pair not converged, the solve is refused
(`FemError::Solver`, solver `"sparse_state_whirl"`).

**Eigenvalues from `nu`.** `s^2 = 1/nu - sigma` (complex division), `s = sqrt(s^2)` (principal complex root: `sqrt((|z| + a)/2)`,
`sign(b) sqrt((|z| - a)/2)`, `sign(b) = -1` only for `b < 0`, so a negative real `s^2` gives an exactly imaginary `s`); the
canonical root `rho` is the one of `+/- s` with `Im > 0`, or `Re >= 0` if `Im = 0`. Each selected `nu` contributes `rho` and
`-rho` to the returned eigenvalues (a conjugate pair `nu, conj nu` gives the quartet `+/- rho, +/- conj rho`), ordered by `(Im s,
Re s)` as ADR 0416. The flag and `real_defect` by ADR 0416 D3 on this set.

### D3. The scale and the mode vectors

The scale `c = sqrt(sigma)` balances the two blocks of an eigenvector `x~_s = [c phi; s phi]` for `|s|` near the shift, so that
the eigenvalues near the shift are well conditioned in the Euclidean product of `x~` (ADR 0416 D1 balanced `A` for the same
reason). **Modes**: for each selected `nu` the Ritz vector `x~ = Q y` (complex, real and imaginary parts) is smoothed by one
further application of `P~` to each part (one step of inverse iteration at the shift: an error component along an eigenvector of
eigenvalue `nu_j` is multiplied by `nu_j / nu`, which damps the far, high-frequency components that the quadratic residual would
amplify by `|s_j|^2 / |s|^2`), unscaled (`u = x~_u / c`, `v = x~_v`), and the mode of `rho` is `phi = u + v / rho` (if `x = alpha
x_rho + beta x_-rho` with `x_t = [phi_t; t phi_t]`, then `u + v/rho = 2 alpha phi_rho`); then the normalisation and the residual of
ADR 0416 D4 (`phi^H M phi = 1`, the entry of largest modulus real and positive; `||Q(rho) phi|| / (||K phi|| + |rho|^2 ||M phi|| +
|rho| ||G phi||)`, by sparse products) and, on a part, the whirl number `kappa = 2 r^T C t / (r^T M r + t^T M t)` with the reduced
`C` (as ADR 0427 D4). A degenerate Ritz vector (`alpha ~ 0`) gives a poor mode, seen in its residual (check Q1; limits).

### D4. The indefinite factor (c7)

`R` is assembled in CSR by triplets (`K`, then `-sigma M`, then `c G_s`, `-c G_s`; duplicates summed in that order) and factored
by `SkylineLdlt::factor_with` with reverse Cuthill-McKee on the graph of `R`, **`PivotRule::NonZero`** (ADR 0274 D4: symmetric
indefinite, `L D L^T` **without pivoting**, a pivot refused when `|d_i| <= theta_i = (w + 1) eps (|a_ii| + sum |u_ik l_ik|)`) and
the symmetry tolerance `1e-12`. `R` is indefinite in general (at `G = 0` its inertia is twice that of `K - sigma M`, which has a
negative eigenvalue for every `sigma` above the lowest `lambda`, and `K` itself is indefinite above the loss). **A refused pivot
refuses the solve** with the key `"shift"` and the pivot in the message ("choose another shift"); it is certain when `R` is
singular (`+/- i sqrt(sigma)` an eigenvalue) and possible, without pivoting, at an unlucky leading minor. No symmetric pivoting
(Bunch and Kaufman 1977, Math. Comp. 31, cited from memory) is written: it would be a new factor in `rcfd-linalg` (limits). The
growth of an unpivoted factor is not bounded a priori; its effect is seen a posteriori in the eigenvalue checks and the stored
residuals (Q1). `M` is factored with `PivotRule::Positive` as the test that the problem's mass is positive definite (`"mass"`, as
ADR 0427 D4); the factor of `M` is not applied.

### D5. Interfaces (all new, in `rotating/whirl/state/arnoldi.rs`, re-exported from `rotating`)

- `SparseStateOptions { shift, count, tolerance }` (`#[non_exhaustive]`), `SparseStateOptions::new(shift, count)` (tolerance
  `1e-12`) and `.tolerance(t)`. `shift` is `sigma` in rad^2/s^2 (`sqrt(shift)` the frequency of the shift point).
- `gyroscopic_state_eigen_sparse(k: &CsrMatrix, m: &CsrMatrix, g: &CsrMatrix, opts) -> Result<SparseStateEigen>`: `k` symmetric
  of any inertia, `m` symmetric positive definite, `g` skew (its skew part `(g - g^T)/2` used); `SparseStateEigen { eigenvalues,
  stability, real_defect, growth, omegas, re, im, residuals, steps, isotropy }` (`growth`, `omegas`, `re`, `im`, `residuals` of
  the reported modes `rho`, ordered by `(Im, Re)`).
- `SparseStateWhirl { speed, loss, shift, eigenvalues, stability, real_defect, modes: Vec<StateWhirlMode>, unknowns, steps,
  isotropy }`; `RotatingPart::whirl_state_sparse(omega, &opts)` (the reduction of `whirl_state` repeated: `K_eff(omega)`, `M`,
  `C` reduced on the model's path, the zero-mass refusal, `loss` by the inertia test of ADR 0403 D3; `G = 2 omega C` with
  `options.coriolis`, 0 without; modes expanded to the model's equations); `RotatingSector::whirl_state_sparse(harmonic, omega,
  &opts)` (the reduction of `RotatingSector::whirl_state` repeated: doubled for a pair index, `harmonic_transformation`; modes on
  the sector equations, `multiplicity x n`; no wave sense). No dense matrix is formed and `check_size` is not called on these
  paths.
- Refusals: `shift` not finite or `<= 0`, or a refused pivot of `R` (`"shift"`); `count == 0` or `count > n` (`"modes"`);
  `tolerance` not in `(0, 1)` (`"options"`); matrices of unequal or non-square size or with non-finite entries (`"matrix"`); `m`
  not positive definite (`"mass"`); a negative or non-finite speed (`"speed"`); a harmonic above `N/2` (`"harmonic"`); a run that
  does not converge, or a Francis failure (`FemError::Solver`, `"sparse_state_whirl"` / `"gyroscopic_state_eigen"`).
- Defaults: nothing existing reads `SparseStateOptions` or calls the new functions; `WhirlSolver` and `RotatingOptions` are not
  changed (`Dense` stays the default of the F-506 option, which `whirl_state` does not read; whether it should is a remaining
  question).

## Set-up (fixed now)

Material, axis and the cube exactly as ADR 0416 (the test file copies `cube`, `closed`, `point_mass`): steel, `e_z`, `omega_0 =
1000 rad/s`, `omega_y = 1.2 omega_0`, `m = 7.85 kg`, one Hex8 `h = 0.1`, `u_z = 0`, nodal springs; `RotatingOptions::new(12)
.coriolis(true)` (12 reported dense modes instead of 4, so that W1 has dense modes to compare; the reported count does not enter
`K_eff`, `M`, `C` or the eigenvalues). **CBI** at `1.2, 1.5, 2.0 omega_0`; **CBA** at `1.05, 1.1, 1.18, 1.3 omega_0`. Shifts
**`sigma_a = (0.25 omega_0)^2 = 62 500`** and **`sigma_b = (2.7 omega_0)^2 = 7.29e6`**, `count = 6`, at every speed. (`n = 16`, so
`max_steps = 16`.)

**DS** exactly as ADR 0420 (`ds_mesh`, `ds_sector`, `full_of` copied; `N = 6`), both effects on. `omega_ref` and **`Omega_T =
1.25 Omega_L`** by the rule of ADR 0420 D3 (bracket `2, 4, 8, 16 omega_ref`, fallback `stress_stiffening(false)`, 60 halvings)
with ADR 0420's own options `RotatingOptions::new(4).modal(ds_modal()).coriolis(true)`, recomputed by the test (deterministic;
ADR 0420 measured `Omega_T = 27843.75417335` rad/s, index 1 lost first, both effects on). The solves at `Omega_T` (sparse and
dense) use a sector and a full rotor built with `RotatingOptions::new(8).modal(ModalOptions::new(8).tolerance(1e-9))` (400
iterations) `.coriolis(true)`: 8 reported dense modes per unit multiplicity instead of 4, for W1. The modal options do not enter
`K_eff`, `M` or `C` (`RotatingPart::with_unit_prestress` reads only their thread count), so the eigenvalues are those of ADR
0420's problem. Indices `k = 0 .. 3` and the full rotor (252
unknowns) at `Omega_T`. Shifts **`sigma_a = (omega_ref / 2)^2`**, **`sigma_b = (2 omega_ref)^2`**, `count = 8`.

**MP** (above the cap): the ring of ADR 0427, `sector_model(16, [4, 1, 2])` and its full rotor (2112 reduced unknowns), both
effects on, Coriolis on, `RotatingOptions::new(4).modal(modal(4))`. `omega_mp` = the lowest rest frequency over `k = 0 .. 8` of
the MP sector (`RotatingSector::whirl_modes(k, 0)`); `Omega_T,mp = 1.25 Omega_L,mp` by the same rule of ADR 0420 D3 on the MP
sector (indices `0 .. 8`; bracket `2, 4, 8, 16 omega_mp`; fallback `stress_stiffening(false)` on sector and full rotor alike if
nothing is lost by `16 omega_mp`). Shift **`sigma_mp = (omega_mp / 2)^2`**, `count = 10`. The reference is the union over `k = 0
.. 8` of `RotatingSector::whirl_state(k, Omega_T,mp)` of the MP sector (the dense state solver on each index, at most 264
unknowns).

**PM** (matrix level): the point masses of ADR 0416 D5 as `2 x 2` CSR matrices: PMI at `1.5 omega_0` and PMA at `1.1 omega_0`,
`sigma = sigma_a`, `count = 2` (all).

**RND** (matrix level): random problems, `n = 6, 12, 30, 60` with `count = 2, 3, 5, 8`: `M = spd(n)` (ADR 0416's `spd`, the same
`lcg`, seed 19), `K = spd(n) - (n + 2) I` (indefinite for every one of these `n`, arithmetic 4), `G` skew with entries `3
lcg`, `sigma = 1.0`, as CSR, against `gyroscopic_state_eigen_dense` of the same dense matrices.

## Arithmetic (done before the code, by hand; disclosed)

1. **The cube's spectrum near the shifts** (from ADR 0416 D5 and its addendum; the amendment-1 torsion near `sqrt(3) omega_0`
   for CBI, `sqrt(3.66) omega_0 ~ 1913` for CBA by the same cancellation): CBI at 1.2, 1.5, 2.0: `|Omega - omega_0| = 200, 500,
   1000`, `Omega + omega_0 = 2200, 2500, 3000`, torsion `~1732`; CBA (ADR 0416 arithmetic 2): 1.05 growth `86.2300`, frequency
   `2156.95`; 1.1: `99.5893`, `2206.79`; 1.18: `59.7702`, `2286.56`; 1.3: frequencies `172.6026`, `2406.29`. The elastic modes of
   the cube are of the order `pi c_s / h ~ 1e5` rad/s. The shifts were chosen so that no closed-form value is within 5 % of
   `sqrt(sigma)` (`250` and `2700` rad/s; the first choice `0.5 omega_0 = 500` would have made `R` singular at CBI `1.5 omega_0`,
   `Omega - omega_0 = 500`, and was rejected for that reason before any code). DS and MP have no closed forms; the shifts are
   fixed multiples of the reference frequency, and an exact coincidence with an eigenvalue has probability zero.
2. **Coverage of the flags** (c4, c5). Cube, `sigma_a`: `nu` of a divergence `gamma = 59.8 .. 99.6` is `1/(gamma^2 + 62 500) ~
   1.4e-5 .. 1.6e-5`; of CBI's `200`: `1/(62 500 - 40 000) = 4.4e-5`; of the next frequencies `500 .. 3000`: `|nu| <= 5.3e-6`; of
   the torsion and the frequencies above 1700: `<= 3.5e-7`; of the elastic modes `~1e-10`. Cube, `sigma_b`: the frequencies
   `2157 .. 3000` give `|nu| ~ 3.8e-7 .. 9.6e-7`, the torsion `~2.8e-7`, the divergence `1/(7.29e6) = 1.37e-7`, the elastic
   modes `~1e-10`. So with `count = 6` the divergence of CBA is among the selected `nu` at both shifts (at most five groups
   above it), and the sparse flag can see every unstable eigenvalue of the cube. DS at `Omega_T` (ADR 0420): every index flagged
   `Stable` (real defect 2.5e-16), so the flags agree if the returned set is imaginary to `1e-8`. MP: the flag is compared with
   the union's (M2), with the condition registered there.
3. **Accuracy** (E1, E2, E3, U1). Converged pairs have the first-order eigenvalue error `<= TOL = 1e-12` relative to `s` times
   the condition of the eigenvalue in the scaled Euclidean product, `~(|s|/c + c/|s|)/2` times the conditioning of `M`
   (`~1 .. 30` near the shift): `<= 1e-11 .. 3e-11`; at breakdown (the cube, `max_steps = n = 16`, a full isotropic space)
   rounding only. The dense reference: `~eps ||A|| kappa_s` absolute (`||A|| ~ 1e5`, cube; `3e5`, DS), `<= 1e-10` absolute,
   against the smallest returned `|s|` (`59.8` cube, `509.86` DS): `<= 2e-12` relative (ADR 0416 measured AN3 2.4e-12 of
   `omega_0` on the cube, ADR 0420 U1 6.7e-14 on DS). Estimate `<= 5e-11` against `1e-9`. MP against the sector union adds the
   pre-stress difference sector against full rotor (`~1e-13` relative on `K_eff`, ADR 0407; amplified by `|s^2 + ...|`
   cancellation of a softened mode by at most `~10`): estimate `<= 1e-11` against `1e-8`.
4. **RND inertia.** `spd(n) = B B^T + n I` with the entries of the square `B` uniform in `[-1, 1]` (variance 1/3): `B B^T` has
   its eigenvalues from near 0 (the smallest singular value of a square random matrix is of order `1/sqrt(n)`) to about `4 n /
   3` (the Marchenko-Pastur edge, cited from memory), so `spd(n) - (n + 2) I` has its lowest eigenvalue near `-2 < 0` and its
   highest near `4 n / 3 - 2 > 0`: indefinite for each `n >= 6`. If a drawn `K` came out definite, the check would still be a
   valid comparison (below the loss), printed.
5. **Cost.** Cube and PM: microseconds. DS: indices of 42 / 84 unknowns, `R` of 84 / 168; dense references as ADR 0420 (under a
   second). MP: `R` of order 4224 with an envelope about twice the ring's (ADR 0427 arithmetic 2: `~3 x 132` per slice for `K`;
   `~800` here): `~3.4e6` entries, the factor `~n b^2 / 2 ~ 1.4e9` flops (1 s); `<= 300` steps of one solve (`~1.4e7` flops) and
   the Gram-Schmidt against `<= 600` vectors of 4224; the 9 dense sector state solves of order `<= 528` (`~10 x 528^3 ~ 1.5e9`
   flops each, 1 to 3 s); the bisection of `Omega_L,mp` (inertia tests and dense whirl solves of `<= 264` unknowns, ADR 0420's
   rule). Estimate for the whole test file under 60 s in release (no heavy record needed).

## Checks (limits fixed now)

| # | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| A1 | c1 | every limit of this table and of the controls fixed in this file, committed alone before any code | holds | the commit order |
| A2 | c2, c3 | MP: `whirl_state` (dense) at `Omega_T,mp` refused with `"unknowns"`; `whirl_state_sparse` returns `2 p >= 20` eigenvalues; `loss` is `Some` (above the loss); review: the sparse paths call neither `check_size` nor `dense_of`, and factor `R` (whose diagonal blocks are `K_eff - sigma M`) with the skyline | each | D5 |
| E1 | c4 | cube, CBI and CBA at their 7 speeds, both shifts: the returned eigenvalues against the dense state solver's by the reading (boundary valid, one-to-one, relative to each value) | `<= 1e-9` | `<= 5e-11` (arithmetic 3) |
| F1 | c4 | the same 14 solves: the sparse flag equals the dense flag (`Stable` at CBI 1.2, 1.5, 2.0 and CBA 1.3; `Unstable` at CBA 1.05, 1.1, 1.18 as ADR 0416 AN3), unstable growths printed (covered by E1) | equal, each | arithmetic 2 |
| L1 | c2 | the `loss` of every cube solve equals the dense `whirl_state`'s (CBI `Some(2)`; CBA `Some(1)` x 3, `Some(2)`), and every registered speed of E1 to M2 has some `loss` | equal; each | the same inertia test |
| E2 | c5 | DS at `Omega_T`, `k = 0 .. 3`, both shifts: as E1 against each index's `RotatingSector::whirl_state` | `<= 1e-9` | `<= 5e-11` |
| F2 | c5 | the same 8 solves: per index the sparse flag equals the dense flag; the rotor's `Stability::combined` of the sparse flags equals the combined dense flags | equal | arithmetic 2 |
| E3 | extra | the full rotor of DS (252 unknowns) at `Omega_T`, both shifts: as E1 against `RotatingPart::whirl_state`; flags equal | `<= 1e-9`; equal | as E2 |
| M1 | c2 | MP at `Omega_T,mp`, `sigma_mp`: the returned eigenvalues against the union over `k = 0 .. 8` of the dense sector state eigenvalues (the reading's ranking and validity applied to the union) | `<= 1e-8` | `<= 1e-11` (arithmetic 3) |
| M2 | c2 | MP: the sparse flag equals `Stability::combined` of the nine dense index flags, if the union's eigenvalue of largest `Re s` is in the union's first `2 p` by `|nu|`; otherwise recorded as not exercised (the sparse flag certifies its set only) | equal | D2 reading |
| P1 | D1 | PMI at `1.5 omega_0` (`sigma_a`, count 2): frequencies `Omega - omega_0 = 500`, `Omega + omega_0 = 2500` relative to each; `Stable`. PMA at `1.1 omega_0`: growth `sqrt(lambda_+)` and frequency `sqrt(-lambda_-)` of ADR 0416 D5 (by `closed`) relative to each; `Unstable` | `<= 1e-9`; flag each | D1, rounding of a `2 x 2` |
| U1 | D1, D2 | RND: the returned eigenvalues against `gyroscopic_state_eigen_dense` by the reading; flags equal | `<= 1e-9`; equal | `~1e-13` |
| Q1 | D3 | the stored residual of every returned mode (PM, RND, cube, DS, full DS, MP) | `<= 1e-8` | `~1e-11` after the smoothing step |
| W1 | D3 | cube and DS indices: every returned mode whose eigenvalue matches a dense reported mode (within `1e-8` relative) and has a relative gap `> 1e-3` to every other dense eigenvalue: `|kappa_sparse - kappa_dense|`; the number compared printed (vacuous if 0, recorded) | `<= 1e-6` | `~2 TOL kappa_s / gap ~ 2e-8` |
| I1 | D2 | the isotropy diagnostic of every registered solve | `<= 1e-10` | rounding, `~1e-15` |
| R1 | D5 | refusals of `gyroscopic_state_eigen_sparse`: shift `0`, `-1`, `NaN` (`"shift"`); `count = 0`, `count = n + 1` (`"modes"`); a `K` of the wrong size (`"matrix"`); `M = -I` (`"mass"`); tolerance `0` (`"options"`); `K = M = I`, `G = 0`, `sigma = 1` (`R = 0`, the first pivot zero: `"shift"`); `whirl_state_sparse(-1.0, .)` (`"speed"`); `RotatingSector::whirl_state_sparse(4, ., .)` on DS (`"harmonic"`) | each its key | D4, D5 |
| R2 | c8 | `cargo test -p rcfd-fem --release` passes with no existing test changed (in particular `si_rotating_whirl_state`, `si_rotating_whirl_sector_state`, `si_rotating_whirl_sparse`, `si_rotating_whirl`); `cargo check -p rcfd-post --tests` | pass | additive edits only |

Negative controls (expected to fail the property, each with a limit; their premises are exact arithmetic, not rounding, after the
lesson of ADR 0427 amendment 1):

| # | Control | Expected |
|---|---|---|
| N1 | `K = M = I_20`, `G = 0`, `sigma = 0.25`, `count = 2`: `P~ = -(1/0.75) I` exactly to rounding, so the first Arnoldi vector spans an invariant subspace (breakdown at step 1 with `h_21` at rounding, `<= 1e-15 ||P~ q_1||`) and one Ritz value exists | refused (`FemError::Solver`, `"sparse_state_whirl"`); with `count = 1` the eigenvalues `+/- i` within `1e-12` |
| N2 | CBA at `1.1 omega_0`, `sigma_a`: the sparse solver on the part with `coriolis(false)` against the dense state solver with it on | worst matched relative deviation `> 1e-2` (`+/- 458`, `479 i` against `+/- 99.6`, `2207 i`, ADR 0416 D5 with `G = 0`) |
| N3 | CBI at `1.5 omega_0`, `sigma_a`, with `coriolis(false)`: the translation pair diverges | flagged `Unstable`, `|growth / sqrt(Omega^2 - omega_0^2) - 1| <= 1e-8` (`1118.034`) and growth `> 1e3` |

Planned mutations (on a `git archive` copy in `f519-scratch`, each run once against the new test file; what catches each is
recorded):

1. `R` with the diagonal blocks `K` (the `-sigma M` dropped): E1, E2, U1, P1.
2. The right-hand side term `-G f~ / c` dropped: E1 (Coriolis on), P1, U1.
3. `s^2 = 1/nu + sigma` (the sign of the shift): E1, P1, U1.
4. Only `rho` returned, not `-rho` (half the eigenvalues): E1, U1 (the counts no longer match the reference by the reading).
5. The sign of the `G / c` block of `Omega^` reversed (an invalid form, `Omega' C` not symmetric): E1 (Coriolis on), U1.
6. The mode `phi = u - v / rho` (the mode of `-rho`): Q1, W1.
7. `TOL = 1e-2`: E2, M1 (the first check step passes with inaccurate Ritz values on the indices of 42 and 84 unknowns).
8. The flag tolerance `0.1` in place of `STABILITY_TOLERANCE`: F1 (CBA 1.05 to 1.18: growth over the returned `max|s|` at most
   `99.6 / 2157 < 0.05`, flagged `Stable`).

## Decision rule

- **done**: A1 to R2 met, N1 to N3 as expected, every planned mutation caught by a registered check, no amendment after a run.
- **partial**: A2, E1, F1, E2, F2 and M1 met (the clauses c2 to c5) with another check missed and its cause diagnosed (a
  `*_known_miss` record), or a control not as expected, or a mutation not caught, or a check met only after an amendment made
  after its run.
- **todo**: E1, E2 or M1 missed, a flag of F1 or F2 disagreeing, or a returned eigenvalue non-finite.

## Limits (what is not covered)

The sparse flag certifies only the returned eigenvalues: an unstable eigenvalue far from `+/- i sqrt(sigma)` (a divergence much
faster than the shift frequency, a flutter of high frequency) is not seen, and the odd-index theorem of ADR 0416 D3 is not applied
to override it (the `loss` count is returned for the caller). A double eigenvalue beyond the `+/- s` pairing (a full rotor with
the Coriolis matrix off, a pair index with `G = 0`) is found only if rounding brings its second copy into the single Krylov space
(ADR 0427 amendment 1); no deflated restart is written. No implicit restart (Sorensen 1992): the basis is bounded by `20 count +
100` vectors. No pivoting in the indefinite factor (Bunch-Kaufman): a refused pivot refuses the shift. Real shifts (`s_0` real,
`Q(s_0)` unsymmetric) and `sigma <= 0` are not offered. No wave sense on the sector, no table over speeds, no case-file or CLI
switch; `WhirlSolver::Auto` does not route `whirl_state` to the new solver.
