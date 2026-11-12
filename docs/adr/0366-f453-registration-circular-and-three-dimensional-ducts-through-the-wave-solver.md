# ADR 0366: F-453 registration: a spinning and a radial mode of a hard-walled circular duct on an O-grid through the acoustic wave solver, with a matched layer

Status: **REGISTERED 2026-11-12**, written before any code of F-453 and before any run of its checks. Not yet accepted by the
maintainer. Results go in an addendum; nothing above the addendum is changed after a run.

Parent: [ADR 0351](0351-f333-registration-acoustic-wave-solver-and-modal-export.md) (F-333: `AcousticWaveSolver`, the
duct modes and the `rcfd-duct-modes` v1 export; its question 3 is this item). Roadmap item F-453 (depends on F-333).

Number 0366, branch `f453-circular-3d-ducts` from main 862d7e02, worktree `w49i-work/f453`, given by the lead.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here and not changed after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded; a defective set-up is corrected only by an
amendment committed before the corrected run, the old test staying as an ignored record; each registered run is made once;
parameters are fixed here, not searched for; defaults bit-identical; `baseline_hashes.rs`, the hash tables and the goldens
are not touched; no new third-party crate, no unsafe code; unsigned commits without trailers; no push; no CLI command (F-451
needs the maintainer).

## What F-453 asks

Acceptance (`docs/roadmap.json`, binding in full):

> A spinning mode (1, 0) and a radial mode (0, 1) of a hard-walled circular duct on an O-grid propagate with the
> closed-form axial wavenumber within 1 percent after the polygonal cut-off error of the grid, estimated in the
> registration, is accounted for; a matched layer ends the duct with less than 1 percent reflection.

Clauses: (A1) the spinning mode `(1, 0)`, (A2) the radial mode `(0, 1)`, each with its axial wavenumber within 1 percent of
the closed form corrected for the polygonal section; (P) the polygonal cut-off error estimated here, before any run; (B) a
matched layer ending the duct reflects less than 1 percent. Mode labels are those of the library (`duct_modes::Mode`): `m`
the azimuthal order, `n` the radial order counted from 0 for every `m`. So `(1, 0)` is `J_1(j'_{1,0} r/R) e^{i phi}` with
`j'_{1,0}` the first positive zero of `J_1'` (about 1.84), the first spinning mode; `(0, 1)` is `J_0(j'_{0,1} r/R)` with
`j'_{0,1}` the first positive zero of `J_0'` (about 3.83), the first radial mode. (In the traditional count the first is
`(1, 1)`; the second is `(0, 1)` in both.)

## What was read and run before this text (disclosed)

- **Read:** the F-453 and F-333 entries of `docs/roadmap.json`; ADR 0351 in full (derivations DR1 to DR9, model M1, the
  set-up, the addendum and its question 3); `crates/rcfd-fv/src/acoustic_wave.rs` in full; `crates/rcfd-post/src/duct_modes.rs`
  lines 100 to 700 (Bessel functions, `ModeBasis`, `decompose_section`, `HarmonicDft`, `split_axial`, `fit_axial`);
  `crates/rcfd-post/tests/duct_modes_f333.rs` lines 1 to 330 (the run and measurement of the channel checks);
  `crates/rcfd-mesh/src/gen.rs` (the list of generators, `hex_faces`, `cylinder_ogrid`, `annulus_3d`) and the comment of
  `Mesh::from_cell_faces` (orientation and shared faces are derived from the point loops). No generator of a full circular
  section (a disc meshed through its centre) exists: `cylinder_ogrid` and `annulus_3d` are annuli.
- **Run (timing only, an existing test, unedited):** `cargo test --release -p rcfd-post --test duct_modes_f333 s1_mode_one`
  once, to calibrate the cost: 6400 cells, 6400 steps in 19.95 s, `0.49 us` per cell and step in two dimensions; its numbers
  equal ADR 0351's. Nothing of F-453 was compiled or run.
- **Arithmetic, scratch Python (numpy), not committed** (`w49i-work/f453-scratch/m2_dispersion3d.py`, output `m2_out.txt`):
  model M2 below, and the set-up numbers of the tables (closed forms evaluated by hand-written formulas; the two Bessel zeros
  used there were computed in the scratch by bisection on the power series of `J_m'`, `1.8411837813406593` and
  `3.831705970207511`, and agree with the values I recall; the test computes them again, check BZ).
- **By hand:** the derivations DR1 to DR6.

## Decisions

**D1. Mesh: a new generator `rcfd_mesh::gen::circular_duct_ogrid`.** The standard O-grid of a pipe (the "butterfly" grid):
a Cartesian core square `[-a, a]^2`, `a = core R`, of `n x n` cells, and four ring blocks of `n` (around) `x n_r` (radial)
cells between the core's boundary and the circle; extruded along `z` in `n_z` uniform layers from `0` to `length`. The
`4 n` core-boundary points, taken counter-clockwise from the corner `(a, -a)`, are joined by straight lines to the `4 n`
points of the circle at the angles `phi_q = -pi/4 + 2 pi q/(4 n)` (the corners go to the diagonals); the ring points divide
each line uniformly in `n_r`. Every face is planar (side faces are vertical quadrilaterals, end faces lie in planes `z =
const`). The section is invariant under the rotation by 90 degrees and the mirrors in the axes and the diagonals, up to
round-off. Patches: `wall` (the circle's chords, kind `Wall`), `inlet` (`z = 0`, kind `Inlet`), `outlet` (`z = length`,
kind `Outlet`). Arguments refused: `n < 2`, `n_r < 1`, `n_z < 1`, a radius or length not finite and positive, `core`
outside `(0, 1/sqrt 2)` (the core's corner must lie inside the circle). The checks use `core = 0.5` and `n_r = n/2`, so that
the radial spacing in the ring equals the core spacing `R/n` at the middle of each side (`0.586 R/n` on the diagonals; the
tangential spacing at the wall is `pi R/(2 n) = 1.571 R/n`). It is a new public function appended to `gen.rs`; no existing
line of `rcfd-mesh` changes. A library generator rather than a test helper, because a case or the F-451 command will need
it. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

**D2. No solver change.** `AcousticWaveSolver` works on any mesh (ADR 0351); the duct is three-dimensional (no `empty`
patch), all three velocity components are active. No line of `rcfd-fv` or `rcfd-post/src` changes. The new tests are one
file `crates/rcfd-post/tests/circular_duct_f453.rs`.

**D3. Library only.** No case key, no CLI command, no MCP tool, no schema or golden change.

**D4. The two modes run separately**, each at the frequency where it travels at `cos theta = k_z/k = 0.8` (as ADR 0351),
each on its own grid sized from model M2: the spinning mode on `n = 16`, the radial mode (twice the frequency) on `n = 32`.
The full O-grid is used for both (no symmetry reduction: a symmetry plane in the solver is the `Wall` condition, whose
gradient treatment differs from the full grid's).

## Derivations

Notation as ADR 0351: `rho0 = 1.2 kg/m3`, `c = 340 m/s`, time factor `e^{+i omega t}`, `k = omega/c`. Duct radius
`R = 0.06 m` (the 120 mm fan), axis `z`.

**DR1. Circular duct modes** (ADR 0351 DR6, re-derived). Separation of `p_tt = c^2 lap p` in cylindrical coordinates,
`p = psi(r, phi) e^{i(omega t - k_z z)}`, gives `lap_2 psi + k_mn^2 psi = 0`, `k_z^2 = k^2 - k_mn^2`; with `psi = f(r)
e^{i m phi}` Bessel's equation `f'' + f'/r + (k_mn^2 - m^2/r^2) f = 0`, regular solution `J_m(k_mn r)`; the hard wall
`u_r = 0` is `dp/dr = 0` (momentum, DR1 of ADR 0351), so `J_m'(k_mn R) = 0`, `k_mn = j'_mn/R`. Textbook theory (Morse and
Ingard 1968 ch. 9; Pierce 1989 ch. 7; Rienstra and Hirschberg, *An Introduction to Acoustics*, ch. 7; cited from memory),
derived here; no tabulated data is used: every zero is computed (check BZ).

**DR2. Bessel functions in the test, independent of the library.** The power series `J_m(x) = sum_k (-1)^k (x/2)^{2k+m} /
(k! (k+m)!)` and its term-by-term derivative `J_m'(x) = sum_k (-1)^k ((2k+m)/2) (x/2)^{2k+m-1} / (k! (k+m)!)` (for `m = 0`
the `k = 0` term vanishes), summed by the term recurrence `t_{k+1} = -t_k (x/2)^2 / ((k+1)(k+1+m))` until a term falls below
`1e-17` of the running sum after `k > x`; for `x <= 10` the largest term is below `1e3`, so the round-off is about `1e-13`.
Zeros of `J_m'`: a scan in steps of 0.01 from 0.01 and bisection to an interval of `1e-15`. The library computes `J_m` from
Bessel's integral by the trapezoidal rule and its zeros on a 0.05 scan (ADR 0351 DR6): two independent implementations, so
their agreement checks both.

**DR3. The polygonal cut-off error (clause P).** The FV section is the inscribed regular polygon of `N = 4 n` sides (the wall
faces are the chords), area `A_N = (N/2) R^2 sin(2 pi/N)`. Its Neumann eigenvalues differ from the circle's. First-order
domain perturbation (Hadamard's formula for the Neumann problem, from memory, re-derived by the divergence theorem on the
perturbed Rayleigh quotient): for a boundary moved by `delta n(s)` along the outward normal, `delta lambda = int_dOmega
(|grad_t psi|^2 - lambda psi^2) delta n ds / int_Omega psi^2 dA`. For `psi = J_m(j r/R) e^{i m phi}` on `r = R`:
`|grad_t psi|^2 - lambda |psi|^2 = (m^2/R^2 - j^2/R^2) J_m(j)^2`, constant along the circle; with `int_Omega |psi|^2 dA =
pi R^2 (1 - m^2/j^2) J_m(j)^2` (ADR 0351 DR6, checked there by quadrature) this gives `delta lambda / lambda = 2 <delta n>/R`,
`<delta n>` the mean displacement along the circle: to first order the polygon has the cut-offs of the circle of radius
`R + <delta n>`. The harmonics of `delta n` (period `2 pi/N`) do not enter at first order because `|psi|^2` is constant on
the circle (spinning form) or has the period `pi/m` with `2m` not a multiple of `N` (for `m <= 1 < N/2`); they enter at
second order, `O((pi/N)^4)`. For the chord, `delta n(psi) = R cos(pi/N)/cos(psi) - R` on `|psi| <= pi/N`, mean `-R
(pi/N)^2/3 + O(N^-4)`; the equal-area radius `R_A = sqrt(A_N/pi) = R (1 - (pi/N)^2/3 + O(N^-4))` is the same to this order.
Hence the corrected closed form used by the K checks:

    k_cut,poly = j'_mn / R_A = j'_mn sqrt(pi / A_N),    k_z,poly = sqrt(k^2 - k_cut,poly^2),

with `A_N` the closed form above (check G1 confirms that the mesh section has this area). The relative error of the circle's
closed form, `k_z,cf / k_z,poly - 1 = (k_cut^2/k_z^2) (pi/N)^2/3`, to first order (`tan^2 theta = 0.5625` at `cos theta = 0.8`):

| run | N | `A_N / (pi R^2)` | `k_cut,poly / k_cut - 1` | `k_z,poly / k_z,cf - 1` |
|---|---|---|---|---|
| C10 (n = 16) | 64 | 0.99839439 | `+8.04e-4` | `-4.52e-4` |
| C01 (n = 32) | 128 | 0.99959845 | `+2.01e-4` | `-1.13e-4` |
| C10c (n = 8) | 32 | 0.99358685 | `+3.22e-3` | `-1.82e-3` |
| C01c (n = 12) | 48 | 0.99714666 | `+1.43e-3` | `-8.05e-4` |

At the registered grids the polygonal error is a tenth or less of the 1 percent budget; it is accounted for as above and the
uncorrected comparison is recorded too. Not modelled: the scheme does not see a polygon only, it sees cells; the
discretisation error of the scheme (model M2) is a separate, larger term.

**DR4. Driving a mode.** The `inlet` patch is `Driven` with the ghost state `p_g = w(t) Re(psi(x, y) e^{i omega t})`,
`u_g = (0, 0, (k_z,cf/(rho0 omega)) p_g)`, `psi` the library's normalised circular mode (`ModeBasis::eval`, area mean of
`|psi|^2` equal to 1), the ramp `w(t) = sin^2(pi t/(2 T_r))` for `t < T_r = 8 T`, then 1; amplitude 1 Pa. For `(1, 0)`,
`Re(psi e^{i omega t}) = N_10 J_1(j'_10 r/R) cos(omega t + phi)`: a pattern rotating in `-phi`, the spinning mode. The
DFT of ADR 0351 DR7 returns the complex field `psi`, whose section fit gives the amplitude of `(1, 0)`; the counter-rotating
`(-1, 0)` has a different eigenvalue (`i` against `-i`) of the grid's rotation by 90 degrees, which the discrete operator
commutes with, so it is not excited except by round-off (check SP). In general the D4 symmetry couples `m` only with
`m + 4 l`: the `(1, 0)` run contains `m = 1, -3, 5, ...`, all cut off at its frequency except `m = 1`; the `(0, 1)` run
contains `m = 0, +-4, ...`, of which `(0, 0)`, `(+-4, 0)` (`j' = 5.32 < kR = 6.39`) propagate and are excited only by the
mismatch between the circle's shapes and the grid's.

**DR5. Measurement** (ADR 0351 DR9, unchanged). Steps per period `N_T = ceil(T / max_dt(0.48))`, `dt = T/N_T` (the rule is
fixed here; the number follows from the mesh); 34 periods; the solver time is set to `(step + 1) dt` after each step; DFT
of every cell over the last 4 periods. Stations: the layers of cells (`z` index); section points the cell centres `(x, y)`,
weights the cell volumes. Each station is decomposed on `ModeBasis::below(circle R, 1.5 k)` (every circle mode with
`k_mn <= 1.5 k`). The amplitude of the driven mode at the stations whose centre lies in `[0.5, 2.5] lambda_z`
(`lambda_z = 2 pi/k_z,cf`) is fitted by `fit_axial` with the guess `k_z,poly` (a complex `k_z`, brackets `[0.9, 1.1]` and
`[-0.02, 0.05]` of the guess; a minimum at a bracket edge fails the test). Results: `kr` (clauses A1, A2), the loss per
axial wavelength `1 - exp(-2 pi ki/kr)`, `R = |a-/a+|` at `x_ref` (clause B: the layer front).

**DR6. The matched layer** (ADR 0351 DR4): `SpongeLayer { origin (0, 0, 2.5 lambda_z), normal +z, thickness lambda_z,
exponent 2, rate_max = rate_max_for_reflection(lambda_z, c, 1e-6, 2) }`, matched form. Differential reflection of a mode at
`cos theta = 0.8` from the rigid end behind it: `R0^0.8 = 1.6e-5`; ADR 0351 measured `9e-5` for 48 cells per axial
wavelength in the channel, the discrete layer.

## Model M2 (scratch, before any code)

ADR 0351's model M1 in three dimensions: the semi-discrete scheme (central gradient on a uniform grid, exact Riemann flux,
SSP-RK3) on a periodic Cartesian grid `dx = dy = dz = h`, `c dt (3/h) = 0.5`; for the driven `omega` and a transverse wave
vector of length `k_mn` at the angle `alpha` to the `x` axis, the complex `k_z` from a secant iteration on the eigenvalue
whose amplification equals `e^{i omega dt}`. Mode `(1, 0)`: `k_mn R = 1.84118`, `omega R/c = 3.06864`; mode `(0, 1)`:
`3.83171`, `6.38618`. `rel = kr/kz_cf - 1` at `alpha = 0 / pi/8 / pi/4`, and the loss per wavelength at `alpha = 0`:

| mode | h | `k h` | rel (0 / 22.5 / 45 deg) | loss |
|---|---|---|---|---|
| (1, 0) | R/8 | 0.384 | `-9.64e-3 / -9.05e-3 / -8.47e-3` | `2.75e-2` |
| (1, 0) | R/16 | 0.192 | `-2.53e-3 / -2.38e-3 / -2.23e-3` | `3.63e-3` |
| (0, 1) | R/12 | 0.532 | `-1.76e-2 / -1.65e-2 / -1.55e-2` | `6.89e-2` |
| (0, 1) | R/32 | 0.200 | `-2.74e-3 / -2.58e-3 / -2.41e-3` | `4.08e-3` |

Second order. M2 has no walls, no O-grid (non-uniform, skewed cells near the core's corners, tangential spacing up to
`1.571 h` at the wall) and no polygon; I assume, without a model, that the O-grid at most doubles the error, as the estimate
of the checks: `<= 5.1e-3` for C10 and `<= 5.5e-3` for C01 (plus the polygon term, accounted for by DR3).

## Set-up of the runs (fixed here)

`R = 0.06 m`, `core = 0.5`, `n_r = n/2`, length `3.5 lambda_z`, `n_z = round(3.5 lambda_z / (R/n))`. Patches: `inlet`
Driven (DR4), `wall` Wall, `outlet` Wall. Matched layer from `2.5 lambda_z` to the end (DR6), absent in NC10c. Ramp 8
periods, 34 periods, DFT over the last 4. Transit at the group velocity `0.8 c`: the duct and back in `7/0.64 = 10.9`
periods, so the end of the ramp has crossed twice by period 30.

| run | mode | `omega R/c` | f [Hz] | `k_z,cf` [1/m] | `lambda_z` [m] | n | `n_z` (dz) | cells | layer cells | est. `N_T` | test |
|---|---|---|---|---|---|---|---|---|---|---|---|
| C10 | (1, 0) | 3.06864 | 2767.5 | 40.915 | 0.15357 | 16 | 143 (R/15.96) | 109 824 | 41 | ~253 | heavy, `#[ignore]` |
| C01 | (0, 1) | 6.38618 | 5759.6 | 85.149 | 0.07379 | 32 | 138 (R/32.06) | 423 936 | 39 | ~244 | heavy, `#[ignore]` |
| C10c | (1, 0) | 3.06864 | 2767.5 | 40.915 | 0.15357 | 8 | 72 (R/8.04) | 13 824 | 21 | ~127 | fast |
| C01c | (0, 1) | 6.38618 | 5759.6 | 85.149 | 0.07379 | 12 | 52 (R/12.08) | 22 464 | 15 | ~92 | fast |
| NC10c | (1, 0) | as C10c, no layer | | | | 8 | 72 | 13 824 | 0 | ~127 | fast |

`rate_max`: 45 882 1/s (C10, C10c), 95 485 1/s (C01, C01c); `rate_max dt` about 0.07 (heavy) and 0.13 to 0.18 (coarse).
Cost at `~0.9 us` per cell and step (twice the measured two-dimensional cost, for the third direction): C10 about 14 min,
C01 about 53 min, each coarse run about one minute. C10 and C01 are heavy records (`#[ignore]`, run once each with
`--ignored`); C10c, C01c and NC10c are the fast tests of the same code path.

Fast mesh and solver tests (no duct run): G1, G2 on `circular_duct_ogrid(8, 4, 6, R, 0.5, 0.1)` and `(16, 8, 4, ...)`;
U8 on `circular_duct_ogrid(6, 3, 6, R, 0.5, 0.1)`, all three patches `Wall`, initial `p = (1 + x/R + 0.5 y/R)
cos(pi z/0.1)`, `u = 0`, 100 steps of `max_dt(0.48)`.

## Registered checks

All limits fixed now. Every check below decides the status (Rule I of the decision rule).

| id | clause | quantity | limit | estimate (source) |
|---|---|---|---|---|
| K10 | A1 | C10: `|kr / k_z,poly - 1|`, spinning `(1, 0)` | `<= 1e-2` | `2.2e-3` to `2.5e-3` (M2), `<= 5.1e-3` with the O-grid factor |
| K01 | A2 | C01: `|kr / k_z,poly - 1|`, radial `(0, 1)` | `<= 1e-2` | `2.4e-3` to `2.7e-3` (M2), `<= 5.5e-3` |
| S10 | B | C10: `R` at the layer front | `< 1e-2` | `R0^0.8 = 1.6e-5` plus the discrete layer, `<= 1e-3` (ADR 0351: `9e-5`) |
| S01 | B | C01: `R` at the layer front | `< 1e-2` | `<= 1e-3` |
| P | P | the polygonal correction `k_z,poly / k_z,cf - 1` of C10 and C01 computed by the test from `A_N` and the test's zeros, against the first-order `-(k_cut^2/k_z^2)(pi/N)^2/3` of DR3 | relative difference `<= 1e-2` of the correction | `O((pi/N)^2) <= 3e-3` (the next order) |
| SP10 | A1 | C10: largest `|a_(-1,0)| / |a_(1,0)|` over the stations (the spinning character) | `<= 1e-6` | round-off, `<= 1e-10` (DR4) |
| K10c | A1 (fast) | C10c: `|kr / k_z,poly - 1|` | `<= 3e-2` | `8.5e-3` to `9.6e-3` (M2), `<= 2e-2` |
| S10c | B (fast) | C10c: `R` | `< 1e-2` | `<= 3e-3` |
| SP10c | A1 (fast) | C10c: as SP10 | `<= 1e-6` | `<= 1e-10` |
| K01c | A2 (fast) | C01c: `|kr / k_z,poly - 1|` | `<= 5e-2` | `1.5e-2` to `1.8e-2` (M2), `<= 3.6e-2` |
| S01c | B (fast) | C01c: `R` | `< 1e-2` | `<= 3e-3` |
| NC10c | control | NC10c: `R` at the rigid end (`x_ref = 3.5 lambda_z`), no layer | `>= 0.9` | `~1` (ADR 0351 NC1: 1.003) |
| BZ | DR2 | the test's zeros `j'_{m,n}` for `(m, n)` in `(0,1) (0,2) (1,0) (1,1) (2,0) (3,0) (4,0)` against `duct_modes::bessel_jp_zeros`; the series residual `|J_m'(j)|`; the series `J_m` against `duct_modes::bessel_j` at `x = 0.5, 1, .., 10`, `m = 0..5` | `<= 1e-10`; `<= 1e-12`; `<= 1e-11` | round-off, `<= 1e-12` |
| G1 | mesh | cell count `(n^2 + 4 n n_r) n_z`; every cell volume `> 0`; total volume against `A_N length`; areas of `inlet`, `outlet` against `A_N`; area of `wall` against `4 n 2 R sin(pi/(4 n)) length`; face counts of the patches | exact; exact; `<= 1e-12` relative each | round-off |
| G1d | mesh | the cell centres mapped by the rotation by 90 degrees about `z` and by the mirror `x -> -x`: largest distance to the nearest cell centre | `<= 1e-12 R` | round-off, `~1e-16 R` |
| G2 | mesh | refusals: `n = 1`, `n_r = 0`, `n_z = 0`, radius `0`, `-1`, NaN; length `0`; `core` `0`, `0.71`, NaN | an error each | as stated |
| U8 | solver on the O-grid | closed O-grid duct, 100 steps: `|sum p V - sum p0 V| / sum |p0| V` | `<= 1e-12` | round-off (ADR 0351 U1: `6.5e-15`) |

Recorded, no limit (they do not decide): `kr / k_z,cf - 1` against the circle's uncorrected closed form; the loss per axial
wavelength (M2: `3.6e-3` C10, `4.1e-3` C01); the largest amplitude of the other modes relative to the driven one; the fit
residual; `N_T`, `rate_max dt`; the run times.

## Planned mutations (scratch copy, once each, not committed; the fast tests only)

| id | mutation | registered check expected to catch it |
|---|---|---|
| MU1 | `circular_duct_ogrid`: the circle's points at `0.98 R` | G1 (volume, areas) |
| MU2 | `acoustic_wave.rs`, matched form: the tangential force `-(rate/rho0) grad_t Q` removed | S10c or S01c (ADR 0351 MU2: `R = 3.9e-2` at `cos theta = 0.8`) |
| MU3 | `acoustic_wave.rs`: `Wall` faces given the `Open` flux | U8 |
| MU4 | the test's series derivative `J_m'` with `(J_{m-1} + J_{m+1})/2` in place of the term-by-term derivative | BZ |
| MU5 | the test's drive of `(1, 0)` with the standing `N J_1 cos(phi) cos(omega t)` in place of the spinning form | SP10c |
| MU6 | `circular_duct_ogrid`: the ring points joined to the circle in the clockwise order (`phi_q = +pi/4 - 2 pi q/(4 n)` on each side) | G1 (a volume `<= 0`, or the mesh refused) |

A mutation not caught by its registered check is a miss by Rule I, reported and diagnosed, whatever else catches it.

## Decision rule (registered, Rule I)

- **done** when every registered check above meets its limit at its first registered run with no post-run amendment, the
  existing tests of the touched crates (`rcfd-mesh`, `rcfd-post`) and `baseline_hashes` pass unedited, fmt and clippy are
  clean, and every registered mutation is caught by its registered check.
- **partial** when a registered check misses (kept as `*_known_miss`, diagnosed), or a registered mutation is not caught by
  its registered check, or a check is met only after a post-run amendment, or a registered run could not be made.
- **todo** does not apply once code exists.

## Limits (what is not covered)

- The two lowest non-planar modes only, each at one frequency (`cos theta = 0.8`); higher spinning orders, modes near their
  cut-off and the counter-rotating mode's own propagation are not run.
- One O-grid topology (`core = 0.5`, uniform ring); no grading towards the wall, no unstructured section.
- A rectangular three-dimensional duct (modes `(m, n)` with both indices non-zero) is not run; the circular duct is the
  three-dimensional duct of this item.
- One planar matched layer normal to the axis; medium at rest; linear; no impedance wall; no mean flow or swirl.
- The O-grid factor of the estimates is an assumption, not a model.
- Library only: no case key, CLI or MCP tool.

## Sources

- P. M. Morse and K. U. Ingard, *Theoretical Acoustics*, McGraw-Hill 1968, ch. 9; A. D. Pierce, *Acoustics*, ASA 1989,
  ch. 7; S. W. Rienstra and A. Hirschberg, *An Introduction to Acoustics* (Eindhoven lecture notes), ch. 7: circular duct
  modes and cut-off. From memory; DR1 is derived above.
- G. N. Watson, *A Treatise on the Theory of Bessel Functions*, CUP 1944, ch. 2 (the series), from memory; computed and
  checked in BZ.
- J. Hadamard (1908), the variation of an eigenvalue with its domain, in the form found in R. Courant and D. Hilbert,
  *Methods of Mathematical Physics* vol. 1, ch. 6, from memory; re-derived in DR3.
- ADR 0351 for the solver, the layer and the measurement.

No vendor code or documentation was used.

## Amendment 1 (2026-11-12, written after the registered fast runs and before the runs it registers)

The registered fast runs (commit 4005c4dc, once each, release) met every limit except **SP10c**: the counter-rotating
amplitude `|a_(-1,0)| / |a_(1,0)|` of C10c is `1.233e-5` against the limit `1e-6` (estimate "round-off"). The miss stands:
the test becomes `c10c_spinning_mode_coarse_known_miss` (ignored, limit intact, the number recorded), and a fast test
`c10c_spinning_mode_coarse_k_s` keeps the K10c and S10c assertions of the same run (both met: `-9.74e-3`, `1.53e-4`) so
that the code path stays covered by a fast test. No limit changes. The heavy runs C10 and C01 were started from the same
commit before this text and are not affected by it.

**Hypothesis H1.** The estimate assumed that the problem is invariant under the rotation by 90 degrees combined with a
quarter-period shift in time. The grid and the steady drive are, but the ramp `w(t)` is not: the real drive is
`(w psi e^{i omega t} + w conj(psi) e^{-i omega t})/2`, and the second term, which has the shape of `(-1, 0)`, has content at
`+omega` through the ramp's spectrum at `2 omega` (the ramp's second derivative jumps at `0` and `T_r`; `|W(2 omega)| /
T_r ~ 2 (pi/(2 T_r))^2 / (2 omega)^3 / T_r = 4.9e-6` for `T_r = 8 T`). That transient travels and rings in the duct; in
the DFT window it leaves a residue of this size. NC10c's `7.97e-4` (the same set-up without the layer, where the transient
does not leave) is consistent with H1. H1 says the counter-rotating content is the start transient, not a symmetry breaking.

**Diagnostic runs (registered here, each once, they do not decide the status):**

| id | run | prediction under H1 | limit |
|---|---|---|---|
| D1 | the C10c set-up for 60 periods instead of 34 (DFT over the last 4) | the counter-rotating ratio falls | `<= 0.5 x 1.233e-5` |
| D2 | the C10c mesh and drive for 300 steps of the C10c step, once with the standing drive `w N J_1 cos(phi) cos(omega t)` and once with `w N J_1 sin(phi) cos(omega t)`; `p_B` at the cell rotated by +90 degrees against `p_A` | equal to round-off: the discrete operator commutes with the rotation | `max |p_B(R x_c) - p_A(x_c)| / max |p_A| <= 1e-9` |

If D1 holds and D2 holds, the miss is diagnosed as the start transient and SP10 measures the ramp, not the grid. If D2
fails, the grid or the solver breaks the symmetry. The decision by Rule I is **partial** in either case (SP10c missed).
