# ADR 0420: F-511 registration: whirl stability of a cyclic sector above the loss of stiffness (each harmonic index solved by the state matrix of ADR 0416 in its real cos/sin form, the stability flag and growth rate per index, the union of the indices against the full rotor above the first loss of stiffness of the disc sector)

Status: **REGISTERED 2026-11-13, written before any code of F-511 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-511 was written, compiled or run: the code listed under "What was read" was read, the
base was built once (`cargo build -p rcfd-fem --release --tests`, no test run, log `f511-scratch/build0.log`), and the only
numbers used below that came from a run are those printed by ADR 0411's registered run (`f499-scratch/run1.log`, lines 86 to
110), disclosed under "Arithmetic". Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0416](0416-f505-registration-gyroscopic-stabilisation.md) (F-505: the state matrix `A = [[0, c I], [-K^/c,
-G^]]`, Hessenberg reduction and the Francis double-shift QR algorithm, the flag `Stable` iff `max|Re s| <= 1e-8 max|s|`,
otherwise `Unstable { growth: max Re s }`; its limit "the cyclic sector ... not wired to the harmonic reduction" and remaining
question 2, recommendation (a)), [ADR 0411](0411-f499-registration-coriolis-gyroscopic-whirl-modes.md) (F-499: `G = 2 Omega C`,
the sector reduction of `K_eff`, `M`, `G` of index `k` by `T_k` on the doubled matrices (D5), the disc sector DS and its full
rotor), [ADR 0407](0407-f500-registration-rotating-modes-of-a-cyclic-sector.md) (F-500: `RotatingSector`, the loss of stiffness
per index, Sylvester's count of the full rotor as the sum of the indices' counts) and [ADR
0419](0419-f507-registration-stationary-frame-whirl.md) (F-507: the stationary column of `CyclicWhirlCampbell`; not touched
here). Roadmap item F-511 (dep F-505). Number 0420 given by the lead. Branch `f511-sector-whirl-stability` (base `76706dc0`),
worktree `w49i-work/f511`, scratch `w49i-work/f511-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, the hash tables or a golden file; no new dependency, no unsafe code; clean-room.

Library edits, additive only and confined to the rotating module (another branch, F-391, changes the `DofMap` of `rcfd-fem`;
nothing here touches it, the new code only reads `dof_map().n_equations()` as `whirl.rs` does). A new file
`crates/rcfd-fem/src/rotating/whirl/state/sector.rs`, a child module of `rotating::whirl::state` (so that it reads the private
`state_solve` of `state.rs` and the private helpers of `whirl.rs`, `dense_of`, `inertia`, `check_size`, `doubled`, `dot`,
`whirl_number`, `speed_refused`, `mass_refused`, and the private fields of `RotatingSector`). In `state.rs`: one line `mod
sector;` and one `pub use sector::{...};`, nothing else. In `whirl.rs`: the names added to the existing `pub use state::{...}`
list. In `rotating.rs`: the names added to the `pub use whirl::{...}` list of ADR 0416 and one sentence of the module comment. No
existing function changes what it computes: `RotatingSector::{whirl_modes, whirl_campbell, modes, campbell}` still report a
speed with `K_eff` not positive definite as a loss, and `RotatingPart::{whirl_state, whirl_stability}` are not changed. The new
analysis is reached only through the new functions (opt-in by calling them). Tests: a new file
`crates/rcfd-fem/tests/si_rotating_whirl_sector_state.rs` and one unit test in `sector.rs`. No existing test is edited or
retired.

## What F-511 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. RotatingSector solves each harmonic index by the state
> matrix of ADR 0416 at speeds where its reduced K_eff is not positive definite, with the stability flag and growth rate per
> index; on the disc sector of ADR 0411 the union of the sector's eigenvalues equals the full rotor's within 1e-8 relative at a
> speed above the first loss of stiffness, and the flags agree.

Clauses: (c1) every limit not stated in the acceptance registered here before the run; (c2) `RotatingSector` solves each
harmonic index by the state matrix of ADR 0416 at speeds where its reduced `K_eff` is not positive definite; (c3) with the
stability flag and the growth rate per index; (c4) on the disc sector DS of ADR 0411, at a speed above the first loss of
stiffness, the union of the sector's eigenvalues equals the full rotor's within 1e-8 relative; (c5) the flags agree. The brief
adds: (c6) derive at which speed the disc sector's reduced `K_eff` first loses definiteness and pick the test speed from that
derivation; (c7) derive how the state matrix handles the complex Hermitian problem of an index `k > 0` (complex arithmetic or the
real 2 x 2-block form) and cite it; (c8) existing outputs bit-identical, the new path opt-in.

Readings (registered now). "Within 1e-8 relative" (c4): the two lists of eigenvalues `s` (complex, all `2 n` of each) have
equal length, are matched one to one (the matching rule under U1), and every matched pair has `|s_sector - s_full| / |s_full|
<= 1e-8` (relative to each value, the stricter reading, as ADR 0416 took for its c4). "The flags agree" (c5): (i) the full
rotor's flag equals the flag of the union (stable iff every index is stable; if unstable, the growth rates agree within 1e-8
relative); and (ii) per index, the sector's flag of index `k` equals the flag the D3 rule of ADR 0416 gives on the full rotor's
eigenvalues matched to that index.

## What was read (branch base 76706dc0, read only)

- `AGENT_RULES.md`; the brief; ADR 0416 (all, with amendment 1 and the addendum); ADR 0411 (D1 to D6, set-up, checks, addendum);
  ADR 0407 (set-up, derived speeds, the addendum's derived values); ADR 0419 (head, D1 to D3).
- `crates/rcfd-fem/src/rotating/whirl/state.rs` (all); `crates/rcfd-fem/src/rotating/whirl.rs` (lines 1 to 60, 600 to 1000:
  `check_size`, `dense_of`, `whirl_number`, `inertia`, `doubled`, `RotatingSector::{whirl_modes, whirl_at}` with the reduction
  by `harmonic_transformation` of the doubled matrices and the wave sense); `crates/rcfd-fem/src/rotating.rs` (lines 1 to 145:
  module comment, `pub use`, `RotatingOptions`; `HarmonicLoss`, `RotatingSector`, `check_harmonics`).
- Tests: `si_rotating_whirl.rs` (lines 1 to 80, 485 to 690: `Checks`, DS, `full_of`, `ds_options`, `omega_ref`, S1, S2);
  `si_rotating_whirl_state.rs` (lines 1 to 30 and the list of its tests).
- `f499-scratch/run1.log` (lines 86 to 110) and the addendum of ADR 0407 (its disc's `Omega_L` with the fallback).

## Design

Notation of ADR 0411 and ADR 0416: speed `Omega >= 0` about the unit axis `a`; `N` sectors; `K = K_eff(Omega) = K_0 + Omega^2
D`, `D = K_sigma,1 - M_perp` (both effects; `D = -M_perp` with the stress stiffening off); `M` symmetric positive definite; `G =
2 Omega C` skew; the quadratic pencil `Q(s) = s^2 M + s G + K`; the reduced matrices of index `k` are those of ADR 0411 D5:
`X_k = T_k^T blockdiag(X_s, X_s) T_k` for a pair index (`0 < 2k < N`, order `2 n_k`), `T_k^T X_s T_k` for a single index (`k =
0`, `2k = N`, order `n_k`).

### D1. The complex Hermitian problem of a pair index in its real 2 x 2-block form (c7; derived here)

ADR 0411 D5 showed that the real reduced matrices of a pair index commute with `J: (a, b) -> (-b, a)` (a real orthogonal map,
`J^2 = -I`, `J^T = -J`), `K_k`, `M_k` symmetric and `G_k` skew. `J` has the eigenvalues `+i` and `-i`, each of multiplicity
`n_k` (its eigenspaces are complex conjugates of each other, `J v = i v` iff `J conj v = -i conj v`). Let `V` (`2 n_k x n_k`)
have orthonormal columns spanning the `+i` eigenspace; then `conj V` spans the `-i` eigenspace and `U = [V, conj V]` is unitary.
A real `X` commuting with `J` maps each eigenspace into itself, so `X V = V X_c` with `X_c = V^H X V` and `X conj V = conj V conj
X_c`, that is `U^H X U = blockdiag(X_c, conj X_c)`. From the symmetry of `K_k`, `M_k` and the skewness of `G_k`: `K_c`, `M_c` are
Hermitian (`M_c` positive definite), `G_c` is skew-Hermitian. This `n_k x n_k` complex problem `Q_c(s) = s^2 M_c + s G_c + K_c`
is the complex Hermitian problem of the harmonic (the field `e^{-/+ i j k alpha}` on copy `j`), and `U^H Q(s) U = blockdiag(Q_c(s),
conj Q_c(conj s))`. Hence

`det Q(s) = det Q_c(s) conj(det Q_c(conj s))`, `sigma(Q) = sigma(Q_c) U conj sigma(Q_c)` (with multiplicity).

So **the real 2 x 2-block form is used**: the real reduced pencil of ADR 0411 D5, of order `2 n_k`, goes unchanged into the real
state matrix of ADR 0416 D1 (order `4 n_k`), whose eigenvalues are those of `Q` (ADR 0416 D1: `det(s I - A) = det(s^2 I + s G^
+ K^)`, independent of the scale `c` and of whether the Cholesky factor of `M` commutes with `J`). The spectrum delivered is that
of the complex Hermitian problem together with its conjugate. Reason for the choice: the repository has no complex type and no
complex QR algorithm; the real form needs none (the real Francis algorithm of ADR 0416 D2 applies as it is), and its cost, a real
QR on order `4 n_k` against a complex single-shift QR on order `2 n_k` (complex flops about four real ones), is about twice the
work, negligible at the sector's sizes. The complex-arithmetic alternative (a complex Hessenberg reduction and a single-shift QR)
is not written.

Structure. `Q_c(s)^H = conj(s)^2 M_c - conj(s) G_c + K_c = Q_c(-conj s)`, so `sigma(Q_c)` is symmetric about the imaginary axis
only; the real form's spectrum is symmetric about both axes (ADR 0416 D3), and the flag of D3 applied to it is "every `s` of
`Q_c` imaginary". At `G = 0` (`Omega = 0` or Coriolis off) `Q_c` is the Hermitian pencil, `sigma(Q_c)` is also symmetric under `s
-> -s`, so every eigenvalue of the real form is double (the standing-wave pair); with `G != 0` the forward and backward waves of
ADR 0411 D5 split and the eigenvalues of the real form are simple in general. A simple eigenvalue of the real form has its
eigenvector in one eigenspace of `J`, `b = -/+ i a`, so its wave sense (ADR 0419 D1) is `|tau| = 1`, whether the mode grows or
not (check W1).

**Parity (the odd-index theorem, ADR 0416 D3, for the indices).** For a single index the reduced problem is real and the theorem
applies as in ADR 0416: an odd count of negative eigenvalues of `K_k` forces `Unstable`. For a pair index `det K_k = |det K_c|^2
>= 0` and the count of `K_k` is twice that of `K_c` (even, as `HarmonicLoss` already states), so the theorem never forces a pair
index unstable; a pair index whose `K_c` has one negative eigenvalue may be stable or not (the product of the `2 n_k` eigenvalues
of `Q_c`, `det K_c / det M_c`, carries no sign condition for imaginary roots that are not in conjugate pairs).

### D2. The full rotor as the union of the indices (c4, c5)

The full rotor commutes with the shift by `2 pi / N` (ADR 0407 D1); the real discrete Fourier basis of the copies (ADR 0407 D1,
ADR 0411 D5) is orthogonal and block-diagonalises `K_eff`, `M` and `G` of the full rotor into the blocks of the indices `k = 0
.. floor(N/2)` (a pair index carrying the cosine and sine blocks, that is the harmonics `k` and `N - k`, whose complex problems are
complex conjugates: `sigma(Q_c,N-k) = conj sigma(Q_c,k)`). The full rotor's free unknowns are the sum of the indices' (ADR 0411
S1: `42 + 84 + 84 + 42 = 252`). Hence, exactly in exact arithmetic:

- `sigma(Q_full) = U_k sigma(Q_k)` with multiplicity (c4);
- the full rotor is stable iff every index is, and its growth rate is `max_k growth_k` (c5 (i));
- by Sylvester's law of inertia (the congruence is orthogonal) the count of negative eigenvalues of the full `K_eff` is the sum of
  the indices' counts (ADR 0407 L2), check L1.

The only differences in floating point are the rounding of the two QR runs and the pre-stress of the sector against the full
rotor's own static solve (ADR 0407 arithmetic 3, `~1e-13`).

### D3. The first loss of stiffness of the disc sector (c6; derived here)

For index `k` let `mu_k(t)` be the lowest eigenvalue of `(K_0,k + t D_k) phi = mu M_k phi`, `t = Omega^2`. By the min-max
principle `mu_k(t) = min_phi (phi^T K_0,k phi + t phi^T D_k phi) / phi^T M_k phi` is a minimum of affine functions of `t`, hence
**concave**; `mu_k(0) = omega_k,1^2 > 0`. A concave function that is positive at 0 and zero at `t_1` has a slope `<= -mu_k(0) /
t_1 < 0` beyond it, so it stays negative: **the speeds at which index `k` has lost stiffness form a half-line `[Omega_L,k,
infinity)`, or the set is empty**, and the first loss of the sector is `Omega_L = min_k Omega_L,k`. `Omega_L,k^2 = 1 / nu_k`
with `nu_k` the largest eigenvalue of `-D_k phi = nu K_0,k phi` if `nu_k > 0`; no loss at any speed if `D_k >= 0` on the
admissible motions.

Bounds. (i) If the centrifugal stress is tensile (`K_sigma,1 >= 0`), `D >= -M_perp >= -M` and `K_eff >= K_0 - Omega^2 M`, so
`Omega_L,k >= omega_k,1 >= omega_ref` (ADR 0407's bound; ADR 0411 found no loss at `0.8 omega_ref`). (ii) Which motions can lose
stiffness at all: a rigid rotation about the axis has `phi^T D phi = 0` (ADR 0416 amendment 1: the pre-stress cancels the spin
softening on it), a rigid in-plane translation has `K_sigma,1 1_v = 0` and `phi^T D phi = -phi^T M phi < 0` (full softening), and
out-of-plane bending has little in-plane motion (`phi^T M_perp phi` small) and a large stress stiffening. Both rigid motions are
excluded by the clamped hub; the candidates for `nu_k > 0` are in-plane fields that are close to a translation of the rim
relative to the hub (index 1) or to the torsion (index 0), for which the softening and the stiffening are of the same order
(the stiffening of a field varying over a length `L` is `~ sigma_r / L^2` per unit amplitude, with `sigma_r ~ rho Omega^2 R^2`,
against the softening `rho`: a ratio `~R^2 / L^2 ~ 1` for `L ~ R`). **The sign of `nu_k` with both effects on cannot be fixed by
this estimate**; on ADR 0407's disc (another mesh) every rest mode was stiffened more than softened. With the stress stiffening
off (`D = -M_perp <= 0`, `phi^T M_perp phi > 0` for every field with in-plane motion) a loss is guaranteed, with `omega_k,1 <=
Omega_L,k <= min_i omega_k,i / sqrt(phi_k,i^T M_perp phi_k,i)` over the M-normalised rest modes.

Hence the rule (fixed now; its values are computed by the run and printed, not chosen):

1. `omega_ref` as ADR 0411 (the lowest rest frequency over `k = 0 .. 3`, by `RotatingSector::whirl_modes(k, 0)`).
2. **Bracket.** With both effects on: the loss of each index (`RotatingSector::whirl_modes(k, Omega).loss()`, the inertia test of
   ADR 0403 D3, not changed here) at `Omega = 2, 4, 8, 16 omega_ref` in that order; `Omega_hi` the first of these at which any
   index has lost. If none has lost at `16 omega_ref` (no loss of definiteness up to there with both effects on, a finding
   recorded as such), **fallback**: every check below runs with `stress_stiffening(false)` on the sector and on the full rotor
   alike (both effects on except the pre-stress; Coriolis on), and the bracket is repeated with it (a loss is then guaranteed; if
   still none by `16 omega_ref` the set-up is defective).
3. **Bisection.** For each index `k` lost at `Omega_hi`: 60 halvings of `[0.8 omega_ref, Omega_hi]` on "index `k` has lost"
   (monotone by the concavity above); `Omega_L,k` the upper end. `Omega_L = min_k Omega_L,k`, `k*` its index.
4. **The test speed** `Omega_T = 1.25 Omega_L`, so that `t_T = 1.5625 t_L`: by concavity `mu_k*(t_T) <= mu_k*(t_L) + (t_T - t_L)
   slope <= -0.5625 mu_k*(0) < 0`, a negative eigenvalue of `K_eff` well away from zero (`|s|` of the losing motion of order
   `0.75 omega_k*,1`, not near 0, so the relative comparison of c4 is not degraded by a near-zero eigenvalue); 1.25 is fixed
   now, not tuned. Other indices may or may not have lost at `Omega_T`; all are solved and compared.

Prediction (registered, from arithmetic 2): with both effects on the outcome of step 2 is open; with the fallback `k* = 0` (the
torsion) at `Omega_L ~ 1.2 .. 1.5 omega_ref`, count 1, so index 0 and the full rotor are flagged `Unstable` by the odd-index
theorem. Only the conditional form below (P1) is a check; the numbers are an expectation.

### D4. Interfaces (all new, in `rotating/whirl/state/sector.rs`, re-exported from `rotating`)

- `RotatingSector::whirl_state(harmonic, omega) -> Result<HarmonicStateWhirl>`: `K_eff(omega)`, `M` and `C` of the sector,
  doubled for a pair (`doubled`), reduced by `harmonic_transformation` (index 0 by `transformation`), as
  `RotatingSector::whirl_at` of ADR 0411 D5 does; `G = 2 omega C_k` with `options.coriolis`, 0 without; the inertia test gives
  `loss` (`None` when positive definite); then `state_solve` of ADR 0416 with `options.modal.modes x multiplicity` reported modes.
  No zero-speed shortcut (at `omega = 0` the state solve with `G = 0` is valid; its pair eigenvalues are double). Refusals as
  `RotatingSector::whirl_modes`: a harmonic above `N/2` (`"harmonic"`, checked first), a negative or non-finite speed
  (`"speed"`), a mode count of 0 or `count >= n_k` (`"modes"`), a zero mass diagonal (`"density"`), more than
  `MAX_WHIRL_UNKNOWNS` (`"unknowns"`).
- `HarmonicStateWhirl { harmonic, multiplicity, state: StateWhirl, waves: Vec<Option<f64>> }`: `state` the `StateWhirl` of ADR
  0416 D6 on the index's real problem (`loss` with that problem's count, even for a pair; the modes expanded to the sector
  equations, cosine then sine part for a pair, length `multiplicity x n`, as `WhirlMode` of ADR 0411; `whirl` on the reduced
  matrices); `waves` the wave sense `tau` of each mode (ADR 0411 D5's formula on the expanded vectors with the sector mass),
  `None` for a single index.
- `Stability::combined(flags: &[Stability]) -> Stability`: `Stable` iff all are (an empty list is `Stable`), otherwise
  `Unstable { growth: max of the unstable growths }` (D2: the rotor's flag from its indices).
- `RotatingSector::whirl_stability(harmonics, speeds) -> Result<CyclicWhirlStability>` (harmonics refused as
  `RotatingSector::campbell`, key `"harmonics"`; speeds as ADR 0403 D4, key `"speeds"`): `CyclicWhirlStability { harmonics,
  speeds, results: Vec<Vec<HarmonicStateWhirl>> }` (`results[h][j]` index `harmonics[h]` at `speeds[j]`) with `rotor(j) ->
  Stability` (`Stability::combined` over the indices at speed `j`; the rotor's flag when `harmonics` covers `0 .. N/2`) and
  `to_csv()`: columns `harmonic,speed_rad_s,negative_stiffness,stable,growth_rate_1_s` and `f_<j>_hz` for the reported modes
  (as ADR 0416's `WhirlStability::to_csv`), one row per index and speed (index-major), shortest round-trip numbers.

Bit-identity (c8): nothing existing calls the new functions; the edits of `state.rs`, `whirl.rs` and `rotating.rs` are a `mod`
line, `pub use` names and a comment sentence. No output of an existing function can change.

## Set-up (fixed now)

**DS**, exactly as ADR 0411 (`si_rotating_whirl.rs`: `ds_mesh`, `ds_sector`, `full_of`, `ds_modal`, copied): `N = 6`, the
annular Hex20 sector `hex_box([0.25, 0, -0.1], [0.75, alpha, 0.2], [2, 1, 1])` mapped to
`(r cos t, r sin t, z)` and upgraded to Hex20, `xmin` (the hub) clamped, the cyclic constraint `ymin`/`ymax` about `e_z`, steel;
`RotatingOptions::new(4).modal(ds_modal()).coriolis(true)` (both effects on unless the fallback of D3 applies). The full rotor
by `full_rotor` with `xmin` clamped, `RotatingPart::new(full, e_z, 0, same options)`.

Speeds: `0.8 omega_ref` (positive definite, the cross-check against ADR 0411's solver) and `Omega_T` of D3.

Matching rule for U1 (fixed now): all `|s_i - s'_j|` over the two lists, taken in increasing order (ties by `(i, j)`), each pair
accepted when neither member is taken (a global greedy assignment); the measure is `max |s_sector - s_full| / |s_full|` over the
accepted pairs. Each full eigenvalue so matched carries the index of its sector partner; the per-index flag of c5 (ii) is the D3
rule (`max|Re s| <= 1e-8 max|s|` over the full eigenvalues matched to the index).

## Arithmetic (done before the code; disclosed)

1. Sizes (ADR 0411 run 1, `run1.log` line 87): sector unknowns `42, 84, 84, 42` for `k = 0 .. 3`, full rotor 252. State orders
   `84, 168, 168, 84` and 504; the full QR `~10 x 504^3 ~ 1.3e9` flops, about a second in release; the bisection `4 x 60`
   inertia tests and dense solves of order `<= 84` (milliseconds each). The whole test well under two minutes.
2. The fallback estimate of D3 from `run1.log` lines 90 and 107: index 0 has at `0.4` and `0.8 omega_ref` (`omega_ref =
   1655.3226` rad/s) a mode at `2419.85`, `2447.87` rad/s with `kappa = 0.044`, `0.088` (little whirl: tangential motion, the
   torsion), the other low one at `1915`, `2234` (bending, out of plane). With the pre-stress off the torsion is fully softened
   (`phi^T M_perp phi ~ 1`), so `Omega_L,0 <~ 2420` rad/s `~ 1.46 omega_ref`; index 1's in-plane pair (`4889`, `5868` at `0.4
   omega_ref`, mean `~5380`) loses later. Expectation `k* = 0`, `Omega_L ~ 1.2 .. 1.5 omega_ref`, count 1 (odd). With both
   effects on the in-plane means rise with speed (`5378 -> 5409` rad/s for the index-1 pair, `2420 -> 2448` for the torsion,
   between `0.4` and `0.8 omega_ref`), which suggests stiffening, so the fallback may well be taken; not decided here.
3. Rounding (ADR 0416 arithmetic 5 for the scale): with `c = sqrt(max|K^| + max|G^|^2)`, `||A|| ~ max|s|`, which for DS is the
   highest whirl frequency of the coarse Hex20 mesh, `~1e5` rad/s at most (estimate); eigenvalue errors `~10 .. 100 eps ||A|| ~
   2e-10 .. 2e-9` absolute against the smallest `|s| ~ 1e3` rad/s (the losing motion at `Omega_T`, D3 step 4): `<= 2e-12`
   relative; the pre-stress difference sector against full adds `~1e-13` relative to `K_eff` (ADR 0407), which for a motion
   whose `K_eff` Rayleigh quotient is cancelled to a fraction `0.56` of its rest value is amplified by at most `~2`: `~1e-12`
   on `|s|^2`. Estimate `<= 1e-11` against the limit `1e-8`. A near-coincidence of `Omega_T` with the loss of another index (an
   eigenvalue near 0) would degrade it and would be a diagnosed miss.
4. Growth rates: of the order of `|s|` of the losing motion (`~1e3` 1/s for the fallback torsion); relative errors as 3.

## Checks (limits fixed now)

| # | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| D | c6 | the derived speeds printed: `omega_ref`; which bracket speed first showed a loss and with which effects (both or the fallback); `Omega_L,k` per lost index, `Omega_L`, `k*`, `Omega_T`; assert `Omega_L > 0.8 omega_ref` (bound (i)) | printed; holds | D3 |
| B1 | c2 | at `Omega_T`: `RotatingSector::whirl_state(k*, Omega_T)` has `loss == Some(_)` (its `K_eff` not positive definite) with a count `>= 1` (even for a pair) and twice its reduced unknowns of finite eigenvalues, `whirl_modes(k*, Omega_T)` (ADR 0411) still a `LossOfStiffness` | each | D3 step 4 |
| L1 | c2 | at `Omega_T`: the full rotor's `whirl_state(Omega_T).loss` count equals the sum over `k = 0 .. 3` of the indices' counts (0 for `None`), each pair count even | exact | D2 (Sylvester) |
| U1 | c4 | at `Omega_T`: the union over `k = 0 .. 3` of the `whirl_state` eigenvalues against the full rotor's: count equal (504), matched by the rule of the set-up, `max |s_sector - s_full| / |s_full|` | count; `<= 1e-8` | `<= 1e-11` (arithmetic 3) |
| F1 | c3, c5 | at `Omega_T`: (i) the full rotor's flag equals `Stability::combined` of the four indices' flags, and if unstable `|growth_full / max_k growth_k - 1|`; (ii) for each `k` the sector's flag equals the D3 flag of the full eigenvalues matched to `k`, and if unstable `|growth_k / growth_full,k - 1|` (the matched set's `max Re s`) | equal; `<= 1e-8`; equal; `<= 1e-8` | D2 |
| P1 | c3 | at `Omega_T`: every single index (`k = 0, 3`) with an odd count is flagged `Unstable`, and the full rotor with an odd total count too (the odd-index theorem); vacuous (recorded as such) if no count is odd | each | ADR 0416 D3 |
| Q1 | c2 | the stored residual of every returned mode of the four indices at `Omega_T` and at `0.8 omega_ref` | `<= 1e-9` | `~1e-12` (ADR 0416 Q1 on the cube: 3e-12) |
| E1 | c2 | each returned sector mode at `Omega_T` expanded to the full rotor (real and imaginary parts by `FullRotor::expand`): the relative residual `||Q_full(s) phi|| / (||K phi|| + |s|^2 ||M phi|| + |s| ||G phi||)` at the free equations, with the full `K_eff`, `M`, `G` | `<= 1e-8` | `~1e-12` (ADR 0411 S2: 2e-13) |
| W1 | D1 | at `Omega_T`, pair indices `k = 1, 2`: every returned mode whose eigenvalue has a relative gap `> 1e-6` to every other eigenvalue of its index: `| |tau| - 1 |` | `<= 1e-6` | `~1e-12` (D1: simple eigenvalue in one eigenspace of `J`) |
| X1 | c2 | at `0.8 omega_ref` (positive definite): per index, `loss == None`, flagged `Stable`, and the positive imaginary parts of its eigenvalues (as many as its reduced unknowns, `StateWhirl::unknowns`) against `frequencies_all` of `RotatingSector::whirl_modes` (ADR 0411's solver): count equal, `max |omega / omega_ref - 1|` | each; count; `<= 1e-9` | `~1e-12` (ADR 0416 X1: 3.6e-12) |
| W2 | D4 | at `0.8 omega_ref`, `k = 1, 2`: each returned state mode against the `whirl_modes` mode of the same position: `|omega/omega' - 1| <= 1e-9` and the wave sense `|tau - tau'|` | `<= 1e-8` | same vectors up to a phase (`tau` phase-invariant) |
| T1 | D4 | `whirl_stability(&[0, 1, 2, 3], &[0.8 omega_ref, Omega_T])`: each entry equal (`==`) to `whirl_state` of its index and speed; `rotor(j)` equal to `Stability::combined` of the column; `to_csv`: `1 + 8` lines, `5 + width` fields per line, `harmonic` and `stable` columns equal to the entries, every speed, growth and frequency parsing back to its bits | each | D4 (self-consistency) |
| U2 | D4 | unit test of `Stability::combined`: `[]` and `[Stable, Stable]` are `Stable`; `[Stable, Unstable{3}, Unstable{5}, Stable]` is `Unstable{5}`; `[Unstable{5}, Unstable{3}]` is `Unstable{5}` | each | D4 |
| R1 | D4 | refusals on DS: `whirl_state(4, .)` (`"harmonic"`), `whirl_state(1, -1.0)` and `(1, NaN)` (`"speed"`), `whirl_stability(&[], .)` (`"harmonics"`), `whirl_stability(&[1], &[2.0, 1.0])` (`"speeds"`) | each its key | D4 |
| R2 | c8 | the existing tests of `rcfd-fem` pass unchanged (release), in particular `si_rotating_whirl`, `si_rotating_whirl_state`, `si_rotating_whirl_stationary`, `si_rotating_cyclic`; `cargo check -p rcfd-post --tests` | pass | additive edits |

Negative controls (expected to fail the property, each with a limit):

| # | Control | Expected |
|---|---|---|
| N1 | U1 with the sector's `coriolis(false)` against the full rotor with it, at `Omega_T` | worst matched relative deviation `> 1e-6` |
| N2 | the full rotor at `Omega_T` with `coriolis(false)`: `K_eff` has a negative eigenvalue (L1's count `>= 1`), so `s = +/- sqrt(-lambda)` is real | flagged `Unstable`, growth `> 1` 1/s |

Planned mutations (on a `git archive` copy in `f511-scratch/mut`, each run once against `si_rotating_whirl_sector_state` and the
unit tests of `whirl::state`; what catches each is recorded):

1. In `RotatingSector::whirl_state`, `G = omega C` (the factor 2 dropped): U1, E1.
2. The sign of `G` reversed in `RotatingSector::whirl_state`: E1 (the eigenvalue set is the same by the symmetry of D1 and ADR 0416
   D3, but the vectors then solve the problem with `-G`, so the full residual with the true `G` is of order 1); U1 and F1 are
   expected not to see it.
3. `K_eff` of the sector at speed 0 (`operator(0)`) in `whirl_state`: B1, L1, U1.
4. The Coriolis matrix of a pair index taken as zero (`G = 0` when `multiplicity == 2`): U1, E1, W1 (the eigenvalues double).
5. `Stability::combined` with the minimum growth instead of the maximum: U2.
6. `Stability::combined` `Stable` when any (instead of every) flag is stable: U2.
7. The wave sense with its numerator negated: W2.
8. The sine half of a pair mode dropped in the expansion (only the cosine part kept, the rest zero): E1, W1.

## Decision rule

- **done**: D, B1, L1, U1, F1, P1, Q1, E1, W1, X1, W2, T1, U2, R1, R2 met, N1 and N2 as expected, and every mutation caught
  except mutation 2 by U1 and F1 (registered as not expected there). The fallback of D3 step 2, if taken, does not by itself
  prevent done: it is registered here before the run as the reading of "the disc sector of ADR 0411" (its mesh, constraints and
  material; the pre-stress switched off only if with it no index loses definiteness up to `16 omega_ref`), and it is reported.
- **partial**: U1 and F1 met (clauses c4 and c5) with another check missed and its cause diagnosed (a `*_known_miss` record), or a
  mutation not caught, or a check met only after an amendment made after its run.
- **todo**: U1 or F1 missed, or the full rotor's or an index's state solve refused at `Omega_T`.

## Limits (what is not covered)

The stationary-frame frequency of a state mode (ADR 0419's column) is not added to the new table (the wave sense is stored, so
`omega + tau k Omega` can be formed; not asked); tracking of the supercritical branches over speed (per-speed results only, as
ADR 0416); damping; the flag at a defective double eigenvalue (a Krein collision), as ADR 0416; parts above the dense cap of 1500
reduced unknowns per index; the aliased harmonics `k + m N` (ADR 0419 D2); output through case files and the CLI. One disc
geometry.
