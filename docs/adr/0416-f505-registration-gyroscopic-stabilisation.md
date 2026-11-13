# ADR 0416: F-505 registration: whirl modes above the loss of stiffness (the state matrix of the gyroscopic problem solved by Hessenberg reduction and the Francis double-shift QR algorithm, a stability flag with the growth rate, gyroscopic stabilisation of the point mass, the unstable band of an anisotropic spring pair)

Status: **REGISTERED 2026-11-13, written before any code of F-505 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-505 was written, compiled or run: the code listed under "What was read" was read, the
base was built once (`cargo build -p rcfd-fem --release --tests`, no test run), and the arithmetic below was done with one
scratch script (`f505-scratch/arith.py`: the closed forms of D5, and `numpy.linalg.eigvals` of the 4 x 4 state matrix of the
point mass as an independent cross-check of those closed forms; it does not call rcfd; disclosed under "Arithmetic"). Results go
in the addendum at the end; nothing above the addendum is changed after a run.

Parent: [ADR 0411](0411-f499-registration-coriolis-gyroscopic-whirl-modes.md) (F-499: `G = 2 Omega C`, the whirl problem `(K_eff
- omega^2 M + i omega G) phi = 0` by Meirovitch's symmetric linearisation for `K_eff > 0`, the whirl number `kappa`, the point
mass PM and the cube on springs CB; its "Limits" and remaining question 1: speeds above the loss of stiffness reported as a
loss). Roadmap item F-505 (dep F-499). Number 0416 given by the lead. Branch `f505-gyroscopic-stabilisation`, worktree
`w49i-work/f505`, scratch `w49i-work/f505-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, the hash tables or a golden file; no new dependency, no unsafe code; clean-room.

Library edits, additive only. A new file `crates/rcfd-fem/src/rotating/whirl/state.rs`, a child module of `rotating::whirl` (so
that it reads the private helpers of `whirl.rs`, `cholesky`, `dense_of`, `inertia`, `whirl_number`, `check_size`, and the private
fields of `RotatingPart`). In `whirl.rs`: one line `mod state;` and one `pub use state::{...};`, nothing else. In `rotating.rs`:
the `pub use whirl::{...}` list gains the new names and the module comment one sentence. No existing function changes what it
computes: `RotatingPart::{whirl_modes, whirl_campbell}` still report a speed with `K_eff` not positive definite as a loss (the
check L1 of ADR 0411 stays true), and the new analysis is reached only through the new functions (opt-in by calling them). Tests:
a new file `crates/rcfd-fem/tests/si_rotating_whirl_state.rs`. **Scaffold tests**: no existing `si_*` test is retired. The only
existing assertion about speeds above the loss is L1 of `si_rotating_whirl.rs` (`whirl_modes(1.2 omega_0)` is a
`LossOfStiffness`), and it stays the behaviour of `whirl_modes`; so no ignored record and no `_after_landed` copy is needed.

## What F-505 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. At a speed where K_eff is not positive definite the whirl
> problem is solved by a non-symmetric eigensolver of the state matrix written from first principles; each speed is flagged
> stable (all frequencies real within a registered tolerance) or unstable with the growth rate; the point mass on isotropic
> springs gives Omega - omega_0 and Omega + omega_0 above omega_0 within 1e-8 and is flagged stable; an anisotropic spring pair
> gives the closed-form unstable band between the two spring frequencies.

Clauses: (c1) every limit registered here before the run; (c2) at a speed with `K_eff` not positive definite the whirl problem
solved by a non-symmetric eigensolver of the state matrix written from first principles; (c3) each speed flagged stable (all
frequencies real within a registered tolerance) or unstable with the growth rate; (c4) the point mass on isotropic springs:
`Omega - omega_0` and `Omega + omega_0` above `omega_0` within 1e-8, flagged stable; (c5) an anisotropic spring pair: the
closed-form unstable band between the two spring frequencies.

Reading of "within 1e-8" in c4 (registered now): relative to each closed-form value, `|omega / omega_cf - 1| <= 1e-8`, the
stricter of the two readings (ADR 0411 measured relative to `omega_0`; here `Omega - omega_0` can be `0.1 omega_0`). Reading of c5:
the speeds flagged unstable are exactly those strictly between the two spring frequencies, the band edges found by bisection of
the flag equal the two spring frequencies, and inside the band the growth rate (and outside it the frequencies) equal the closed
form; limits in the table.

## What was read (branch base 5638ad5a, read only)

- `AGENT_RULES.md`; the brief; the F-505 entry of `docs/roadmap.json`; ADR 0411 (all, with its addendum).
- `crates/rcfd-fem/src/rotating/whirl.rs` (all: `coriolis_matrix`, `Whirl`, `WhirlMode`, the dense solver `solve_dense` with
  `cholesky`, `lower_solve`, `skew_tridiagonal`, `tql`, `quadratic_residual`, `whirl_number`, `inertia`, `RotatingPart::whirl_at`
  with its reduction by `Path`), `crates/rcfd-fem/src/rotating.rs` (lines 1 to 160: the module comment, the `pub use` of
  `whirl`, `RotatingOptions`, `LossOfStiffness`).
- `crates/rcfd-fem/tests/si_rotating_whirl.rs` (lines 1 to 370 and 755 to 808: `Checks`, the cube CB, `translation_share`,
  `pair_of`, PM, L1, R1).

## Design

Notation as ADR 0411: speed `Omega >= 0` about the unit axis `a`; rotating-frame displacements `u`; `M` symmetric positive
definite, `K = K_eff(Omega)` symmetric (now of any inertia), `G = 2 Omega C` skew; all three reduced to the admissible motions
(`n` unknowns).

### D1. The state matrix (first principles)

`M u_tt + G u_t + K u = 0`. With `M = L L^T` (Cholesky) and `u = L^-T y`: `y_tt + G^ y_t + K^ y = 0`, `G^ = L^-1 G L^-T` (skew),
`K^ = L^-1 K L^-T` (symmetric), both formed as in ADR 0411 D2 and made exactly skew resp. symmetric by `1/2 (X -/+ X^T)`. With
the state `z = [y; y_t / c]`, `c > 0` a scale:

`z_t = A z`, `A = [[0, c I], [-K^ / c, -G^]]` (real, `2 n x 2 n`, not symmetric),

since `(y_t / c)_t = y_tt / c = -(K^/c) y - G^ (y_t / c)`. The eigenvalues `s` of `A` are those of the quadratic pencil
`Q(s) = s^2 M + s G + K` (`det Q(s) = det M det(s^2 I + s G^ + K^)`, and `det(s I - A) = det(s^2 I + s G^ + K^)` by the block
determinant with the commuting lower blocks), independent of `c`. The scale balances the two off-diagonal blocks:
`c = sqrt(k_max + g_max^2)`, `k_max = max |K^_ij|`, `g_max = max |G^_ij|` (`c = 1` if both are 0), so that `||A||` is of the
order of the largest `|s|` rather than of `k_max` (for the cube `k_max ~ 1e11`, `c ~ 3e5`; without the scale `||A|| ~ 1e11` and
the eigenvalue errors `eps ||A||` would be `~2e-5` absolute). The mode: `Q(s) phi = 0`; `u = Re(phi e^{s t})`; for `s = i omega`
this is ADR 0411's `(K - omega^2 M + i omega G) phi = 0`.

### D2. The non-symmetric eigensolver (Hessenberg reduction and the Francis double-shift QR algorithm)

Written from the algorithm as given in Golub and Van Loan, *Matrix Computations* (algorithms 7.4.2 Householder reduction to
Hessenberg form, 7.5.1 the Francis QR step, 7.5.2 the real Schur iteration) and in the Handbook procedure `hqr` of Martin, Peters
and Wilkinson (1970, Numer. Math. 14), after Francis (1961/62, The Computer Journal 4); all cited from memory, nothing taken from
them beyond what is stated and derived here. Eigenvalues only (the vectors by D4):

1. **Householder reduction to upper Hessenberg form** `H = P^T A P`: for column `k = 0 .. 2n - 3` the reflection `I - beta v v^T`
   that zeros `A[k + 2 .., k]` (with `v_{k+1} = a_{k+1,k} + sign(a_{k+1,k}) alpha`, `alpha` the norm of the column below the
   diagonal) applied from the left and the right. A similarity, so the spectrum is kept.
2. **Deflation**: the active window `[l, h]` ends where `|H[l, l-1]| <= eps (|H[l-1, l-1]| + |H[l, l]|)` (`eps` the machine
   epsilon; that sub-diagonal is set to 0). A window of size 1 gives a real eigenvalue `H[h, h]`; of size 2 the two eigenvalues of
   the 2 x 2 block `[[a, b], [c, d]]`: `t = (a + d)/2`, `q = ((a - d)/2)^2 + b c`; for `q >= 0` the real pair `r_1 = t + sign(t)
   sqrt(q)` (`sign(0) = +1`) and `r_2 = (a d - b c) / r_1` (no cancellation), or `r_2 = t - sign(t) sqrt(q)` when `r_1 = 0`; for
   `q < 0` the complex conjugate pair `t +/- i sqrt(-q)` (exact conjugates).
3. **Francis double-shift step** on a window of size `>= 3`: the shifts are the two eigenvalues of the trailing 2 x 2 block, used
   through their sum `s` and product `t`; the first column of `(H - s1)(H - s2)` restricted to the window is `x = h_ll^2 + h_l,l+1
   h_l+1,l - s h_ll + t`, `y = h_l+1,l (h_ll + h_l+1,l+1 - s)`, `z = h_l+1,l h_l+2,l+1`; a 3 x 3 Householder reflection of
   `(x, y, z)` creates the bulge, which further 3 x 3 reflections (and a final 2 x 2 one) chase down the window, each applied to
   the rows and columns of the window (eigenvalues only: the blocks outside the window do not change the window's spectrum).
   Exceptional shifts at the 10th and 20th iteration of a window (`s = 1.5 w`, `t = w^2`, `w = |h_h,h-1| + |h_h-1,h-2|`); more
   than 30 iterations on one window is refused (`FemError::Solver`, solver `"gyroscopic_state_eigen"`).

### D3. The structure of the spectrum and the stability flag

`Q(s)^T = s^2 M - s G + K = Q(-s)` (`G^T = -G`), so `det Q(-s) = det Q(s)`; and `Q` is real, so `det Q(conj s) = conj det Q(s)`.
**The spectrum is symmetric about both axes** (`s, -s, conj s, -conj s`): a root with `Re s != 0` always has a partner with
`Re > 0`. The motion is (marginally) stable iff every `s` is imaginary, i.e. every frequency real. The **growth rate** of the
speed is `sigma = max Re s` (1/s); the **flag**:

`Stable` iff `max_j |Re s_j| <= STABILITY_TOLERANCE * max_j |s_j|`, `STABILITY_TOLERANCE = 1e-8`; otherwise `Unstable { growth:
max_j Re s_j }`.

The tolerance (registered): the computed real part of a simple imaginary eigenvalue is a rounding error `~ kappa_s eps ||A||`
(QR is backward stable, not structure-preserving); with the scale of D1 `||A|| ~ max |s|` and `kappa_s` of order 1 to 100 here,
the relative defect is `~1e-14 .. 1e-13`, five orders below `1e-8`. The tolerance hides a genuine growth below `1e-8 max|s|`
(for PM `~2e-5` 1/s; near a band edge the growth falls as `sqrt(distance)`, so the flag edge moves by `~1e-12` rad/s, arithmetic
3). At a double eigenvalue that is defective (a Jordan block, at a band edge itself) the rounding is `~sqrt(eps) |s|` and the flag
there is not reliable (limits).

**Odd index (Kelvin-Tait-Chetaev, cited from memory; this part derived).** `det(s^2 I + s G^ + K^) = prod_j (s - s_j)`, at `s = 0`:
`det K^ = prod_j s_j` (`2 n` factors). A spectrum on the imaginary axis in conjugate pairs has `prod s_j = prod omega_j^2 >= 0`.
So if `K` has an odd number of negative eigenvalues (`det K^ < 0`) the speed is unstable for any `G`; with an even number it may
be stabilised by `G` (the point mass above `omega_0`: two negative, stable).

**Reported modes.** The eigenvalues with `Im s > 0`, and the real ones with `Re s >= 0`, ordered by `(Im s, Re s)`: one of each
pair `s, conj s` and of each real pair `+/- sigma`; for a stable speed these are the `n` values `i omega_j`. For each: `omega =
Im s` (rad/s, `>= 0`), `growth = Re s`, `frequency = omega / 2 pi`.

### D4. The mode vectors

**Inverse iteration** on the complex `n x n` matrix `Q(s)` (written in real pairs; no complex type exists in the repository and
none is added): LU with partial pivoting by modulus, a pivot of modulus below `eps max|Q_ij|` replaced by `eps max|Q_ij|` (the
matrix is singular to rounding at an eigenvalue); three solves `Q(s) x_{k+1} = x_k` from a fixed irregular start vector, each
normalised. Then the normalisation of ADR 0411 D2 (`phi^H M phi = 1`, the entry of largest modulus real and positive), the
residual `||Q(s) phi|| / (||K phi|| + |s|^2 ||M phi|| + |s| ||G phi||)` and, on a part, the whirl number `kappa = Im(phi^H C
phi) / phi^H M phi` with its direction (ADR 0411 D3; for `s = i omega` exactly that definition). For a semi-simple double
eigenvalue the vector is some member of the eigenspace; for a defective one it is approximate (limits).

### D5. Closed forms (first principles)

A point mass `m` in the plane normal to `a = e_z`, springs `k_x`, `k_y` fixed in the rotating part, `omega_x^2 = k_x/m`,
`omega_y^2 = k_y/m`, `omega_x <= omega_y`. Rotating frame (ADR 0411 PM, with the spin softening `-m Omega^2` of in-plane motion):

`x_tt - 2 Omega y_t + (omega_x^2 - Omega^2) x = 0`, `y_tt + 2 Omega x_t + (omega_y^2 - Omega^2) y = 0`.

With `e^{s t}` and `a = omega_x^2 - Omega^2`, `b = omega_y^2 - Omega^2`: `(s^2 + a)(s^2 + b) + 4 Omega^2 s^2 = 0`, so with
`lambda = s^2`:

`lambda^2 + S lambda + P = 0`, `S = omega_x^2 + omega_y^2 + 2 Omega^2 > 0`, `P = (omega_x^2 - Omega^2)(omega_y^2 - Omega^2)`,

discriminant `D = S^2 - 4 P = (omega_x^2 - omega_y^2)^2 + 8 Omega^2 (omega_x^2 + omega_y^2) > 0` (expanded: `S^2 - 4P = p^2 +
4 p Omega^2 + 4 Omega^4 - 4 omega_x^2 omega_y^2 + 4 Omega^2 p - 4 Omega^4`, `p = omega_x^2 + omega_y^2`). Both roots are real,
`lambda_- = -(S + sqrt D)/2 < 0` and `lambda_+ = P / lambda_-` (Vieta; no cancellation near the band edges).

- **Isotropic** (`omega_x = omega_y = omega_0`): `sqrt D = 4 omega_0 Omega`, `lambda = -(omega_0 -/+ Omega)^2`, `s = +/- i
  |omega_0 -/+ Omega|`: **above `omega_0` the frequencies `Omega - omega_0` and `Omega + omega_0`, all real: stable**, although
  `K_eff = m (omega_0^2 - Omega^2) I` is negative definite (two negative eigenvalues, even: gyroscopic stabilisation). Mode:
  `Q(i omega)` on `(1, i)` is `m(omega_0^2 - Omega^2 - omega^2 + 2 Omega omega)`, zero at `omega = Omega -/+ omega_0`; on `(1,
  -i)` it is zero at `omega = -Omega +/- omega_0 < 0` (no positive root above `omega_0`). So both positive frequencies have the
  vector `(1, i)`, the orbit `(cos, -sin)`, clockwise about `e_z`: **both modes whirl backward**, `kappa = -1` (ADR 0411 D3 gave
  `(1, -i)` `kappa = +1`). Below `omega_0` the forward branch is `omega_0 - Omega`; it passes through 0 at `omega_0` and comes
  back as the backward `Omega - omega_0`.
- **Anisotropic** (`omega_x < omega_y`): **for `omega_x < Omega < omega_y`, `P < 0`, `lambda_+ > 0`, `s = +/- sqrt(lambda_+)`
  real: unstable (divergence) with the growth rate `sigma = sqrt(lambda_+)`** and one real frequency `sqrt(-lambda_-)`; for
  `Omega < omega_x` or `Omega > omega_y` (`P > 0`) both `lambda` negative: stable with the frequencies `sqrt(-lambda_+)`,
  `sqrt(-lambda_-)`. The band is exactly `(omega_x, omega_y)`; `K_eff` has one negative eigenvalue there (odd, D3) and two above
  it (even, stabilised).

### D6. Interfaces (all new, in `rotating/whirl/state.rs`, re-exported from `rotating`)

`STABILITY_TOLERANCE`; `Stability::{Stable, Unstable { growth }}` with `is_stable()`; `gyroscopic_state_eigen_dense(k, m, g,
n) -> DenseStateWhirl { eigenvalues: Vec<[f64; 2]> (all 2n, (re, im), ordered by (im, re)), stability, real_defect (max|Re s| /
max|s|), growth, omegas, re, im, residuals (the reported modes, D3, D4) }` (`k` symmetric of any inertia, `m` symmetric positive
definite, `g` skew; refuses more than `MAX_WHIRL_UNKNOWNS` (`"unknowns"`, checked first), matrices of the wrong length or with
non-finite entries (`"matrix"`), an `m` not positive definite (`"mass"`)); `StateWhirlMode { growth, omega, frequency, re, im,
whirl, direction, residual }`; `StateWhirl { speed, loss: Option<LossOfStiffness> (the inertia test of ADR 0403 D3; None when
K_eff is positive definite), eigenvalues, stability, real_defect, modes (the lowest options.modal.modes reported ones, expanded to
the model's equations), unknowns }`; `RotatingPart::whirl_state(omega)` (any finite speed `>= 0`; `"speed"` otherwise; `G = 2
Omega C` with `options.coriolis`, 0 without, as ADR 0411); `RotatingPart::whirl_stability(speeds) -> WhirlStability { speeds,
results: Vec<StateWhirl> }` (speeds refused as ADR 0403 D4, `"speeds"`) with `to_csv()`: columns `speed_rad_s,
negative_stiffness, stable, growth_rate_1_s` and `f_<j>_hz` for the reported modes (the count of negative eigenvalues of
`K_eff`, 0 when positive definite, empty when the factorisation met a zero pivot; `growth_rate_1_s` 0 for a stable speed),
shortest round-trip numbers.

## Set-up (fixed now)

Material and axis as ADR 0411: steel `E = 2e11 Pa`, `nu = 0.3`, `rho = 7850 kg/m^3`, `e_z` through the origin; `omega_0 = 1000
rad/s`, `m = 7.85 kg`.

**PMI (isotropic point mass, dense):** `M = m I_2`, `K = m (omega_0^2 - Omega^2) I_2`, `G = 2 Omega m [[0, -1], [1, 0]]`, by
`gyroscopic_state_eigen_dense`; speeds `Omega / omega_0 = 1.1, 1.5, 2, 3, 5`.

**PMA (anisotropic point mass, dense):** `K = m diag(omega_x^2 - Omega^2, omega_y^2 - Omega^2)`, `omega_x = omega_0`, `omega_y
= 1.2 omega_0`; speeds `Omega / omega_0 = 0.5, 0.9, 1.02, 1.05, 1.1, 1.15, 1.18` (the last five in the band) `, 1.3, 1.5, 2, 3`.

**CBI (the cube on isotropic springs of ADR 0411 CB):** one Hex8 cube `h = 0.1 m` centred on the origin, `u_z = 0`, nodal springs
`k_n = m omega_0^2 / 8` in `x` and `y` at all 8 nodes, both effects on, `RotatingOptions::new(4).coriolis(true)`. The translation
pair spans an exact invariant subspace (ADR 0411 CB), so D5 isotropic holds for it exactly. Torsion on the springs: `omega_t^2 ~
3 omega_0^2 - Omega^2` (ADR 0411 L1; positive below `~1.73 omega_0`). Speeds `0.5, 1.2, 1.5, 2.0 omega_0`.

**CBA (the cube on anisotropic springs):** as CBI with `k_x = m omega_x^2 / 8`, `k_y = m omega_y^2 / 8` per node (`omega_x =
omega_0`, `omega_y = 1.2 omega_0`). The subspace `span{1_x, 1_y}` stays invariant (`K_spring 1_x = k_x 1_x`, `K 1_v = 0`,
`K_sigma 1_v = 0`, `M_perp 1_v = M 1_v`, `C 1_v = M 1_{e_z x v}`, `M 1_v = (m/8) 1_v`, ADR 0411 CB), on which the equations are
PMA's: D5 anisotropic holds exactly for the pair. Torsion `omega_t^2 = 12 (k_x + k_y)/m - Omega^2 = 1.5 (omega_x^2 + omega_y^2)
- Omega^2 = (3.66 - (Omega/omega_0)^2) omega_0^2` (8 nodes at `(+/-h/2, +/-h/2)`: torsional stiffness `2 h^2 (k_x + k_y)`, inertia
`m h^2 / 6`), positive at every CBA speed. Speeds `0.9, 1.05, 1.1, 1.18, 1.3 omega_0`.

The translation pair is picked among a result's modes by its complex share in `span{1_x, 1_y}` `>= 1 - 1e-8` (ADR 0411's
`translation_share`).

## Arithmetic (done before the code; disclosed)

1. PMI through `numpy.linalg.eigvals` of `[[0, I], [-M^-1 K, -M^-1 G]]`: at `1.1 .. 5 omega_0` the positive imaginary parts
   `Omega - omega_0`, `Omega + omega_0` to `<= 3e-15` relative, `max |Re s| <= 6.4e-12` (`<= 1.1e-15` of `max|s|`).
2. PMA, closed form D5 against numpy (agreeing to `~1e-12` relative; the closed form with `lambda_+ = (-S + sqrt D)/2` loses
   `~1e-12` to cancellation, hence Vieta in D5): stable at `0.5` (frequencies `586.3168, 1611.2829` rad/s), `0.9` (`172.3369,
   2007.5607`); unstable at `1.02` (growth `59.7344` 1/s, frequency `2127.0562`), `1.05` (`86.2300`, `2156.9505`), `1.1`
   (`99.5893`, `2206.7891`), `1.15` (`86.2623`, `2256.6438`), `1.18` (`59.7702`, `2286.5635`); stable at `1.3` (`172.6026,
   2406.2852`), `1.5` (`386.1302, 2605.9362`), `2` (`892.4021, 3105.4176`), `3` (`1894.5007, 4104.9808`).
3. Flag edge: at `Omega = omega_x + delta` the growth is `~sqrt(198 delta)` 1/s (`sigma^2 ~ 2 omega_x delta (omega_y^2 -
   omega_x^2) / (3 omega_x^2 + omega_y^2)`), equal to `1e-8 max|s| ~ 2.1e-5` at `delta ~ 1e-12` rad/s (numerically: `delta =
   1e-12` gives `2.16e-5`). Near the edge the near-defective double root at 0 carries a rounding `~sqrt(eps ||A||^2) ~ 3e-5`,
   which moves the flag edge by `(3e-5)^2/198 ~ 5e-12` rad/s. So bisection of the flag finds the edges within `~1e-14` relative.
4. Without Coriolis, CBI at `1.2 omega_0`: the translation pair diverges with `sigma = sqrt(Omega^2 - omega_0^2) = 663.3250`
   1/s; PMI at `1.5 omega_0` with `G = 0`: `sqrt(1.25) omega_0 = 1118.0340`.
5. Rounding of the cube (ADR 0411 arithmetic 3 for the element; D1 for the scale): `||A|| ~ 3e5`, eigenvalue errors `~10 eps
   ||A|| ~ 7e-10` absolute, against the smallest checked value `0.2 omega_0 = 200` (CBI at 1.2) `~4e-12` relative; growth rates
   `>= 59` 1/s, `~1e-11` relative. Estimates `<= 1e-10` against the limits `1e-8`.

## Checks (limits fixed now)

| # | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| A1 | c2 | the state solver on the random gyroscopic problems of ADR 0411's unit test (SPD `K`, `M`, skew `G`, `n = 1, 2, 5, 12`, the same seed): its `2 n` eigenvalues against `+/- i omega` of `gyroscopic_eigen_dense`, each matched in order: `max |s - s_ref| / max|s|`; and the flag `Stable` | `<= 1e-10`; each | `~1e-14` (D3) |
| A2 | c2 | the real Schur iteration (D2, the internal function) on the companion matrix of `(s - 1)(s - 2)(s - 3)(s^2 + 1)(s^2 + 2 s + 5)`: the 7 roots `1, 2, 3, +/- i, -1 +/- 2i`, `max |s - s_exact| / 3` | `<= 1e-9` | `~1e-13` (simple, well separated roots) |
| B1 | c2 | CBI at `1.2 omega_0`: `whirl_state` gives `loss == Some(LossOfStiffness { negative: Some(2), .. })` (K_eff not positive definite, as `whirl_modes` reports), `2 n` finite eigenvalues, `n` reported (`n = 16`) | each | D3 |
| Q1 | c2 | the stored residual of every returned mode (PMI, PMA, CBI, CBA at all their speeds) | `<= 1e-9` | `~1e-12` (eigenvalue error over `|s|`) |
| X1 | c2 | CBI at `0.5 omega_0` (`K_eff > 0`): the reported frequencies of `whirl_state` against `frequencies_all` of `whirl_modes` (ADR 0411's solver), count equal, `max |omega/omega_ref - 1|`; `loss == None`; flagged `Stable` | count; `<= 1e-9`; each | `~1e-12` |
| F1 | c3 | CBI at `1.2 omega_0` with `coriolis(false)`: `Unstable`, `|growth - sqrt(Omega^2 - omega_0^2)| / omega_0` | flag; `<= 1e-8` | `~1e-12` (arithmetic 4) |
| F2 | c3 | CBI at `2.0 omega_0`: `loss.negative == Some(3)` (the pair and the torsion, odd) and flagged `Unstable` (D3 odd index) with growth `> 0` | each | D3; torsion `3 - 4 < 0` |
| F3 | c3 | `whirl_stability` of CBI over `0.5, 1.2, 1.5, 2.0 omega_0`: each result equal to `whirl_state` of the speed (by `==`); `to_csv`: `1 + 4` lines, `stable` column `true, true, true, false`, `negative_stiffness` `0, 2, 2, 3`, every growth and frequency parses back to its bits | each | D6 |
| P1 | c4 | PMI at `1.1, 1.5, 2, 3, 5 omega_0`: two reported modes, `|omega_0' / (Omega - omega_0) - 1|` and `|omega_1' / (Omega + omega_0) - 1|`; growth parts | `<= 1e-8`; each flagged `Stable` | `~3e-15` (arithmetic 1) |
| P2 | c4 | PMI: both modes `kappa <= -1 + 1e-8` (`kappa = 2 r^T C s / (r^T M r + s^T M s)` with `C = m [[0, -1], [1, 0]]`, as ADR 0411's PM test), i.e. backward | each | D5 |
| P3 | c4 | CBI at `1.2`, `1.5 omega_0`: `loss.negative == Some(2)`; the translation pair against `Omega -/+ omega_0` (relative to each value); both `kappa <= -1 + 1e-8` and `Whirl::Backward`; flagged `Stable` | each; `<= 1e-8`; each; each | `~4e-12` (arithmetic 5) |
| AN1 | c5 | PMA at its 11 speeds: flagged `Unstable` exactly at `1.02 .. 1.18` and `Stable` at the other six; inside: `|growth - sigma_cf| / omega_0` and the one positive frequency against `sqrt(-lambda_-)`; outside: both frequencies against `sqrt(-lambda_+)`, `sqrt(-lambda_-)` (`/ omega_0`) | flags exact; `<= 1e-8` | `~1e-14` (arithmetic 2) |
| AN2 | c5 | PMA: bisection of the flag, 60 halvings, on `[0.95, 1.1] omega_0` (stable, unstable) and on `[1.1, 1.3] omega_0` (unstable, stable): `|Omega_edge / omega_x - 1|`, `|Omega_edge / omega_y - 1|` | `<= 1e-8` each | `~1e-14` (arithmetic 3) |
| AN3 | c5 | CBA at `0.9, 1.05, 1.1, 1.18, 1.3 omega_0`: flagged `Unstable` exactly at `1.05, 1.1, 1.18`; growth against `sigma_cf` (`/ omega_0`) inside; the translation-share modes with `omega > 0` against the closed-form frequencies (`/ omega_0`); `loss`: `None`, `Some(1)` x 3, `Some(2)` | flags exact; `<= 1e-8`; `<= 1e-8`; exact | `~1e-11` (arithmetic 5) |
| R1 | D6 | refusals: `whirl_state(-1.0)`, `whirl_state(NaN)` (`"speed"`); `whirl_stability(&[0, 2, 1])` (`"speeds"`); dense with a matrix of the wrong length (`"matrix"`), with `m` not positive definite (`"mass"`), with `n = 1501` (`"unknowns"`) | each its key | D6 |
| R2 | rule 3 | the existing tests of `rcfd-fem` pass unchanged (release), in particular `si_rotating_whirl` (with L1: `whirl_modes(1.2 omega_0)` still a `LossOfStiffness`); `cargo check -p rcfd-post --tests` | pass | additive edits |

Negative controls (expected to fail the property, each with a limit):

| # | Control | Expected |
|---|---|---|
| N1 | F1 is itself the control of P3: without Coriolis the cube at `1.2 omega_0` is flagged `Unstable` | growth `> 1e2` 1/s (`663`) |
| N2 | PMI at `1.5 omega_0` with `G = 0` (dense) | flagged `Unstable`, growth within `1e-8 omega_0` of `1118.0340` and `> 1e3` |

Planned mutations (on a `git archive` copy in `f505-scratch`, each run once against `si_rotating_whirl_state` and the unit tests
of `whirl::state`; what catches each is recorded):

1. `G^` dropped from the state matrix (`A = [[0, c I], [-K^/c, 0]]`): P1, P3, AN1 (the stabilised speeds become unstable).
2. The sign of `G^` in the state matrix reversed: P2, P3 (the spectrum is the same by D3, the vectors from `Q(s)` with the true
   `G` then belong to `-s`: residuals Q1 and the whirl directions).
3. The imaginary part of a complex 2 x 2 block taken as `sqrt(-q) / 2`: A1, A2, P1, AN1.
4. `STABILITY_TOLERANCE = 1e-2`: AN2 (the flag edges move by `~2` rad/s, `2e-3` relative).
5. Inverse iteration on `Q(conj s)`: Q1, P2.
6. The right-hand application of the Francis reflections left out: A1, A2, P1.
7. In `whirl_state`, `G = Omega C` (the factor 2 dropped): P3, AN3.
8. The reported set from `Im s < 0` instead of `> 0`: P1, X1 (negative frequencies).

## Decision rule

- **done**: A1 to R2 met, N1 and N2 as expected, and all eight mutations caught.
- **partial**: A1, B1, Q1, P1 and AN1 met (clauses c2 to c5 in their core reading) with another check missed and its cause
  diagnosed (a `*_known_miss` record), or a mutation not caught, or a check met only after an amendment made after its run.
- **todo**: A1, P1 or AN1 missed, or a stable speed of PMI flagged unstable.

## Limits (what is not covered)

The cyclic sector (`RotatingSector`): the state solver is not wired to the harmonic reduction (follow-up); tracking of the
supercritical branches in a Campbell table (the stability table gives per-speed results only, untracked); damping (the state
matrix would take `-M^-1 D`; not asked); the flag at a defective double eigenvalue (the band edges themselves, a Krein collision),
where rounding is `~sqrt(eps)` and the mode vector is approximate; large parts (the dense cap of ADR 0411, 1500 reduced unknowns,
the QR `O((2n)^3)`); frequencies in the stationary frame; output through case files and the CLI.
