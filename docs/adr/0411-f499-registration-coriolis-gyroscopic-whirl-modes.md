# ADR 0411: F-499 registration: Coriolis coupling of a rotating part (the gyroscopic matrix, whirl modes by a symmetric linearisation, forward and backward whirl in the Campbell table, on the whole part and on a cyclic sector)

Status: **REGISTERED 2026-11-13, written before any code of F-499 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-499 was written, compiled or run: the code listed under "What was read" was read, and
the arithmetic below was done with one scratch script (`f499-scratch/arith.py`, numpy only, on 2 x 2 matrices of the point
mass and on the crossing formulas; it does not call rcfd; disclosed under "Arithmetic"). Results go in the addendum at the end;
nothing above the addendum is changed after a run.

Parents: [ADR 0403](0403-f396-registration-rotating-part-modes-and-campbell-table.md) (F-396: `K_eff = K + Omega^2 (K_sigma,1 -
M_perp)`, the loss of stiffness by inertia, the Campbell table; D6 says the Coriolis matrix is neglected) and [ADR
0407](0407-f500-registration-rotating-modes-of-a-cyclic-sector.md) (F-500: `RotatingSector`, the real cos/sin pair of index `k`,
cluster tracking by the subspace assurance). Roadmap item F-499 (dep F-396), from ADR 0403 question 3; its note (ADR 0407) asks
for the sector as well. Number 0411 given by the lead. Branch `f499-coriolis-gyroscopic`, worktree `w49i-work/f499`, scratch
`w49i-work/f499-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, the hash tables or a golden file; no new dependency, no unsafe code; clean-room.

Library edits, additive only. In `crates/rcfd-fem/src/rotating.rs`: one new field `coriolis: bool` of `RotatingOptions`
(default `false`) with its builder; one new private field `axis` of `RotatingPart` (the unit axis, stored by
`with_unit_prestress`); one line `mod whirl;` and one `pub use` of the new items; the module comment gains a sentence. No
existing function changes what it computes: `RotatingPart::{modes, campbell, operator, slope}` and everything of
`RotatingSector` never read the new field. New code in a new file `crates/rcfd-fem/src/rotating/whirl.rs` (a child module, so it
reads the private matrices of `RotatingPart`). `rcfd-linalg` is not touched (the dense solver lives in `whirl.rs`). Tests: a new
file `crates/rcfd-fem/tests/si_rotating_whirl.rs`. No existing test asserts a refusal of the Coriolis term (grep of `coriolis`,
`gyroscop`, `whirl` over `crates/rcfd-fem`: only the module comment of `rotating.rs`), so no existing test is edited.

## What F-499 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The gyroscopic matrix G = 2 Omega int rho N_a N_b [a x]
> is skew to rounding; the quadratic eigenproblem (K_eff - omega^2 M + i omega G) phi = 0 is solved by a symmetric
> linearisation; at Omega = 0 the frequencies are those of F-396; a point mass on an isotropic in-plane spring pair gives the
> closed-form whirl frequencies within 1e-8; the tracked forward and backward branches enter the Campbell table of F-396 with
> their direction.

The note: on a cyclic sector the gyroscopic matrix couples the cos and sin parts of a harmonic pair; the item should solve it on
the sector as well as on the whole part.

Clauses: (c1) `G` skew to rounding; (c2) the quadratic eigenproblem solved by a symmetric linearisation; (c3) at `Omega = 0` the
frequencies of F-396; (c4) the point mass on an isotropic in-plane spring pair: the closed-form whirl frequencies within 1e-8;
(c5) the tracked forward and backward branches in the Campbell table of F-396 with their direction; (c6, the note) the same on
the sector.

## What was read (branch base 9c30146b, read only)

- `AGENT_RULES.md`; the brief; the F-499 entry of `docs/roadmap.json`; ADR 0403 (all of the design, D1 to D6); ADR 0407 (head,
  D1, the checks, decision rule, limits).
- `crates/rcfd-fem/src/rotating.rs` (all), `assemble.rs` (lines 1 to 175: `assemble_with` scatters the full element matrix,
  every entry, in element order), `cyclic.rs` (`SectorMode`, `HarmonicModes`, `doubled_matrix`, `FullRotor::expand`,
  `rotation_matrix`), `mpc.rs` (`Transformation::{expand, reduce}`: `reduce` forms `T^T A T` entry by entry for any `A`, it does
  not assume symmetry), `model.rs` (`ElasticSupport::NodalSpring`, N/m per node), `modal.rs` (`frequency`, `ModalOptions::new`,
  tolerance 1e-10), `builders.rs` (`hex_box`, the set `all`), `rcfd-linalg/src/eigen.rs` (lines 270 to 400: `jacobi`,
  `symmetric_eigen`, `generalized_unchecked`).
- Tests: `si_rotating.rs` (lines 1 to 140), `si_rotating_cyclic.rs` (lines 1 to 200: the disc sector D, `full_of`).

## Design

Notation as ADR 0403: speed `Omega >= 0` about the unit axis `a` through `p`; `[a x]` the matrix with `[a x] v = a x v`;
displacements `u` in the frame that rotates with the part.

### D1. The gyroscopic matrix (first principles)

ADR 0403 D1 expanded the kinetic energy `T = 1/2 int rho |u_t + Omega a x (X - p + u)|^2` and kept all terms but `int rho u_t .
(Omega a x u)`. With `u = N_a u_a` this term is `Omega u_t^T C u`, `C[(a, i), (b, j)] = int rho N_a N_b [a x]_ij = m_ab [a
x]_ij`, `m_ab` the scalar consistent mass of ADR 0284 with the element mass rule (as `M_perp` uses it). `C` is skew: `[a x]` is
skew and `m_ab = m_ba`. Lagrange: `d/dt dT/du_t = M u_tt + Omega C u_t + ...`, `dT/du = Omega C^T u_t + ... = -Omega C u_t +
...`, so the equation of free vibration about the pre-stressed state is

`M u_tt + G u_t + K_eff(Omega) u = 0`, `G = 2 Omega C = 2 Omega int rho N_a N_b [a x]`,

the roadmap's matrix. For a point mass `m` at the origin with `a = e_z`: `m (r_tt + 2 Omega e_z x r_t) + ...`, the Coriolis
acceleration of the rotating frame. With `u = Re(phi e^{i omega t})`: `(K_eff - omega^2 M + i omega G) phi = 0`.

Element matrix: `C_e[(a, i), (b, j)] = 1/2 (m_ab + m_ba) [a x]_ij`, the mean of the two mass entries, so that each element matrix
is skew in floating point (the product of a symmetric number and a skew one), and the assembly (`assemble_with`, every entry the
sum of the same terms in element order) gives `C_ij = -C_ji` bit for bit. Public: `coriolis_matrix(model, axis, threads) ->
CsrMatrix` (`C`, per unit speed, on the stiffness pattern; `G = 2 Omega C`), axis refused as `spin_softening_mass` does.

A uniform field `1_v` (the same vector `v` at every node) has `(C 1_v)_a = sum_b m_ab (a x v) = (M 1_{a x v})_a`: `C 1_v = M
1_{a x v}` (checked, G2).

### D2. The symmetric linearisation and its eigensolution (Meirovitch's form; derived here)

For `K_eff` symmetric positive definite on the admissible motions (tested first, by inertia, as ADR 0403 D3; otherwise the
speed is a `LossOfStiffness`, as there), with the state `x = [u_t; u]`:

`M* x_t + G* x = 0`, `M* = [[M, 0], [0, K]]` (symmetric, positive definite), `G* = [[G, K], [-K, 0]]` (skew),

since the first row is the equation of motion and the second `K u_t - K u_t = 0`. This is the form of Meirovitch (1974, AIAA
Journal 12, "A new method of solution of the eigenvalue problem for gyroscopic systems"; cited from memory, nothing taken from it
beyond what is derived here). With `x = X e^{s t}`, `s M* X = -G* X`. With `M* = L L^T`, `L = blockdiag(L_m, L_k)` the two
Cholesky factors, and `y = L^T X`:

`s y = -A y`, `A = L^-1 G* L^-T = [[L_m^-1 G L_m^-T, B], [-B^T, 0]]`, `B = L_m^-1 K L_k^-T = L_m^-1 L_k`,

`A` real skew, of order `2 n`. So `s = i omega` with `omega` real (stability for `K_eff > 0`), and `omega y = i A y`: `H = i A`
is Hermitian. At `G = 0`, `A^T A` has the blocks `B B^T` and `B^T B = L_k^T M^-1 L_k`, similar to `M^-1 K`: `omega^2` are the
eigenvalues of the symmetric problem, each with `+omega` and `-omega` (the check Q2).

The eigensolution of `H`, in real arithmetic, from first principles:

1. **Householder reduction of the skew `A` to tridiagonal form**, `A = Q T Q^T`. A reflection `P = I - beta v v^T` keeps skew
   symmetry, and with `p = A v` (so `v^T A = -p^T`, `v^T A v = 0`) `P A P = A + beta (v p^T - p v^T)`: a rank-two update, `O(n^2)`
   per step. `T` is skew tridiagonal with zero diagonal, `T_{j+1, j} = e_j`. `Q` is accumulated.
2. **The diagonal similarity** `D = diag(i^j)`: `(D^-1 (i T) D)_{j+1, j} = i^{-(j+1)} i e_j i^j = e_j` and `(.)_{j, j+1} = i^{-j}
   i (-e_j) i^{j+1} = e_j`. So `S = D^-1 i T D` is **real symmetric tridiagonal with zero diagonal** and `H (Q D w) = omega (Q D
   w)` for `S w = omega w`.
3. **Implicit QL with shifts** on `S` (the tridiagonal QL algorithm of Bowdler, Martin, Reinsch and Wilkinson 1968, `tql2` of the
   Handbook; from memory, written from the algorithm: Givens rotations chasing the bulge from the bottom of each unreduced block,
   the shift from the leading 2 x 2 block), with the rotations applied to an identity to give `w`. 30 iterations per eigenvalue at
   most, else `FemError::Solver`.
4. The spectrum of `S` is symmetric about zero (`(omega, phi)` and `(-omega, conj phi)`); the `n` positive values, ascending, are
   the frequencies (rad/s). The mode of `omega_j > 0`: `y = Q D w`, its real part from the even columns of `Q` (signs `+, -`
   alternating with `j/2`) and its imaginary part from the odd ones; `phi = L_k^-T y_bottom`. A zero or negative "positive" count
   cannot occur for `K_eff > 0`; if fewer than the requested number are `> 0` the result is refused (`"modes"`).
5. Normalisation: `phi^H M phi = 1`, the phase so that the entry of largest modulus (the first on a tie) is real and positive.
6. The residual of each returned pair, `||(K_eff - omega^2 M + i omega G) phi|| / (||K_eff phi|| + omega^2 ||M phi|| + omega ||G
   phi||)`, is stored with the mode.

Dense, `O(n^3)`: a reduced problem of more than `MAX_WHIRL_UNKNOWNS = 1500` unknowns is refused (key `"unknowns"`), so that the
dense state of order 3000 stays near 0.2 GB. A sparse shift-invert Lanczos on `H` is the follow-up for large parts (limits).

**Zero speed (c3).** At `Omega = 0` the whirl analysis returns the modes of `RotatingPart::modes(0.0)` (that is
`modal::modal_analysis`, ADR 0403 D1) as real modes (`im = 0`, whirl 0, direction `Neutral`); the frequencies are its bits by
construction. Continuity at small speed is checked (Z2).

### D3. Forward and backward whirl

For `u = Re(phi e^{i omega t})` the time mean of the mass-weighted moment of the motion about the axis, `<u^T C u_t>`, is `1/2
omega Im(phi^H C phi)`, positive when the orbit turns in the sense of `Omega` about `a`. The **whirl number** is

`kappa = Im(phi^H C phi) / (phi^H M phi)`, in `[-1, 1]`

(`|phi^H C phi| <= phi^H M phi` since `|[a x] v| <= |v|`). For the point mass `phi = (1, -i, 0)/sqrt 2` (the orbit `(cos, sin)`,
counter-clockwise about `e_z`): `C phi = m (i, 1, 0)/sqrt 2`, `phi^H C phi = i m`, `kappa = 1`. First-order perturbation of
`(K_eff - omega^2 M + i omega G) phi = 0` in `G`: `-2 omega delta_omega phi^H M phi + i omega phi^H G phi = 0`, so `delta_omega =
i Omega phi^H C phi / phi^H M phi = -Omega kappa`: **a forward-whirling mode (`kappa > 0`) falls in frequency in the rotating
frame and a backward one (`kappa < 0`) rises**, as `omega_0 -/+ Omega` for the point mass. Classification: `Whirl::Forward` for
`kappa > WHIRL_TOLERANCE`, `Whirl::Backward` for `kappa < -WHIRL_TOLERANCE`, `Whirl::Neutral` otherwise, `WHIRL_TOLERANCE =
1e-6` (a mode with no in-plane orbit, such as purely axial motion, has `kappa = 0` to rounding).

### D4. The Campbell table with directions

`RotatingPart::whirl_campbell(speeds)` (speeds as ADR 0403 D4, refused otherwise with `"speeds"`) solves each speed (D2) with
`options.modal.modes` modes and tracks them as ADR 0407 amendment 1 does, by the subspace assurance of eigenvalue clusters, with
one change: each complex mode `phi = r + i s` is the real group `{(r, s), (-s, r)}` (its complex line written in real
coordinates of twice the length, the mass block-diagonal), and a real mode of the speed 0 the group `{(r, 0), (0, r)}`. For two
complex lines this subspace assurance is `|phi^H M psi|^2 / ((phi^H M phi)(psi^H M psi))`, the complex MAC (the projection of
`(c, d)` on the orthogonal, equal-norm pair `(r, s)`, `(-s, r)` is `(Re phi^H M psi)^2 + (Im phi^H M psi)^2`). A real MAC of the
real parts would not distinguish forward from backward whirl of one shape (`phi` and `conj phi` have the same real part); the
complex one gives them 0 for circular orbits. At a degenerate pair of the speed 0 (the cluster) both new modes score the same, and
the tie rule (the lower track, then the lower mode) assigns the lower frequency to the lower track.

The result `WhirlCampbell { table: CampbellTable, whirl: Vec<Vec<Option<f64>>>, directions: Vec<Vec<Option<Whirl>>> }`: the
table of F-396 (eigenvalues `omega^2`, frequencies Hz, the MAC, the modes as the real vectors `(r, s)` of length `2 n`) plus,
per track and speed, `kappa` and the direction. **Crossings**: whirl branches have `d omega/d Omega = -kappa != 0` at rest, so
`omega` is linear in `Omega` to first order and not in `Omega^2` (ADR 0403's rule). `whirl_order_crossings(speeds, omegas,
order)` interpolates `g = omega - order Omega` linearly in `Omega`, exact for `omega_0 -/+ Omega`; `WhirlCampbell::crossings`
uses it. `to_csv` (the columns of ADR 0403 plus `track_t_dir` per track, `forward`, `backward`, `neutral` or empty) and
`to_json` (each track with `"whirl"` and `"direction"` arrays) in shortest round-trip form.

### D5. The sector (c6)

ADR 0407 D1 showed that a full-rotor matrix `A` commuting with the sector shift enters index `k` as `T_k^T blockdiag(A_s, A_s)
T_k`. That argument used only the commutation, not symmetry, and `C` commutes: its node block `m_ab [a x]` satisfies `R [a x]
R^T = [R a x] = [a x]` for a rotation `R` about `a`, and `m_ab` is invariant. Directly: with the sector-local field `c_j a - s_j
b` on copy `j` (`c_j = cos j theta`, `s_j = sin j theta`), `sum_j (c_j a - s_j b)^T A_s (c_j a' - s_j b') = N/2 (a^T A_s a' +
b^T A_s b')` for any `A_s`, since `sum c_j^2 = sum s_j^2 = N/2` and `sum c_j s_j = 0` for `0 < 2k < N`. So the reduced gyroscopic
matrix of index `k` is `G_k = T_k^T blockdiag(G_s, G_s) T_k` (single block for `k = 0` and `2k = N`), skew. **Where the coupling
of cos and sin enters (the note, stated precisely):** not in a block of `G_k` but through `T_k`, whose rows `a_H = R (cos theta
a_L - sin theta b_L)` mix the two parts at the boundary; without `G` the reduced pair problem commutes with `J: (a, b) -> (-b,
a)` (a phase shift of the harmonic, which maps the rows onto themselves) and every eigenvalue is double; `G_k` is real and also
commutes with `J`, but `i omega G_k` is not real, so the eigenvectors are the eigenvectors of `J` in the complexified space, `b =
-/+ i a`: travelling waves, one forward and one backward in the sector sense, with different frequencies. The ADR 0407 D1 remark
("`[[0, -G],[G, 0]]`-like after realification") was looser than this and is superseded here.

So `RotatingSector::whirl_modes(k, Omega)` reduces `K_eff`, `M` and `G` of the sector (doubled for a pair) by `T_k` and solves D2;
`options.modal.modes x multiplicity` modes are returned (a split pair counts twice). The mode's vectors are `(cos, sin)` of the
real and of the imaginary part, expanded by `T_k`. Besides `kappa` (computed on the reduced matrices, which equals the full
rotor's by the identity above), the **wave sense** of a pair mode `tau = 2 Im(a^H M b) / (a^H M a + b^H M b)`: `+1` for a wave
travelling in the sense of `Omega` (`b = i a`: the full field `Re((cos k phi a - sin k phi b) e^{i omega t})` is then `a cos(k
phi - omega t)`), `-1` for the other, `0` for a standing wave; `None` for single indices. At `Omega = 0` the modes of
`modal_analysis_harmonic` (bits). `RotatingSector::whirl_campbell(harmonics, speeds)` gives one `WhirlCampbell` per index
(`CyclicWhirlCampbell`), the groups of a pair mode in real coordinates of length `4 n`.

### D6. Interfaces (all new, in `rotating/whirl.rs`, re-exported from `rotating`)

`coriolis_matrix`; `Whirl`, `WHIRL_TOLERANCE`, `MAX_WHIRL_UNKNOWNS`; `WhirlMode { omega, eigenvalue, frequency, re, im, whirl,
direction, wave, residual }`; `WhirlModes { modes, frequencies_all, unknowns }` (`frequencies_all` every positive `omega` of the
reduced problem, ascending, rad/s); `WhirlSpeedModes::{Modes, LossOfStiffness}`; `HarmonicWhirlSpeedModes::{Modes,
LossOfStiffness(HarmonicLoss)}`; `RotatingPart::{coriolis, whirl_modes, whirl_campbell}`; `RotatingSector::{whirl_modes,
whirl_campbell}`; `WhirlCampbell`, `CyclicWhirlCampbell`; `whirl_order_crossings`; `gyroscopic_eigen_dense(k, m, g, n)` (the
solver of D2 on dense row-major matrices, all positive frequencies and modes; for the algebraic point mass and for users with
small matrices). `RotatingOptions::coriolis(bool)`: with `false` (the default) `whirl_modes` solves D2 with `G = 0` (the dense
solution of the F-396 problem, the control Q2); with `true` the physical `G`.

## Set-up (fixed now)

Material steel `E = 2e11 Pa`, `nu = 0.3`, `rho = 7850 kg/m^3`; axis `e_z` through the origin.

**PM (algebraic point mass):** `m = 7.85 kg`, `omega_0 = 1000 rad/s`, `k = m omega_0^2`, `M = m I_2`, `K_eff = (k - m Omega^2)
I_2`, `G = 2 Omega m [[0, -1], [1, 0]]`, through `gyroscopic_eigen_dense`. Closed form (`z = x + i y`: `z_tt + 2 i Omega z_t +
(omega_0^2 - Omega^2) z = 0`, `z = e^{i s t}`: `s^2 + 2 Omega s - (omega_0^2 - Omega^2) = 0`, `s = -Omega +/- omega_0`): the
rotating-frame frequencies `omega_0 - Omega` (forward, `kappa = +1`) and `omega_0 + Omega` (backward, `kappa = -1`) for `Omega <
omega_0`.

**CB (the point mass as a model):** one Hex8 cube of side `h = 0.1 m` centred on the origin (`hex_box([-0.05; 3], [0.1; 3], [1,
1, 1])`), `m = rho h^3 = 7.85 kg`; nodal springs `k_n = m omega_0^2 / 8 = 981250 N/m` at all 8 nodes in `x` and in `y`; `u_z = 0`
at all nodes; both effects of ADR 0403 on (stress stiffening, spin softening). The rigid translations `1_x`, `1_y` span an exact
invariant subspace: `K 1_v = 0` (no strain), the springs give `8 k_n`, `K_sigma 1_v = 0` (`sum_b grad N_b = 0`), `M_perp 1_v = M
1_v` (`v` in the plane), `C 1_v = M 1_{e_z x v}` (D1), and the consistent row sums of the cube are equal at all nodes (symmetry),
so `M 1_v = (m/8) 1_v`. Hence the closed form of PM, `omega_0 -/+ Omega` with `omega_0^2 = 8 k_n / m`, holds exactly for the
translation pair among the cube's modes; the other modes (torsion on the springs, `omega_t^2 = 8 k_n (h^2/2) / (m h^2/6) = 3
omega_0^2` at rest; internal modes `~1e5 rad/s`) are separate. `modes = 3`. Speeds `Omega_j = 0.07 j omega_0`, `j = 0 .. 12` for
the Campbell table; single speeds `0.1, 0.3, 0.5, 0.7, 0.9 omega_0`; `1.2 omega_0` for the loss.

**DS (the disc sector):** `N = 6`; `hex_box([0.25, 0, -0.1], [0.75, alpha, 0.2], [2, 1, 1])` mapped to the annulus `(r cos t, r
sin t, z)` and upgraded to Hex20 (`upgrade_to_hex20_mapped`, as ADR 0407's `d_mesh`), `xmin` (the inner radius) clamped, the
cyclic constraint `ymin`/`ymax`, `N`; both effects on; `modes = 4`, modal tolerance 1e-9, 400 iterations (ADR 0407's options).
The full rotor by `full_rotor` with `xmin` clamped, as ADR 0407's `full_of`. `omega_ref` = the lowest rest sector frequency over
the indices `0 .. 3`. Comparison speeds `0.4 omega_ref` and `0.8 omega_ref` (below `omega_ref`, where `K_eff >= K - Omega^2 M`
stays positive definite if `K_sigma,1 >= 0`; a loss there would be a defective set-up). Campbell speeds `0, 0.2, 0.4, 0.6, 0.8
omega_ref`, indices `0 .. 3`.

## Arithmetic (done before the code; disclosed)

1. PM through numpy (`np.linalg.eigh` of `i A` of D2, the same construction): at `Omega/omega_0 = 0.1 .. 0.9` the frequencies
   `omega_0 -/+ Omega` to `<= 4e-16 omega_0`, `kappa = +1` for the lower and `-1` for the upper to `2e-16`, residuals `<= 3.3e-14`.
   So the sign convention of D3 is the right one (forward = lower in the rotating frame).
2. Crossings of PM on the grid `0.07 j omega_0`: by D4's rule (linear in `Omega`) exact (`0.5, 1/3, 0.25, 0.2` for the forward
   branch with orders 1 to 4, `0.5, 1/3` for the backward with orders 3, 4); by ADR 0403's rule (linear in `Omega^2`) wrong by
   `-1.1e-3 .. +3.2e-3` relative (the control N3).
3. CB rounding: the translation eigenvalue is perturbed by the rounding of the element stiffness on a rigid motion, `~eps
   (omega_max/omega_0)^2 ~ 2e-16 x 1e4 x 10 ~ 2e-11` relative, so `omega` within `~1e-11 omega_0`; the dense solver adds `eps
   ||A|| / omega_0 ~ 2e-16 x 1e5/1e3 ~ 2e-14`. Estimate `<= 1e-10` against the limit 1e-8.
4. DS: sector `~32` nodes, `~19` per sector in the full rotor, `~110` full nodes, `~290` free unknowns: the full dense state of
   order `~580`, seconds. The split of a bending pair by Coriolis: `kappa ~ (t/R)^2 ~ 0.02` for the in-plane motion of a thick
   disc's bending mode, split `2 kappa Omega/omega ~ 1e-2` at `0.8 omega_ref` (order of magnitude only; S4 asks for `> 1e-6`).

## Checks (limits fixed now)

| # | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| G1 | c1 | `max |C_ij + C_ji| / max |C_ij|` of `coriolis_matrix` on CB, DS (sector) and the full rotor | `<= 1e-14` | 0 (D1: skew elements, same-order assembly) |
| G2 | c1 | CB: `||C 1_x - M 1_y|| / ||M 1_y||` and `||C 1_y + M 1_x|| / ||M 1_x||` | `<= 1e-14` | `~1e-16` (D1) |
| Q1 | c2 | the stored residual of every mode returned at `Omega > 0` with Coriolis (PM, CB at the five speeds and the 12 Campbell speeds, DS every index at both comparison speeds, the full rotor) | `<= 1e-9` | `<= 1e-12` (arithmetic 1, 3) |
| Q2 | c2 | Coriolis off (`coriolis(false)`): `whirl_modes` at `0.5 omega_0` (CB) and at `0.4 omega_ref` (DS index 1) against `modes` (the subspace solution of F-396): `max |omega_dense / sqrt(lambda) - 1|` over the returned modes | `<= 1e-9` | `~1e-12` (subspace residual 1e-10, eigenvalue error quadratic) |
| Q3 | c2 | with `K_eff > 0` the dense problem has exactly `n` positive frequencies (`frequencies_all.len() == unknowns`), all finite (CB, DS, full rotor) | exact | D2 |
| Z1 | c3 | `whirl_modes(0)` of CB: frequencies and eigenvalues by `to_bits` equal to `modes(0)` and to `modal_analysis`; DS: `whirl_modes(k, 0)` against `modal_analysis_harmonic` (each real eigenvalue's frequency), `k = 0 .. 3` | identical | by construction (D2) |
| Z2 | c3 | CB at `1e-6 omega_0` with Coriolis: `max |omega/omega_rest - 1|` over the 3 modes | `<= 2e-6` | `|kappa| Omega/omega_0 <= 1e-6` (D3) |
| P1 | c4 | PM at `0.1 .. 0.9 omega_0`: `|omega_f - (omega_0 - Omega)|/omega_0`, `|omega_b - (omega_0 + Omega)|/omega_0` | `<= 1e-8` | `4e-16` (arithmetic 1) |
| P1b | c4 | CB at the same speeds: the two returned modes of largest translation share (complex MAC with `span{1_x, 1_y}`, each `>= 1 - 1e-8`) against `omega_0 -/+ Omega` | `<= 1e-8` relative to `omega_0` | `<= 1e-10` (arithmetic 3) |
| P2 | c5 | PM and CB at those speeds: the mode at `omega_0 - Omega` has `kappa >= 1 - 1e-8` and `Whirl::Forward`, the one at `omega_0 + Omega` `kappa <= -1 + 1e-8` and `Whirl::Backward` | each | `kappa = +/-1` exactly (D3) |
| P3 | c5 | CB `whirl_campbell` over the 13 speeds: tracks 0 and 1 (the translation pair at rest) follow `omega_0 - Omega` and `omega_0 + Omega` within `1e-8 omega_0` at every speed `> 0`, with `Forward` resp. `Backward` at every speed `> 0` and `None` at 0; their stored MAC `>= 0.9` at every step (the backward branch crosses the torsion track near `0.7 omega_0`) | each | MAC `~1` (exact invariant subspace) |
| P4 | c5 | CB `WhirlCampbell::crossings(E)`, `E = 1 .. 4`, on tracks 0 and 1: exactly the six crossings of arithmetic 2, each `|Omega_c/Omega_exact - 1|` | count 6; `<= 1e-9` | `~1e-12` |
| P5 | c5 | `to_csv`: `1 + 13` rows, a direction column per track (`forward`/`backward` on tracks 0 and 1 at `Omega > 0`, empty at 0), every frequency parses back to the table's bits; `to_json(&[1, 2, 3, 4])`: no `NaN`/`inf`, the `"direction"` arrays present | each | shortest round-trip |
| S1 | c6 | DS at each comparison speed with Coriolis: the union over `k = 0 .. 3` of `frequencies_all` of the sector, sorted, against the full rotor's (`RotatingPart::new` on the full mesh, `coriolis(true)`): count equal and `max |omega_s/omega_f - 1|` | count equal; `<= 1e-8` | `~1e-12` (the pre-stresses agree to 9e-14, ADR 0407) |
| S2 | c6 | each returned sector mode (`k = 0 .. 3`, both speeds) expanded to the full rotor (real and imaginary parts by `FullRotor::expand`): the relative residual of the full quadratic problem (full `K_eff`, `M`, `G`) at the free equations | `<= 1e-8` | `~1e-12` |
| S3 | c6 | pair indices `k = 1, 2` at both speeds: every returned mode whose relative gap to its neighbours in the index's list exceeds 1e-6: `| |tau| - 1 |` | `<= 1e-6` | `~1e-10` (the `J` symmetry, D5) |
| S4 | c6 | pair index 1 at `0.8 omega_ref`: the largest relative split `|omega_{2i+1} - omega_{2i}| / mean` over its returned pairs | `> 1e-6` | `~1e-2` (arithmetic 4) |
| S5 | c6 | DS `whirl_campbell(&[0, 1, 2, 3], speeds)`: 4 families with multiplicities `1, 2, 2, 1`, tracks `4, 8, 8, 4`, every stored frequency finite, a direction at every speed `> 0`; the minimum stored MAC reported (recorded, no limit: veering inside a family is not excluded) | each | D5 |
| L1 | D2 | CB at `1.2 omega_0` with Coriolis: `LossOfStiffness` with `negative == Some(2)` (the translation pair has `8 k_n - m Omega^2 < 0`, the torsion `3 omega_0^2 m - m Omega^2 > 0` at this order) | exact | D2 |
| R1 | D6 | refusals: `whirl_modes(-1.0)` and `whirl_modes(NaN)` (`"speed"`); `whirl_campbell` with a decreasing list (`"speeds"`); `RotatingSector::whirl_modes(4, .)` on DS (`"harmonic"`); `gyroscopic_eigen_dense` with a matrix of the wrong length (`"matrix"`) and with a `K` that is not positive definite (`"stiffness"`) | each refused with its key | D6 |
| R2 | rule 3 | `RotatingOptions::new(m).coriolis == false`; on CB, `modes(0.5 omega_0)` and `campbell` of a part with `coriolis(true)` equal by `to_bits` those of the default part; the existing tests of `rcfd-fem` pass unchanged (release); `cargo check -p rcfd-post --tests` | each | no existing function reads the new field |

Negative controls (expected to fail the property, each with a limit):

| # | Control | Expected |
|---|---|---|
| N1 | P1b with `coriolis(false)` at `0.5 omega_0`: the translation pair at `sqrt(omega_0^2 - Omega^2) = 0.866 omega_0` (double) | deviation from `omega_0 -/+ Omega` `> 1e-2` |
| N2 | S1 at `0.8 omega_ref` with the sector `coriolis(false)` against the full rotor with it | deviation `> 1e-6` |
| N3 | P4 by ADR 0403's rule (`table.crossings(E)`, linear in `Omega^2`) on the same table | deviation `> 1e-4` (arithmetic 2: `3.7e-4 .. 3.2e-3`) |

Planned mutations (on a `git archive` copy in `f499-scratch`, each run once against `si_rotating_whirl`; what catches each is
recorded):

1. `G = Omega C` (the factor 2 dropped): P1, P1b, P3.
2. The sign of `G` reversed: P2 (the directions swap with the frequencies).
3. `kappa` from the real part `Re(phi^H C phi)` (zero): P2.
4. The sector's `G` doubled as `blockdiag(G_s, 0)`: S1, S2.
5. Crossings interpolated in `Omega^2`: P4.
6. The zero-speed shortcut removed (the dense solve at `Omega = 0`): Z1.
7. The spin softening left out of the whirl operator (`K` with `K_sigma` only): P1b, P3.
8. In the tracker, the real-part MAC (`{(r, 0)}` groups) instead of the complex line: P3 (expected to survive if the frequencies
   keep the order; recorded either way).

## Decision rule

- **done**: G1 to R2 met (S5 as registered) and N1 to N3 as expected.
- **partial**: G1, Q1, Z1, P1, P1b, P2 met (clauses c1 to c4) with one other check missed and its cause diagnosed (a
  `*_known_miss` record), or a clause met only in a registered reading.
- **todo**: G1, Q1, Z1 or P1 missed, or a speed with `K_eff > 0` gives a non-finite frequency.

## Limits (what is not covered)

Speeds above the loss of stiffness of `K_eff` (a gyroscopic system can be stable there, the point mass above `omega_0` for
example; `M*` of D2 is then indefinite and the frequencies need a non-symmetric eigensolver: reported as a loss, as F-396 does);
damping; large parts (the dense solver stops at 1500 reduced unknowns; a sparse shift-invert Lanczos on `H` is the follow-up);
frequencies in the stationary frame (for a mode of index `k` `omega_stat = omega -/+ k Omega` by the wave sense, not computed
here); the pre-stress linear in `Omega^2` as F-396; one cube and one disc geometry; output through case files and the CLI.
