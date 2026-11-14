# ADR 0504: F-562 registration: damped massless unknowns of a rotor solved as first-order unknowns

Status: **REGISTERED 2026-11-14, written before any code of F-562 and before any run of its checks** (one disclosed probe of
the existing solver, below, was run before the registration). Not yet accepted by the maintainer. Results go in the addendum at
the end; nothing above the addendum is changed after a run.

Roadmap item F-562 (deps F-416), structural solver, the follow-up proposed by ADR 0476. Number 0504 given by the lead. Branch
`f562-work` from d85922b8 (the head of the F-564 branch, which sits on the F-563 branch; ADRs 0498 and 0501 and
`crates/rcfd-fem/src/rotor.rs` are shared with them), worktree `w49i-work/f562`, scratch `w49i-work/f562-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical (the new behaviour is reached only through a new method); no edit of `baseline_hashes.rs`, of the hash tables or
of any golden file; no existing test edited; no new dependency, no unsafe code; clean-room. Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation), except the open questions at the end.

## What F-562 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. A bearing pedestal with stiffness and damping in series and no mass gives the whirl of the same model with a pedestal mass of 1e-9 of the disc's within 1e-6; models without such unknowns bit-identical.

Clauses: **c1** the whirl of a model with a massless damped pedestal equals, within 1e-6, the whirl of the same model with a
pedestal mass of 1e-9 of the disc's; **c2** models without a massless damped unknown are bit-identical. The context asks for
the formulation to be derived, the matching of roots between the two models to be registered (the 1e-9 model has spurious
very fast roots), the existing refusal to be found and lifted only for the new path, and 1e-6 to be confirmed reachable given the
conditioning of the 1e-9 model.

## What was read (base d85922b8, read only)

ADR 0476 (F-416) D3 to D5, ADR 0498 and 0501 (style, models), and `crates/rcfd-fem/src/rotor.rs`:

* `RotorModel::whirl(omega)` -> `whirl_with`. Unknowns are `(u_x, u_y, theta_x, theta_y)` per station. An unknown whose row and
  column of `M`, of `C` and of `G` are all exactly zero is condensed statically by `Condensation` (D3). Every other unknown is
  kept; then `whirl_with` requires every kept `M_ii > 0` and refuses otherwise with key `"mass"` ("a massless unknown with a
  damper or a gyroscopic term is not taken"). This is the refusal to lift. It is pinned by the existing test
  `sa_rotor.rs::x0_refusals` (`jeffcott(0.0, 0).whirl(1.0)` gives `"mass"`), which is not edited: the refusal of `whirl` stays,
  and the capability is a new method (D1).
* The kept problem is the quadratic `lambda^2 M q + lambda D q + K q = 0`, `D = C + Omega G`, solved by
  `quadratic_eigen_with` (ADR 0450: Cholesky of `M`, state matrix of order `2n`, real Schur form), in the time scale `tau` of
  ADR 0472 D3. A massless unknown makes `M` singular, so the Cholesky fails: this is why the unknown is refused today.
* A node-to-node spring or damper does not exist in `RotorModel`: bearings connect a station to ground, shaft elements connect
  stations. A pedestal with stiffness and damping in series is therefore a station `P` that carries a ground bearing
  `(k_p, c_p)`, joined to the rotor by massless shaft elements (density 0): the shaft's flexibility is the series spring, the
  bearing at `P` the parallel spring and damper of the pedestal. With no disc at `P` and shaft density 0, every entry of the
  `M` row of the translations of `P` is exactly zero while `C` is not: the unknown is massless and damped. The existing model
  `sa_rotor.rs::jeffcott(0, 0)` is a model of this kind (a damper at the massless end station).
* The tests `sa_rotor.rs`, `sa_rotor_speed_default_bits.rs`, `sa_rotor_peaks_default_bits.rs` (patterns for the bits check).

## Arithmetic and a probe done before the registration (disclosed)

With python (not part of the repository) the closed form of D4 was evaluated for model A: shaft `L = 1 m`, `r = 0.025 m`,
`E = 2e11`, `nu = 0.3`, `kappa = 0.9`, so `k_s = 2.93e6 N/m` (D4); with `m = 20 kg`, `k_p = 2e6`, `c_p = 2000` the cubic of D4 has
the roots `-17.7244 +- 293.2603 i` and `-1696.895`, and the pedestal's tilt root is `-k_p/c_p = -1000`.

**Probe (existing code, nothing of F-562 written; run in an uncommitted test file, deleted after the run).** The existing
`whirl(0)` was run on model A with a pedestal mass `mu` times the disc's, `mu = 1e-8, 1e-9, 1e-10`, and compared with the exact
cubic of D4. Result (disclosed because it fixes the registration of c1):

| `mu` | complex mode 1, `Im lambda` | mode 2 (the degenerate x/y partner), `Im lambda` | spurious roots |
|---|---|---|---|
| exact cubic | 293.26029944 | 293.26029944 | none |
| 1e-8 | 293.26029805 | 293.26029966 | real, `-1.0e10` |
| 1e-9 | 293.26030764 | 293.26088542 | real, `-1.0e11` |
| 1e-10 | 293.26025757 | 293.26409897 | real, `-1.0e12` |

The truncation error of the 1e-9 model (relative `~5e-10`, linear in `mu`: `4.8e-9` at 1e-8) is far inside 1e-6. But the
existing solver, which forms `L^-1 K L^-T` with `L L^T = M` of entries `2e-8`, loses accuracy as the mass shrinks: the two
members of the degenerate x/y pair (identical in exact arithmetic) differ by `2.0e-6` relative at `mu = 1e-9` and by `1.3e-5` at
`1e-10`. So **a comparison against the roots of the existing solver for the 1e-9 model cannot be expected to reach 1e-6**
(expected miss of check A3 below, registered with the limit of the acceptance and diagnosed). For that reason the
registration also compares with the exact roots of the 1e-9 model, obtained without the singular-mass conditioning (check
A2: the quartic of D4; B1: a bordered Newton polish of the 1e-9 model's own matrices, D6). The probe values are not used to fix
any limit below except to explain A3.

## Design

### D1. API

`RotorModel::whirl_first_order(omega) -> Result<RotorWhirl>`. For a model with no unknown that is massless and damped it runs
the same code as `whirl` and returns the same bytes (c2; the shared code is moved into a helper without change of arithmetic
and the bytes are pinned by B1). Only when such unknowns exist does it take the first-order path of D2. `whirl` itself keeps
refusing them with `"mass"` (the lifted refusal is the new method's; `x0_refusals` stays valid). `RotorWhirl::roots` then holds
`2 n_m + n_0` roots (D2) instead of `2 n`; the doc says so.

The refusals of `whirl_first_order`: a negative or non-finite speed (`"speed"`); as `whirl` for every other case; plus
`"mass"` when a kept unknown has `M_ii <= 0` but a nonzero entry in its `M` row or column (a partly massless unknown is not a
first-order unknown) or when there is no unknown with mass at all (no inertia, no modes); `"damping"` when the massless damped
block `D_00` is singular (D2: the pencil then has roots at infinity, a model with a massless unknown that no damper holds).

### D2. The formulation (derived)

After the static condensation of the undamped massless unknowns (exact: their equation `K_bb q_b + K_ba q_a = 0` holds at every
frequency, so the condensed `K*` of D3 of ADR 0476 is exact), the kept unknowns split into `m` (rows of `M` not all zero, `M_mm`
symmetric positive definite, `n_m` of them) and `0` (rows of `M` all zero, `n_0`). With `D = C + Omega G` the equations are

```
M_mm q_m'' + D_mm q_m' + D_m0 q_0' + K_mm q_m + K_m0 q_0 = 0
            D_0m q_m' + D_00 q_0' + K_0m q_m + K_00 q_0 = 0
```

The second block has no second derivative. Take the states `x = (q_m, v_m, q_0)` with `v_m = q_m'` (displacement only for the
massless unknowns: no velocity state) and `x = x_0 e^{lambda t}`:

```
lambda q_m                    = v_m
lambda (M_mm v_m + D_m0 q_0)  = -(K_mm q_m + D_mm v_m + K_m0 q_0)
lambda (D_00 q_0)             = -(K_0m q_m + D_0m v_m + K_00 q_0)
```

that is the generalized problem `A x = lambda B x`, of size `N = 2 n_m + n_0`, with

```
A = [[ 0,    I,     0   ],        B = [[ I, 0,    0   ],
     [-K_mm, -D_mm, -K_m0],             [ 0, M_mm, D_m0],
     [-K_0m, -D_0m, -K_00]],            [ 0, 0,    D_00]].
```

`B` is block upper triangular with diagonal blocks `I`, `M_mm` (positive definite) and `D_00`, so it is nonsingular exactly
when `D_00` is: the pencil has `N` finite roots, the degree of `det(lambda^2 M + lambda D + K)` when `D_00` is nonsingular
(the `n_0` rows of the quadratic carry degree 1 only). The solver applies `S = B^-1 A` (block solves with `D_00` and `M_mm`,
real LU) and takes the eigenvalues of `S / tau` by `general_eigen_with` (the same real Schur solver as ADR 0450, with
balancing) in the time scale `tau`: `tau = sqrt(max_{i in m} K_ii/M_ii) + max_{i in m} sum_j |D_ij| / M_ii + max_{i in 0} |K_ii|
/ |D_ii|` (`>= 1` replaced if not positive and finite), and `lambda = tau mu`. Real roots, the choice of the roots with
`Im > 0`, the sort, the cluster resolution of ADR 0476 D5 into circular orbits, the whirl number and the stability flag are the
code of `whirl` (the tail is shared). The mode shape on the stations is `(q_m, q_0)` of the eigenvector, scaled so that
`x^H M x = 1` (`M` of the kept unknowns) and the entry of largest modulus real and positive, as in `whirl`, and the condensed
unknowns recovered by `R` (D3 of ADR 0476). `RotorWhirlMode::residual` is the relative residual of the first-order problem.
For `n_0 = 0` the formulation reduces to the state matrix of `whirl` (the method does not take it: D1).

No published source is used beyond the textbook reduction of a quadratic eigenproblem with a singular leading coefficient to
a first-order or generalized one (the same construction as the state-space form of ADR 0450 D2, extended by the algebraic
rows); it is derived above.

### D3. Matching of roots between the massless model and the 1e-9 model

Let `R_0` be the `N` roots of the massless model (`whirl_first_order`, the real ones and both of a complex pair) and `R_mu` the
`2 (n_m + n_0)` roots of the model with a pedestal mass `mu` (the pedestal mass is added as a `Disc` with mass only, so its
translations are the added unknowns; the rotations of `P` have no inertia in either model and stay condensed). The `n_0` extra
roots are the spurious fast ones, at about `-c_p/mu_p` (`mu_p` the pedestal mass) for an overdamped pedestal translation. Matching
(registered, not chosen after seeing results): each root of `R_0` in order of decreasing modulus takes the nearest unused root
of `R_mu` (distance `|a - b|`); the error of the pair is `|a - b| / |a|`; the error of the model is the maximum over the pairs.
After matching the `n_0` unused roots of `R_mu` must all have modulus at least `1e3` times the largest modulus in `R_0` (they are
the spurious ones; checked, finite). The modes (`Im > 0`) are compared the same way with the directions of the matched modes
required equal. The mode shape of a matched pair is compared by the modal assurance criterion on the full station vectors.

### D4. Closed forms (derived here)

Model A: massless simply supported shaft (density 0; four equal elements, stations `z = 0, 0.25, 0.5, 0.75, 1 m`, `L = 1`) with
a disc of mass `m` at the midpoint (station 2, no inertia) and identical isotropic pedestals `(k_p, c_p)` at stations 0 and 4
(bearing at the station, no mass: massless damped). By symmetry the disc translates without rotation; the shaft's midpoint
stiffness relative to its supports is `k_s = 1/(L^3/(48 E I) + L/(4 kappa G A))` (bending `P L^3/(48 E I)` plus shear
`P L/(4 kappa G A)` under a midspan load `P`, the F-391 element exact for it), the end rotations being free and massless. In
the Laplace variable `s` (`lambda`): the shaft transmits `F = k_s (u - u_p)`, each pedestal carries `F/2` through the impedance
`z_p = k_p + c_p s + mu_p s^2` (`mu_p = 0` massless), the disc obeys `m s^2 u + F = 0`. So `u = F (1/k_s + 1/(2 z_p))` and

```
m s^2 (2 z_p + k_s) + 2 k_s z_p = 0.
```

With `mu_p = 0` this is the cubic `2 m c_p s^3 + m (2 k_p + k_s) s^2 + 2 k_s c_p s + 2 k_s k_p = 0` (one complex pair and one
real root in each of the x and y planes, which are degenerate); the rigid tilt of the shaft on the two pedestals (disc fixed) is a
second family with the single real root `-k_p/c_p`. With `mu_p > 0` it is the quartic with the added terms
`2 m mu_p s^4 + 2 k_s mu_p s^2`. The reference roots are those of these polynomials by the companion matrix of
`general_eigen` followed by Newton polish in complex arithmetic (a residual below `1e-12` relative). Nothing in A couples a
massive unknown to a massless one through `D` (`D_m0 = D_0m = 0` in every model expressible with `RotorModel`: damping enters
only through diagonal-station bearings and `G` of massless shafts is zero), so D5's `U1` tests those blocks.

### D5. A direct test of the dense routine

The dense routine of D2 (matrices `M`, `D`, `K` of the kept unknowns and a mask of the massless ones in, roots and vectors out)
is a private function with a unit test in `rotor.rs` (`U1`): a 4-unknown system with one massless unknown and nonzero
`D_m0`, `D_0m`, `K_m0`, `K_0m`, nonsymmetric `K`, `D`, a fixed generic set of numbers (below). The returned pairs satisfy
`(lambda^2 M + lambda D + K) x = 0` on the original unknowns to `1e-10` relative to `|lambda|^2||M|| + |lambda| ||D|| + ||K||`,
and the count is `2 n_m + n_0 = 5`.

### D6. The exact reference for the 1e-9 model (bordered Newton)

For the model with pedestal mass `mu_p`, matrices `M, D, K` from `RotorModel::matrices_at` on the free unknowns, a root `lambda`
and vector `x` solve `Q(lambda) x = 0`, `v^H x = 1` (`Q = lambda^2 M + lambda D + K`, `v` a unit vector at the entry of largest
modulus of the starting vector). Newton on the bordered system `[[Q, Q'(lambda) x], [v^H, 0]] (dx, dlambda) = -(Q x, v^H x - 1)`,
`Q' = 2 lambda M + D`, complex Gaussian elimination with partial pivoting, started from the root of the existing solver, stopped
at `|dlambda| <= 1e-14 |lambda|` or 40 steps; it is a test helper. It does not form `L^-1 K L^-T`. Its trust is checked by P1.

## Models and parameters (fixed here from the derivation, not searched)

Common: `E = 2e11`, `nu = 0.3`, `kappa = 0.9`, `circle_section(0.025, 0.9)`, shaft density 0, stations `z = 0, 0.25, 0.5, 0.75, 1`,
four shaft elements, `m = 20 kg`.

* **A**: D4: disc `m` at station 2 (no inertia), `Bearing::isotropic(2e6, 2000)` at stations 0 and 4. Speeds 0 and 500 rad/s
  (no gyroscopic term: the roots do not depend on the speed). Reference model `A_mu`: the same with a `Disc` of mass
  `mu m`, `mu = 1e-9`, at stations 0 and 4 (inertias 0).
* **B**: gyroscopic and anisotropic: discs `(m = 20, I_t = 0.05, I_p = 0.1)` at station 1 and `(5, 0.01, 0.02)` at station 3;
  pedestal at station 0: `k = [[2e6, 1e5], [-5e4, 3e6]]`, `c = [[1500, 100], [60, 2500]]` (cross-coupled, `c` not symmetric);
  at station 4: `k = [[1.5e6, 0], [0, 2.5e6]]`, `c = [[2000, 0], [0, 1000]]`. Speeds `0, 300, 900 rad/s`. `B_mu`: `Disc`
  of mass `mu * 20 = 2e-8` (no inertia) at stations 0 and 4. `n_m = 8`, `n_0 = 4`, `N = 20` roots in B (24 in `B_mu`).
* Refusal models R1 (`"damping"`): A with the pedestal at station 0 replaced by `Bearing::new(k = diag(2e6, 2e6), c = [[0, 100],
  [0, 0]])` (the damping row of `u_y` zero, `D_00` singular). R2 (`"mass"`, no mass): the shaft with one pedestal bearing and no
  disc. R3 (`"mass"`, partly massless): not constructible with `RotorModel` (a station with mass has `M_ii > 0`); covered by
  U1's construction through the dense routine's refusal of a zero diagonal with nonzero row.
* U1 numbers: `M = [[2, 0.3, 0, 0], [0.3, 3, 0, 0], [0, 0, 1.5, 0], [0, 0, 0, 0]]` (the fourth unknown massless), `D = [[0.4, 0.1, 0,
  0.2], [0.1, 0.5, 0.1, -0.1], [0, 0.2, 0.3, 0.05], [0.15, -0.05, 0.1, 0.6]]`, `K = [[50, 5, 0, 8], [5, 70, 3, -2], [0, 4, 60, 6],
  [7, -1, 5, 40]]`.

## Checks (limits fixed now; each run once)

All aggregated errors are asserted finite before they are compared (an `f64::max` over NaN would hide a failure).

| id | clause | check | limit | estimate |
|---|---|---|---|---|
| B1 | c2 | digests (FNV-1a of `Debug`) of `whirl` for three models without a massless damped unknown (a two-bearing rotor with discs, speeds 0, 300, 900; the Jeffcott model with a massless undamped end station; a model of D3's condensation: massless shaft with a bearing `k` only), and of the `Err` Debug text of `whirl` on a massless damped model, recorded at d85922b8 BEFORE any library change in `sa_rotor_massless_default_bits.rs`; after the change `whirl` equals them and `whirl_first_order` of the first three equals `whirl` bit for bit | digests `==`; `to_bits` equal | equal by construction |
| B2 | c2 | `sa_rotor`, `sa_rotor_default_bits`, `sa_rotor_speed*`, `sa_rotor_peaks*`, `w46c_rotordyn` and `baseline_hashes` pass unchanged | pass | holds |
| A1 | c1 | A: every root of `whirl_first_order` (speeds 0, 500) against the roots of the cubic of D4 (x and y degenerate: each cubic root twice, plus `-k_p/c_p` twice) by the matching of D3 | relative `<= 1e-10`, count 8 | `<= 1e-12` |
| A2 | c1 | A: the same roots against the exact roots of the quartic of D4 with `mu_p = 1e-9 m = 2e-8` | relative `<= 1e-8` | `5e-10` (probe: truncation linear in `mu`, `4.8e-9` at 1e-8) |
| A3 | c1 | A: the same roots against the roots of `whirl` of `A_mu` (the acceptance as worded), by the matching of D3 including the check on the spurious roots | relative `<= 1e-6` | **expected miss, about `2e-6`** (probe: the existing solver's error of the degenerate partner); recorded as `a3_..._known_miss` if so |
| A4 | c1 | A: the modes (`Im > 0`) of `whirl_first_order`: count, `omega`, `growth`, `damping_ratio` against the cubic's complex root | relative `<= 1e-10`; both modes found, both with a forward and a backward sense in the degenerate pair (directions are one forward and one backward, `Whirl` of ADR 0476) | `<= 1e-12` |
| P1 | D6 | the bordered Newton polish started from `whirl` of `A_mu`'s matched roots gives the quartic's roots (the reference is trustworthy) | relative `<= 1e-10` | `<= 1e-12` |
| B1r | c1 | B: every root at each of the speeds 0, 300, 900 against the roots of `B_mu` polished by D6 (matching of D3) | relative `<= 1e-7`, count 20, the 4 unused roots of `B_mu` of modulus `>= 1e3` times the largest of `R_0` | `~1e-9` (truncation as in A2; the pedestal mass enters through the same impedance) |
| B2r | c1 | B: the same against the roots of `whirl` of `B_mu` (the acceptance as worded) | relative `<= 1e-6` | possibly missed through the reference noise of the probe; recorded as `*_known_miss` if so |
| B3 | c1 | B: the directions of the matched modes equal those of `whirl` of `B_mu`; the number of modes (`Im > 0`) equal to the number of complex pairs of `R_0` | all equal | equal |
| B4 | c1 | B: the modal assurance criterion of each mode shape (all stations, 16 entries) against the matched `B_mu` mode's | `1 - MAC <= 1e-6` | `<= 1e-8` |
| C1 | D2 | residual in the original unknowns: for every mode of A and B at each speed, `(lambda^2 M + lambda (C + Omega G) + K) x` on all free unknowns (the condensed ones recovered), relative to `(|lambda|^2 ||M|| + |lambda| ||D|| + ||K||) ||x||`, from `RotorMatrices` | `<= 1e-10` | `<= 1e-13` |
| C2 | D2 | `RotorWhirlMode::residual` of the first-order problem, every mode | `<= 1e-10` | `<= 1e-14` |
| U1 | D5 | the dense routine on the U1 numbers: count 5, the residual in the original unknowns of every pair | `<= 1e-10`, count `== 5` | `<= 1e-13` |
| N1 | negative control | A with `c_p = 0`: the first-order roots against the cubic with `c_p = 2000` | relative `>= 1e-2` (the damping changes `Re lambda` by `17.7`, `6 %` of `|lambda|`) | `6 %` |
| N2 | negative control | A with `c_p` changed by `+0.1 %` against the cubic with the nominal `c_p` | relative `>= 1e-5` | `~6e-5` |
| N3 | negative control | the pedestal mass ratio 1e-5 (quartic, `mu_p = 2e-4`) against the first-order roots of A | relative `> 1e-6` (so 1e-6 distinguishes a mass 1e-5 from none) and `A2`'s ratio 1e-9 `<= 1e-8` | `4.8e-6`, `5e-10` |
| X1 | refusals | `whirl_first_order`: speed `-1` and NaN (`"speed"`); R1 (`"damping"`); R2 (`"mass"`); `whirl` on A (`"mass"`, unchanged); `whirl_first_order` on a model with no massless damped unknown equals `whirl` (B1) | each refused with its key | refused |

Notes: A3 and B2r use the literal wording of the acceptance and are registered with its limit. The probe shows that the cause of
the expected miss is the conditioning of the existing solver on the 1e-9 model, not the formulation (A2 and B1r use references
without that conditioning). If A3 or B2r is met the miss is not claimed.

## Planned mutations (on a scratch copy with its own `CARGO_TARGET_DIR`; each must be caught)

| id | mutation | caught by |
|---|---|---|
| MU1 | the sign of `K_mm` in `A` reversed | A1, A4, C1, U1 |
| MU2 | `K_m0` omitted from `A` | A1, B1r, C1, U1 |
| MU3 | `D_00` replaced by its transpose in `B` | B1r, C1 (B's `c` of station 0 is unsymmetric), U1 |
| MU4 | `D_m0` omitted from `B` | U1 (no model of D4 has `D_m0`) |
| MU5 | the roots not multiplied by `tau` | A1 |
| MU6 | the massless unknowns placed at the wrong indices in the kept vector (the expansion swaps `q_0` entries) | C1, B4, A1 |
| MU7 | the singular-`D_00` refusal removed | X1 (R1) |
| MU8 | `whirl_first_order` always takes the first-order path, even without massless damped unknowns | B1 |
| MU9 | the nearest-root matching without the spurious-root modulus test | declared equivalent if no model of this ADR has a spurious root close to a kept one; otherwise a probe of the test with a pedestal of mass ratio 1e-1 whose spurious root is near (covered by N3's mass ratio, which fails the 1e3 modulus test and is reported) |

## Decision rule

**Done** if B1, B2, A1, A2, A4, P1, B1r, B3, B4, C1, C2, U1, N1 to N3, X1 are met, A3 and B2r are met (the acceptance as
worded), and every planned mutation is caught. **Partial** if A3 or B2r is missed (recorded as ignored `*_known_miss` tests with
the numbers; the formulation is then confirmed against the exact references of A2 and B1r) or any other limit is missed or any
mutation survives. **Todo** if the library change breaks B1 or B2 (it is then reverted, nothing merged).

## Limits (not covered)

* Only `whirl_first_order`. `campbell`, `harmonic_response`, `unbalance_response`, `unbalance_peaks` and `campbell_crossings`
  are unchanged; the response functions already solve the complex dynamic stiffness and do not need a mass, the Campbell tracker
  and the critical-speed routine do not take massless damped unknowns (no check; stated).
* Pedestals are modelled as a ground bearing at a station joined by massless shaft elements (no node-to-node spring-damper in
  `RotorModel`); a connector between stations is a different feature.
* A massless unknown whose damping is zero but whose gyroscopic or cross-coupling terms are nonzero is condensed or refused
  (singular `D_00`) as in D1; no model builds a nonzero `D_m0` or `D_0m` (D4), so those blocks are checked only by U1.
* Real roots of the massless pedestal (`-k_p/c_p` and the like) are reported in `real_roots` and `roots` and not as modes.
* Unstable pedestal dynamics (negative damping) are solved and flagged by the stability rule; no check of an unstable case.
* The 1e-9 reference by `whirl` is limited by the existing solver's conditioning (probe); not changed here.

## Open questions

1. **Case-file key.** None added. Recommendation, when a rotor case input exists: `[rotor] massless = "refuse"` (default, the
   present behaviour of `whirl`) or `"first_order"` (this method), so that a model with a massless damped station is solved only
   on request and defaults keep their bits. Alternative: use the first-order path automatically whenever such a station exists
   (changes the result of a case that is refused today: not taken, it would also change `x0_refusals`).
2. **Should `whirl` itself lift the refusal?** Recommendation: no (keeps an existing test and the refusal as a guard against
   forgotten masses); the new method is explicit.
3. **A tighter reference for the 1e-9 model** (a solver that does not form `L^-1 K L^-T` for a nearly singular `M`, for example
   the generalized form of D2 with a tiny mass in the second block): out of scope, a follow-up only if the acceptance as worded
   is to be met against the existing solver.

## Amendment 1 (2026-11-14, after the first run of the registered checks, before the corrected runs)

The first run (test `sa_rotor_massless.rs`, library as in the commit that follows) gave:

* A1, A4, C1, C2, B3, B4, B2r, N1, N2, N3 (first half), X1 and the bits of the no-massless models met.
* **Missed with their limits intact, to be kept as ignored `*_known_miss` tests** (not amended): A2 (limit `1e-8`, measured
  `1.7672e-8`), B1r (limit `1e-7`, measured `3.2130e-7`, `3.2136e-7`, `3.2175e-7` at 0, 300, 900 rad/s), N3 second half (the
  same number as A2), A3 (limit `1e-6`, measured `2.0194e-6`, as expected from the probe).
* **Two defective set-ups, corrected here, the old forms kept as ignored records:**
  1. A3 asserted the count `16` for the roots of `A_mu`; `A_mu` has six unknowns with mass (two disc translations and four
     pedestal translations), so `12` roots (a counting slip in the test, the registration text above says `2 (n_m + n_0)`).
     The error number of A3 does not depend on it. The corrected count is `12`.
  2. P1 (limit `1e-10`, the polished roots against the quartic): the reference formed the small tilt root as
     `(-c_p + sqrt(c_p^2 - 4 mu_p k_p)) / (2 mu_p)`, which cancels (`c_p^2 = 4e6`, `4 mu_p k_p = 0.16`; the root `-1000.00001`
     comes out as `-1000.0000088`). The first run measured `1.1561e-9`, all of it on the two tilt roots; the other six roots
     agreed to `1.4e-15`. The corrected reference uses `2 k_p / (-c_p - sqrt(...))` for that root; the limit stays `1e-10`;
     the corrected test is `p1b_...`; the first form is kept as the ignored `p1_..._defective_tilt_reference`.

Diagnosis of the misses (a diagnostic run, not a registered check, disclosed): B1r at `mu = 1e-9, 1e-10, 1e-11` gives
`3.2136e-7, 3.2136e-8, 3.2136e-9`: linear in `mu`, so the error is the truncation of the 1e-9 pedestal-mass model itself (on the
real root `-10109`, relative shift `mu_p |s|/c_p`-like, larger than the `5e-10` I estimated from the complex mode), not an error
of the first-order solver. The registered estimates of A2 and B1r were taken from the complex mode alone and were too small;
the limits are not changed. Both numbers are inside the acceptance's own `1e-6`; the decision rule below still counts them as
misses, so the status is partial unless the lead reads the acceptance's `1e-6` as the binding limit (stated in the addendum).
