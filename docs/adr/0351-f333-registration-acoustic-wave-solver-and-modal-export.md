# ADR 0351: F-333 registration: a linear acoustic wave-equation solver on the finite-volume mesh, its sponge layer, and the modal decomposition and export of hard-walled duct fields

Status: **REGISTERED 2026-11-12**, written before any code of F-333 and before any run of its checks. Not yet accepted by the
maintainer. Results go in an addendum; nothing above the addendum is changed after a run.

Parents: [ADR 0233](0233-f243-registration-impedance-and-quiet-boundary.md) (F-243: question 2, the split of the title's
unbuilt parts, of which this item is the wave-equation solver with sponge layers and the modal export),
[ADR 0237](0237-f331-registration-sponge-layer-of-the-compressible-solver.md) (F-331: the plain and the matched sponge forms
DR1 to DR5, whose linear forms are used here), [ADR 0066](../acoustics.md) (wave 32B, FW-H and the acoustics metrics).
Roadmap item F-333 (depends on F-243, F-331).

Number 0351, branch `f333-acoustic-wave-solver` from main 6430decf, worktree `w49i-work/f333`, given by the lead.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here and not changed after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded; a defective set-up is corrected only by an
amendment committed before the corrected run, the old test staying as an ignored record; each registered run is made once;
parameters are fixed here, not searched for; defaults bit-identical; `baseline_hashes.rs`, the hash tables and the goldens
are not touched; no new third-party crate, no unsafe code; unsigned commits without trailers; no push.

## What F-333 asks

Acceptance (`docs/roadmap.json`, binding in full):

> A duct mode propagates with the closed-form axial wavenumber within 1 percent; a sponge layer ends the duct with less
> than 1 percent reflection; the modal amplitudes of a synthetic field are exported and recovered within 1 percent.

Three clauses: (A) the axial wavenumber of a propagating duct mode, (B) the reflection of a sponge-terminated duct, (C) the
export and recovery of modal amplitudes. The note says the item is "the rest of the F-243 title" (wave-equation solver with
sponge layers, modal export). ADR 0233's question 2 only proposed the split; it did not say what the solver is. That is
decided here (D1).

## What was read and run before this text (disclosed)

- **Read:** the F-333, F-243 and F-331 entries of `docs/roadmap.json`; ADR 0233 (the decision rule, limits, open questions
  and the addendum); ADR 0237 lines 1 to 120 (the derivations DR1 to DR7 of the plain and matched forms); `docs/acoustics.md`
  sections on the sponge (F-331, F-340, F-341, F-407); `crates/rcfd-fv/src/sponge.rs` lines 1 to 270 (the types `Sponge`,
  `SpongeLayer`, `rate_max_for_reflection`); `crates/rcfd-fv/src/geometry.rs` (the fields of `FvGeometry`);
  `crates/rcfd-fv/src/gradient.rs` (`least_squares`, `least_squares_increments`, `solve_masked`); the head of
  `crates/rcfd-fv/tests/sponge_f331.rs`; `crates/rcfd-mesh/src/gen.rs` (`box_mesh`, `box_2d`, `warp`); the module list of
  `crates/rcfd-post/src/lib.rs` and the function list of `acoustics.rs` (there is an FFT; there is no complex-number type in
  the workspace). `crates/rcfd-fv/src/waves.rs` is the free-surface wave module (F-227) and is unrelated.
- **Searched:** `crates/*/src` for "acoustic", "fwh", "sponge", "duct mode", "modal": no wave-equation solver, no duct-mode
  or modal code exists.
- **Arithmetic, one scratch Python script (numpy), no rcfd code, not committed** (`w49i-work/f333-scratch/m1_dispersion.py`):
  model M1, a Fourier analysis of the planned semi-discrete scheme on a uniform Cartesian grid (described under "Model M1"
  below with every equation it solves). Its numbers fixed the grids and are the estimates of the K and E checks.
- **By hand:** the derivations DR1 to DR9 and the set-up numbers.

No library or test code of F-333 existed when this text was written, nothing of F-333 was compiled, and no check below was
run.

## Decisions

**D1. What the wave-equation solver is.** A solver of the **linear acoustic equations about a uniform medium at rest**, in
first-order form for the pressure perturbation `p` and the velocity `u`, on the existing polyhedral finite-volume mesh
(`FvGeometry`), in a new module `rcfd-fv/src/acoustic_wave.rs` (`AcousticWaveSolver`). It lives in `rcfd-fv` because it is
a solver on the FV geometry and reuses `sponge::SpongeLayer`; the modal analysis lives in `rcfd-post` (`duct_modes.rs`),
which already depends on `rcfd-fv`. Relation to the compressible solver: the equations are the small-amplitude limit of the
Euler equations of `CompressibleSolver` about a state at rest (DR1); the wave solver has two unknowns instead of five, an
exact linear Riemann flux instead of HLLC, and no nonlinearity, so it propagates sound over long ducts at a fraction of the
cost. It is not coupled to the compressible or the incompressible solvers in this item (no source from a CFD run, no
hybrid coupling); a source enters through a driven boundary. No mean flow (the convected wave equation is a limit).
Recommendation of mine, taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

**D2. Opt-in, new code only.** Two new files (`rcfd-fv/src/acoustic_wave.rs`, `rcfd-post/src/duct_modes.rs`), one `pub mod`
line in each crate's `lib.rs`. No existing function changes; nothing that does not construct the new types runs new code.
`cargo test -p rcfd-api --release --test baseline_hashes` is run because `lib.rs` of two crates is touched.

**D3. Library only.** No case key, no CLI command, no MCP tool, no schema or golden change. A case section `[acoustics.wave]`
and a CLI `rcfd duct-modes` are proposed follow-ups (they would need the maintainer's approval of new keys and help text).
Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); the brief says the same.

**D4. Duct shapes.** Rectangular (including the two-dimensional channel, modes `(m, 0)`) and circular, both hard-walled.
The simulation checks (clauses A and B) are run in a two-dimensional channel (a rectangular duct's `(m, 0)` modes, one cell
in the third direction with `empty` faces); the circular modes are used in the export check (clause C) only, because a
circular duct on a mesh of straight-sided cells has a cut-off different from the circle's at the 1 percent level unless very
fine (limit, below).

**D5. Modal export format.** JSON, `{"format": "rcfd-duct-modes", "version": 1, ...}` (fields in DR8), written and read by
`rcfd-post::duct_modes`; complex numbers as `[re, im]`. Not a case file, not part of any schema golden.

## Derivations

Notation: density `rho0`, sound speed `c`, `Z = rho0 c`, `K = rho0 c^2`; time factor `e^{+i omega t}` in this ADR (the
library's DFT returns the amplitude `A` of `p = Re(A e^{i omega t})`); `k = omega/c`.

**DR1. Linear acoustics.** Linearising the Euler equations about `(rho0, u = 0, p0)` and using the isentropic relation
`p' = c^2 rho'`: `p_t + K div u = 0`, `rho0 u_t + grad p = 0`. Eliminating `u`: `p_tt = c^2 lap p`. The acoustic energy
`E = p^2/(2K) + rho0 |u|^2/2` obeys `E_t + div(p u) = 0`. In conservation form for a cell of volume `V` with faces of area
`A_f` and unit normal `n_f`: `V dp/dt = -K sum_f u_f.n_f A_f`, `V du/dt = -(1/rho0) sum_f p_f n_f A_f`.

**DR2. The face flux.** Along a face normal `n` the one-dimensional system `p_t + K u_n,x = 0`, `rho0 u_n,t + p_x = 0` has the
characteristic variables `w+- = p +- Z u_n`, travelling with `+-c`; the tangential velocity does not enter. The exact
solution of the Riemann problem between the left state `L` (owner side) and the right state `R`: `w+` from `L`, `w-` from
`R`, hence

    p* = (pL + pR)/2 + Z (unL - unR)/2,      un* = (unL + unR)/2 + (pL - pR)/(2Z).

(The upwind flux of a linear hyperbolic system, Godunov; for example LeVeque, *Finite Volume Methods for Hyperbolic
Problems*, 2002, ch. 3, from memory; the formula is the two characteristic relations above solved for `p*`, `un*`.)
Second order in space: `L` and `R` are the cell values extrapolated to the face centre with an unlimited least-squares cell
gradient (the inverse-distance-squared weighted fit of `gradient::least_squares`, here with the 3x3 matrices factored once,
because the mesh does not change). Unlimited, because the fields are smooth and linear: a limiter would only add error.
Time: SSP-RK3 (Shu and Osher 1988, from memory) with a fixed step chosen by the caller; `max_dt(cfl)` returns
`cfl * min_c 2 V_c / (c sum_f A_f)` over the non-empty faces (on a uniform square grid in two dimensions `cfl dx/(2c)`, the
usual multi-dimensional sum `c dt (1/dx + 1/dy) = cfl`). The checks run at `cfl = 0.48`.

**DR3. Boundaries.** A boundary face takes the Riemann flux against a ghost state:
- `Wall` (hard wall, also a symmetry plane): ghost `pR = pL`, `unR = -unL`, so `un* = 0` and `p* = pL + Z unL`. No flux of
  `p` through the wall; the total `sum_c p_c V_c` is constant in a closed box (check U1).
- `Open` (characteristic, quiet in one dimension): ghost at rest, `pR = unR = 0`, so the incoming characteristic is zero.
  At normal incidence it reflects nothing; a plane wave at the angle `theta` to the normal returns with
  `R = (cos theta - 1)/(cos theta + 1)` (from `w- = 0` at the face: `(1 + R) - cos theta (1 - R) = 0`; ADR 0233 DR2 is the
  same result).
- `Driven(f)`: ghost `(p, u) = f(x_f, t)` given by the caller. If `f` is a wave travelling into the domain, the face imposes
  its incoming characteristic and passes the outgoing one (one-dimensionally), so the domain is driven by the wave while a
  returning wave leaves with the `Open` reflection of its angle.
`empty` faces are skipped. Cyclic (periodic) patches are refused (not built). For the least-squares gradient the boundary
value of `p` is the owner value and the boundary value of `u` is the owner value with the normal component removed on a
`Wall` (exact: the wall velocity has zero normal component) and the owner value elsewhere.

**DR4. Sponge forms for the wave equation.** The rates `sigma_c >= 0` per cell, from `sponge::SpongeLayer::rates` (the
polynomial profile `rate_max (d/L)^m` of ADR 0237 DR5), reference state at rest (`p = u = 0`).
- `Plain`: `p_t += -sigma p`, `u_t += -sigma u`. At normal incidence the characteristic variables decouple
  (`w+-_t +- c w+-_x = -sigma w+-`), so the layer itself reflects nothing for any profile (ADR 0237 DR2) and a wave crossing
  it twice is attenuated by `R0 = exp(-2 I/c)`, `I` the integral of `sigma` across the layer.
- `Matched { normal n }`: `p_t += -sigma p`, `(u.n)_t += -sigma (u.n)`, the tangential velocity is not damped but forced,
  `u_t += -(sigma/rho0) (grad Q - (grad Q.n) n)`, with one memory per cell `Q_t = p`. This is ADR 0237 DR4 written for the
  linear unknowns; re-derived here in the frequency domain: with the stretching `d/dx -> d/dx / alpha`,
  `alpha = 1 + sigma/(i omega)`, the Helmholtz equation `-omega^2 p - c^2 (p_xx/alpha^2 + p_yy) = 0` follows from
  `(i omega + sigma) p = -K (u_x + v_y)`, `(i omega + sigma) rho0 u = -p_x`, `i omega rho0 v = -alpha p_y = -(p + sigma Q)_y`,
  `i omega Q = p` (eliminate `u`, `v`, multiply by `i omega/(i omega + sigma)`). The layer is therefore reflection-free in the
  differential equations at every angle, and a mode whose plane-wave components travel at `cos theta = k_z/k` to the axis
  returns from a fully reflecting end behind the layer with `R0^{cos theta}` (ADR 0237 DR4, DR7). `sigma` is taken outside
  the tangential gradient because the profile varies along `n` only.
- `rate_max = (m + 1) c ln(1/R0) / (2 L)` (`sponge::rate_max_for_reflection`). The source is explicit: `rate_max dt` must
  stay well below 2.5; the checks run at 0.124.
- Known from ADR 0237 / 0248 and not removed here: a matched layer does not damp a field that does not vary along `n` (the
  ringing of a rigid duct at its cut-off frequencies). The checks start from rest and ramp the source (DR9).

**DR5. Modes of a hard-walled rectangular duct.** Duct along `x`, section `0 <= y <= a`, `0 <= z <= b`. Separation
`p = psi(y, z) e^{i(omega t - k_z x)}` in `p_tt = c^2 lap p` gives `psi_yy + psi_zz + k_mn^2 psi = 0` with
`k_z^2 = k^2 - k_mn^2`. The wall condition `u.n = 0` is `grad p . n = 0` (DR1, momentum), so `psi_y = 0` at `y = 0, a` and
`psi_z = 0` at `z = 0, b`:

    psi_mn = sqrt(eps_m eps_n) cos(m pi y/a) cos(n pi z/b),   k_mn^2 = (m pi/a)^2 + (n pi/b)^2,   eps_0 = 1, eps_{>0} = 2,

normalised so that the area mean of `psi^2` is 1. A mode propagates when `k > k_mn` (`k_z = +sqrt(k^2 - k_mn^2)` for the wave
going towards `+x`) and is evanescent otherwise (`k_z = -i sqrt(k_mn^2 - k^2)`, decaying towards `+x`). The modes are
orthogonal over the section (products of cosines on `[0, a]`). The two-dimensional channel of height `a` has the modes
`(m, 0)`.

**DR6. Modes of a hard-walled circular duct.** Radius `R`, polar `(r, phi)`: `psi_mn = N_mn^{-1} J_m(j'_mn r/R) e^{i m phi}`,
`m` any integer, `n = 0, 1, ...`, `k_mn = j'_mn/R` with `j'_mn` the `n`-th non-negative zero of `J'_|m|` (`j'_00 = 0`, the
plane wave; for `m != 0` the zero at `x = 0` is not counted because `J_m(0) = 0`). From separation of `lap psi + k_mn^2 psi = 0`
in polar coordinates (Bessel's equation, the solution regular at `r = 0`) and the wall condition `psi_r(R) = 0`.
`J_m(x) = (1/2 pi) integral_0^{2 pi} cos(m tau - x sin tau) d tau` (Bessel's integral), evaluated by the trapezoidal rule on
`2 (ceil(x) + |m| + 32)` points: the integrand is periodic and analytic, and the rule's error is of the size of the Fourier
coefficients beyond the sampling, which decay like `J_{N}(x)` for `N > x`, below `1e-15` here. `J'_m = (J_{m-1} - J_{m+1})/2`,
`J_{-m} = (-1)^m J_m`. The zeros by a scan in steps of 0.05 and bisection to `1e-14`. Normalisation: the area mean of
`|J_m(j r/R)|^2` with `J'_m(j) = 0` is `(1 - m^2/j^2) J_m(j)^2` (`m = 0, j = 0`: 1), the standard integral of a squared
Bessel function (Lommel, from memory); it is checked by quadrature (unit test, `1e-10`), not trusted. Checked independently
of the integral representation: `J_m` against its power series `sum_k (-1)^k (x/2)^{2k+m}/(k!(k+m)!)` for `x <= 8`, and the
zeros by the residual of Bessel's equation and `|J'_m(j)| <= 1e-12`.

**DR7. Modal decomposition of a section.** Given samples `(y_j, z_j)` (or `(r_j, phi_j)`) with weights `w_j > 0` (the cell
areas of the section) and complex values `f_j`, and a list of modes, the amplitudes minimise
`sum_j w_j |f_j - sum_q a_q psi_q(x_j)|^2`; the normal equations `G a = b`, `G_ql = sum_j w_j conj(psi_q) psi_l`,
`b_q = sum_j w_j conj(psi_q) f_j`, are solved by complex Gaussian elimination with partial pivoting. With exact orthogonality
of the sampled modes this is the projection `a_q = <psi_q, f>/<psi_q, psi_q>`; otherwise it is the least-squares fit, which
is exact for a field in the span of the listed modes. The weighted RMS residual is returned. The complex field at one
frequency comes from a DFT of the time series, `A_c = (2/N) sum_n p_c(t_n) e^{-i omega t_n}` over an integer number of
periods with `N` samples per whole run (exact for a harmonic at `omega`; it rejects the mean and the harmonics).

**DR8. Axial split, power and the export.** At stations `x_s` the amplitude of one mode `A(x_s)` is fitted with
`A(x) = a+ e^{-i kz (x - x_ref)} + a- e^{+i kz (x - x_ref)}` by linear least squares (two complex unknowns) for a given `kz`
(the closed form of DR5, DR6 for the export; a fitted one for the measurement, DR9). Power of a normalised mode over the
section area `S`: `W+- = S |a+-|^2 Re(kz)/(2 rho0 omega)` for a propagating mode (from the axial intensity
`Re(p conj(u))/2`, `u = kz p/(rho0 omega)` by DR1), zero for an evanescent one. The export, `rcfd-duct-modes` version 1:

    {"format": "rcfd-duct-modes", "version": 1, "kind": "section" | "axial",
     "duct": {"shape": "rectangular", "a": .., "b": ..} | {"shape": "circular", "radius": ..},
     "frequency": f [Hz], "sound_speed": c, "density": rho0, "x_ref": .. (axial only), "residual": ..,
     "modes": [{"m": .., "n": .., "k_cut": .., "k_axial": [re, im],
                "amplitude": [re, im]                                  (section)
                "plus": [re, im], "minus": [re, im], "power_plus": .., "power_minus": .. (axial)}, ...]}

Written with `serde_json` (shortest round-trip representation of `f64`), read back with errors that name the field
(`modes[3].plus: expected [re, im]`).

**DR9. How the K, S and E numbers are measured.** The solver runs a driven channel to a time-harmonic state; the last
periods are transformed (DR7) per cell; every column of cells (a station) is decomposed into the channel modes `m = 0..7`
(DR7, with the cell areas as weights); the amplitude `A(x_s)` of the driven mode is fitted with the two-wave model of DR8
with a **complex** `kz = kr - i ki` found by minimising the least-squares residual over `kr` and `ki` (nested golden-section
searches, `kr` in `[0.9, 1.1] kz_cf`, inside it `ki` in `[-0.02, 0.05] kz_cf`; a minimum within `1e-3` of a bracket end is a
failed fit and fails the test). Results: `kr` (clause A), the amplitude loss per axial wavelength `1 - exp(-2 pi ki/kr)`, and
`R = |a-/a+|` at `x_ref`, the front of the layer or the end plane (clause B). The evanescent mode (E1) is a single decaying
exponential: `ln|A|` fitted by a straight line.

## Model M1 (scratch, before any code)

Semi-discrete scheme of DR2 on a uniform grid `dx = dy = h/N`, interior only (periodic in both directions, which a hard
wall reproduces by mirror symmetry if the boundary treatment is the mirror; DR3 is that for the flux, approximately for the
gradient): face values `phi (1 + i sin(theta)/2)` and `phi e^{i theta} (1 - i sin(theta)/2)` (the central gradient that the
least-squares fit gives on a uniform grid), the flux of DR2, the 3x3 operator per wave vector `(kx, ky)`, SSP-RK3 amplification
`1 + z + z^2/2 + z^3/6` at `c dt/dx = 0.25`; for `ky = m pi/h` and the driven `omega` the complex `kz` is solved by a secant
iteration so that an eigenvalue's amplification equals `e^{i omega dt}`. With `k1 = pi/h`, `omega = c k1/0.6` (`cos theta = 0.8`):

| N (cells across h) | mode 1: `kr/kz_cf - 1` | mode 1: loss per wavelength | mode 0: `kr/k - 1` | mode 2 (evanescent): decay rate error |
|---|---|---|---|---|
| 16 | `-7.1e-3` | `1.8e-2` | `-8.2e-3` | `+4.2e-2` |
| 32 | `-1.9e-3` | `2.3e-3` | `-2.2e-3` | `+1.1e-2` |
| 64 | `-4.7e-4` | `2.9e-4` | `-5.5e-4` | `+2.9e-3` |

Second order, as designed. The evanescent rate is about six times more sensitive than the propagating wavenumber, hence its
finer grid. The model ignores the walls' gradient treatment, the driven boundary and the sponge; it is an estimate.

## Set-up of the duct runs (fixed here)

Air-like constants: `rho0 = 1.2 kg/m3`, `c = 340 m/s`, channel height `h = 0.12 m` (the 120 mm fan), `box_2d` with patches
`xmin` (Driven), `ymin`, `ymax` (Wall), `xmax` (Wall or Open), `frontAndBack` (empty). Mode 1: `k1 = pi/h = 26.18 1/m`;
`omega = c k1/0.6`, `f = 2361.1 Hz`, `k = 43.63 1/m`, `kz_cf = 0.8 k = 34.907 1/m`, `lambda_z = 0.18 m = 1.5 h`. Grid
`N = 32` across, `dx = h/32`, 200 cells along (length `6.25 h = 0.75 m`), 6400 cells. Station region: columns whose centre
lies in `[0.5 h, 5 h]` (144 stations, three axial wavelengths). Layer (when present): front at `x_s = 5 h = 0.6 m`, thickness
`L = 0.15 m` (40 cells, about one wavelength), exponent 2, `R0 = 1e-6`, `rate_max = 46973 1/s`; then the end face at
`6.25 h`. Time: 160 steps per period (`dt = T/160 = 2.647e-6 s`, `c dt/dx = 0.240`, multi-dimensional CFL 0.48,
`rate_max dt = 0.124`); the drive is ramped with `w(t) = sin^2(pi t/(2 T_r))` for `t < T_r = 8 T`, then 1; 40 periods in all;
DFT over the last 4 (640 samples). Driven ghost state: `p = w(t) psi_m(y) cos(omega t)`, `u_x = (kz_cf/(rho0 omega)) p`,
`u_y = u_z = 0` (only the normal velocity enters the flux). Amplitude 1 Pa (the equations are linear).

Transit (group velocity `0.8 c`): inlet to end and back 13 periods; after the ramp, the transient has crossed twice by
period 34. Ramp spectrum at the cut-off (`0.4 omega` below `omega`): about `pi/(4 T_r^2 Delta^2) = 1.9e-3` of the amplitude,
and the 4-period DFT passes about 0.12 of a component `1.6` cycles off: `2.4e-4` contamination, against limits of `1e-2`.

| run | mode | end | layer | grid | used by |
|---|---|---|---|---|---|
| S1 | 1 | Wall | matched, normal `+x` | N = 32 | K1, K1b, S1 |
| S1c | 1 | Wall | matched | N = 16, 100 cells along, 80 steps per period | S1c (fast covering test) |
| P1 | 0 (plane) | Wall | plain | N = 32 | K0, P1 |
| M1 | 1 | Open | none | N = 32 | M1 |
| NC1 | 1 | Wall | none | N = 32 | NC1 |
| W1 | 1 | Wall | matched | N = 32, warped (below) | W1k, W1s |
| E1 | 2 (evanescent) | Wall | none | N = 64, length `3 h` (192 cells), 320 steps per period, ramp 16 T, 32 periods, DFT over the last 8 | E1 |

W1: the S1 mesh with every point moved in `y` only, `y -> y + 0.05 h sin(2 pi x/h) sin(pi y/h)` (walls and the `x = const`
column faces stay; rows are sheared by up to 17 degrees and cell heights vary by `+-16` percent): a non-orthogonal mesh on
which the least-squares gradient and the area weights of DR7 matter.

E1: `omega` as above; mode 2 is cut off (`k2 = 2 pi/h = 52.36 1/m > k`), `alpha_cf = sqrt(k2^2 - k^2) = 28.94 1/m`; drive
`p = w psi_2 cos(omega t)`, `u_x = (alpha_cf/(rho0 omega)) w psi_2 sin(omega t)` (the evanescent field's velocity, a quarter
period out of phase); fit of `ln|A_2|` over stations in `[0.25 h, 1.25 h]` (amplitude from 1 to `e^{-3.47}`); the field from
the end wall returns to `1.25 h` with `e^{-2 alpha (3h - 1.25h)} = 5e-6`. The longer ramp and DFT window are for the
propagating mode-2 content of the ramp above `1.2 omega`, which no sponge removes here: about `2e-3` of the amplitude times
the DFT's passband at `1.6` cycles off, against a local amplitude of at least `0.03`.

## Synthetic export set-ups (clause C)

**X1, rectangular, the whole chain.** `a = 0.12 m`, `b = 0.08 m`, `c = 340`, `rho0 = 1.2`, `k = 60 1/m`. Propagating modes
(DR5): `(0,0) 0`, `(1,0) 26.18`, `(0,1) 39.27`, `(1,1) 47.20`, `(2,0) 52.36`; evanescent `(2,1) 65.45`, `(0,2), (3,0) 78.54`.
Synthetic field: the five propagating modes in both directions plus `(2,1)` decaying towards `+x`, with fixed complex amplitudes
(table in the test, magnitudes 0.2 to 1, `minus` of `(2,0)` zero), evaluated exactly at the centres of a 24 x 16 cell section
at 7 stations `x = 0, 0.05, .., 0.30 m`, as time series of 64 samples per period over 2 periods. Chain: DFT (DR7) -> section
fit on the 8 modes listed -> axial split with the closed-form `kz` (DR8) -> export as JSON text -> read back -> compare.

**X2, circular, one section with truncation.** `R = 0.06 m`, `k R = 4.5`: propagating `(0,0)`, `(+-1,1)`, `(+-2,1)`,
`(0,1)`, `(+-3,1)` (the zeros below 4.5 as computed by DR6; the list is formed by the library from `k`, not typed in),
complex amplitudes fixed in the test (magnitudes 0.2 to 1); plus two modes **outside** the fitted basis, `(4, 1)` and
`(0, 2)`, each with magnitude 0.05 (the test asserts that the library finds both cut off at `k R = 4.5`). `(4, 1)` is
orthogonal to every basis mode on the 48 angular samples exactly (`|m - m'| < 48`); `(0, 2)` leaks into `(0, 0)` and `(0, 1)`
through the radial midpoint rule, which is what X2 measures. Sampling: polar
cells, 24 radial x 48 angular, centres at the mid-radius and mid-angle, weights the exact sector areas. Basis of the fit: the
propagating modes only. Export kind `section`, read back, compare.

## Registered checks

All limits fixed now. `kz_cf`, `R_cf`, `alpha_cf` are closed forms (DR3, DR5, DR6).

| id | clause | quantity | limit | estimate (source) |
|---|---|---|---|---|
| K1 | A | S1: `|kr/kz_cf - 1|`, mode 1 | `<= 1e-2` | `1.9e-3` (M1) |
| K1b | A | S1: amplitude loss per axial wavelength | `<= 1e-2` | `2.3e-3` (M1) |
| S1 | B | S1: `R = |a-/a+|` at the layer front, rigid end behind the matched layer | `< 1e-2` | `R0^0.8 = 1.6e-5` plus the discrete layer, ADR 0237 measured `1e-4` to `1e-3` at 30 to 45 degrees: `<= 1e-3` |
| X1 | C | X1: every `plus`/`minus` read back from the JSON against the synthetic value, `|a_rec - a|/|a|` for `a != 0`, `|a_rec|/max|a|` for `a = 0` | `<= 1e-2` each | round-off, `<= 1e-9` (the field lies in the span) |
| X2 | C | X2: every section amplitude read back, the same measure | `<= 1e-2` each | leakage of the truncated modes through the non-orthogonality of the radial midpoint quadrature, `<= 1e-3` |
| K0 | A | P1: `|kr/k - 1|`, plane wave | `<= 1e-2` | `2.2e-3` (M1) |
| P1 | B | P1: `R`, plain layer at normal incidence | `< 1e-2` | `R0 = 1e-6` plus discrete, `<= 1e-4` |
| S1c | A, B | S1c: `|kr/kz_cf - 1|`; `R` | `<= 3e-2`; `< 1e-2` | `7.1e-3` (M1); `<= 3e-3` |
| M1 | method | M1: `|R - 1/9|`, open end, `R_cf = (1 - cos)/(1 + cos) = 1/9` at `cos theta = 0.8` | `<= 2e-2` | discrete boundary, `<= 1e-2` |
| NC1 | control | NC1: `R`, rigid end without a layer | `>= 0.9` | `1 - 2 x 1.25 h` of loss, `~0.99` |
| W1k | A (mesh) | W1: `|kr/kz_cf - 1|` | `<= 1e-2` | not modelled; second order, assumed within twice S1: `<= 4e-3` |
| W1s | B (mesh) | W1: `R` | `< 1e-2` | `<= 2e-3` |
| E1 | A (cut off) | E1: `|alpha/alpha_cf - 1|`, mode 2 | `<= 1e-2` | `2.9e-3` (M1) |

The acoustic check of the acceptance is decided by K1, K1b, S1, X1, X2. K0, P1, W1k, W1s and E1 are additional modes, forms
and meshes; M1 validates the measurement against a closed-form reflection; NC1 is the negative control (the measurement
sees a reflection when there is one). S1c is the fast covering test of the S1 code path if S1 is heavy.

**Unit and fast checks** (in `rcfd-fv/tests/acoustic_wave_f333.rs` and the unit tests of `duct_modes.rs`):

| id | check | limit |
|---|---|---|
| U1 | closed box (`box_mesh` 12 x 10 x 8, warped by a smooth map, all Wall), smooth initial `p`, 200 steps: `|sum pV - sum p0 V|` / `sum |p0| V` | `<= 1e-12` |
| U2 | the factored least-squares gradient against `gradient::least_squares` on the U1 mesh, with the same boundary values | `<= 1e-12` relative to the largest gradient |
| U3 | uniform `p0`, `u = 0`, uniform rate `sigma` in every cell (plain and matched), 100 steps at `sigma dt = 0.1`: `p` against `p0 G^n`, `G = 1 - z + z^2/2 - z^3/6` (the RK3 amplification); matched form: `u` stays exactly 0 | `<= 1e-12` relative; exactly |
| U4 | refusals: `rho0`, `c` not positive; a rate negative, non-finite, or of the wrong length; `dt` not positive or non-finite; a cyclic patch; a boundary map naming a patch that does not exist or missing a non-empty patch; a matched normal of zero length | an error each |
| U5 | 1-D tube 400 x 1, Gaussian pulse (width 20 cells) towards an `Open` end: largest `|p|` after it left / peak | `<= 1e-2` (estimate `<= 1e-3`) |
| U6 | the same pulse between two Walls: peak position after `t = 100 dx/c` against `x0 + c t` | `<= 1 dx` |
| U7 | zero initial state with Wall boundaries: all values stay exactly 0 | bit for bit |
| B1 | Bessel: `J_m` (integral) against the series, `m = 0..5`, `x = 0..8` | `<= 1e-12` |
| B2 | the zeros: `|J'_m(j'_mn)| <= 1e-12`, Bessel residual, increasing, `m = 0..6`, `n <= 4` | as stated |
| B3 | normalisation of rectangular and circular modes by quadrature (2000 x 2000 midpoints / 400 radial x 256 angular) | area mean of `|psi|^2` within `1e-5` of 1 (estimate: the midpoint rule, `~1e-6`) |
| J1 | JSON: `read(write(e)) == e` bit for bit for both kinds; malformed inputs give errors naming the field (format, version, kind, a mode's complex entry) | as stated |

## Planned mutations (scratch copy, once each, not committed)

| id | mutation | expected to be caught by |
|---|---|---|
| MU1 | the upwind terms of the flux dropped (central flux) | U5 or S1c/K1b (an undamped odd-even mode); recorded whichever |
| MU2 | matched form: the tangential force removed (only `p` and `u.n` damped) | S1 / S1c (a mismatched layer at 37 degrees) |
| MU3 | `Wall` treated as `Open` | U1, NC1, K1 (the cut-off changes) |
| MU4 | reconstruction removed (first order) | K1b, S1c (the loss per wavelength) |
| MU5 | the `sqrt(eps_m eps_n)` of the rectangular modes dropped | B3, X1 |
| MU6 | the axial split with `e^{+i kz x}` for the plus wave | X1 |
| MU7 | circular modes on the zeros of `J_m` instead of `J'_m` | B2, X2 |
| MU8 | the DFT normalisation `2/N` replaced by `1/N` | X1 |

A mutation that survives every check is reported as such and diagnosed.

## Decision rule (registered)

- **done** when K1, K1b, S1, X1 and X2 meet their limits, U1 to U7, B1 to B3 and J1 pass, NC1 holds, the baseline hashes
  are unchanged, and no mutation survives undiagnosed. The note then says "library only" (D3).
- **partial** when one of K1, K1b, S1, X1, X2 misses (kept as `*_known_miss`, diagnosed), or a mutation survives without a
  diagnosis, or a run of the acceptance could not be made.
- **todo** does not apply once code exists.
- K0, P1, S1c, M1, W1k, W1s and E1 do not decide the status; a miss there is kept as a `*_known_miss`, diagnosed and named in
  the note. If M1 misses, the R of S1 is still read against its limit, but the addendum says that the measurement was not
  validated.

## Limits (what is not covered)

- Medium at rest: no mean flow, no convected wave equation, no refraction by temperature gradients.
- Linear: no amplitude effects. Not coupled to a CFD solution (no hybrid source); the source is a driven boundary.
- The simulated ducts are two-dimensional channels (the `(m, 0)` modes of a rectangular duct). Three-dimensional and
  circular ducts are not run through the solver; circular modes enter only the export check. A circular duct on a mesh of
  straight-sided cells has a polygonal section whose cut-offs differ from the circle's.
- No impedance (lined) wall in the wave solver, no frequency-domain (Helmholtz) solver, no rotating or swirling modes.
- The sponge forms are the linear plain and matched forms (one planar layer); no corner, no shift (ADR 0248's options).
- Library only: no case key, CLI or MCP tool.
- Verification against closed forms only; no measurement.

## Sources

- A. D. Pierce, *Acoustics*, ASA 1989, ch. 1 (linear acoustics, energy) and ch. 7 (guided waves in ducts); P. M. Morse and
  K. U. Ingard, *Theoretical Acoustics*, McGraw-Hill 1968, ch. 9 (duct modes, cut-off). Cited from memory; DR1, DR5, DR6 are
  derived above.
- R. J. LeVeque, *Finite Volume Methods for Hyperbolic Problems*, CUP 2002, ch. 3 and 4 (Riemann problem of a linear system,
  Godunov flux), from memory; DR2 is derived above.
- C.-W. Shu, S. Osher, J. Comput. Phys. 77 (1988) 439-471 (SSP Runge-Kutta), from memory.
- J.-P. Berenger, J. Comput. Phys. 114 (1994) 185-200; F. Q. Hu, J. Comput. Phys. 173 (2001) 455-480 (matched layers), from
  memory; the form used is ADR 0237 DR4, re-derived in DR4.
- G. N. Watson, *A Treatise on the Theory of Bessel Functions*, 2nd ed., CUP 1944, ch. 2 and 5 (Bessel's integral, the
  series, the integral of `J_m^2`), from memory; each is checked numerically (B1 to B3) rather than trusted.

No vendor code or documentation was used.

## Addendum 1: results (2026-11-12; the registered text above is unchanged)

Registration 59eab488; library and tests 4a020cf8 (one commit, after the registration). Every run below was made once, in
the **release** profile (`cargo test --release -p rcfd-fv --test acoustic_wave_f333`, `cargo test --release -p rcfd-post
--lib duct_modes`, `cargo test --release -p rcfd-post --test duct_modes_f333`). No limit was changed, no amendment was
needed, there is no `*_known_miss` test because no limit was missed, and no test is ignored: the longest run (E1) took
under 50 s with six tests in parallel, so no registered run counts as heavy and S1c is an ordinary fast test.

### What was built

- `crates/rcfd-fv/src/acoustic_wave.rs` (new): `AcousticWaveSolver` (DR1 to DR4), `Medium`, `WaveBc` (`Wall`, `Open`,
  `Driven`), `WaveSponge` / `WaveSpongeForm` (`Plain`, `Matched`), `WaveSponge::from_layer` on `sponge::SpongeLayer`,
  `max_dt`, `pressure_integral`, `energy`, `gradient`.
- `crates/rcfd-post/src/duct_modes.rs` (new): `C64`, `bessel_j`, `bessel_jp`, `bessel_jp_zeros`, `DuctShape`, `Mode`,
  `ModeBasis` (`new`, `below`, `eval`), `axial_wavenumber`, `decompose_section`, `HarmonicDft`, `split_axial`, `fit_axial`,
  `fit_exponential`, `ModalExport` (`section`, `axial`, `to_json`, `to_json_string`, `from_json`, `from_json_str`); unit
  tests B1 to B3, J1.
- `crates/rcfd-fv/src/lib.rs` line 5 and `crates/rcfd-post/src/lib.rs` line 8: one `pub mod` line each. No other existing
  line changed.
- Tests: `crates/rcfd-fv/tests/acoustic_wave_f333.rs` (U1 to U7), `crates/rcfd-post/tests/duct_modes_f333.rs` (S1, S1c,
  P1, M1, NC1, W1, E1, X1, X2).
- `docs/acoustics.md`: a section "Acoustic wave solver and duct modes", two rows of the module table, and the two sentences
  of "Limits of the whole chain" that said the wave solver did not exist.

### Checks

| id | quantity | limit | measured | met |
|---|---|---|---|---|
| K1 | S1: `kr/kz_cf - 1`, mode 1 | `<= 1e-2` | `-1.855e-3` (`kr = 34.8418`, `kz_cf = 34.9066 1/m`) | yes |
| K1b | S1: amplitude loss per axial wavelength | `<= 1e-2` | `2.28e-3` | yes |
| S1 | S1: `R`, matched layer before a rigid end | `< 1e-2` | `8.99e-5` | yes |
| X1 | X1: worst recovery error of 16 axial amplitudes through DFT, fit, split, JSON | `<= 1e-2` | `4.17e-15` | yes |
| X2 | X2: worst recovery error of 8 section amplitudes with two truncated modes, through JSON | `<= 1e-2` | `1.35e-4` | yes |
| K0 | P1: `kr/k - 1`, plane wave | `<= 1e-2` | `-2.179e-3` | yes |
| P1 | P1: `R`, plain layer, normal incidence | `< 1e-2` | `9.77e-7` | yes |
| S1c | S1c: `kr/kz_cf - 1`; `R` | `<= 3e-2`; `< 1e-2` | `-7.140e-3`; `4.23e-4` | yes |
| M1 | M1: `|R - 1/9|`, open end | `<= 2e-2` | `R = 0.1092`, difference `1.9e-3` | yes |
| NC1 | NC1: `R`, rigid end, no layer | `>= 0.9` | `1.003` | yes (held) |
| W1k | W1: `kr/kz_cf - 1`, warped mesh | `<= 1e-2` | `-1.926e-3` | yes |
| W1s | W1: `R`, warped mesh | `< 1e-2` | `7.91e-5` | yes |
| E1 | E1: `alpha/alpha_cf - 1`, cut-off mode 2 | `<= 1e-2` | `+1.658e-3` (`28.991` against `28.943 1/m`) | yes |
| U1 | relative change of `sum p V`, closed warped box, 200 steps | `<= 1e-12` | `6.5e-15` | yes |
| U2 | factored gradient against `gradient::least_squares` | `<= 1e-12` relative | `0` (bit for bit) | yes |
| U3 | `p` against `p0 G^n`, plain and matched; `u` | `<= 1e-12`; exactly 0 | `1.04e-14` both; `0` | yes |
| U4 | refusals | an error each | all refused; a valid step accepted | yes |
| U5 | largest `|p|` after a pulse left through `Open` | `<= 1e-2` | `4.0e-11` | yes |
| U6 | pulse peak against `x0 + c t` | `<= 1 dx` | `0.12 dx` | yes |
| U7 | rest stays at rest | bit for bit | yes | yes |
| B1 | Bessel integral against the series | `<= 1e-12` | `2.0e-14` | yes |
| B2 | `|J'_m(j'_mn)|`; increasing; Bessel residual | `<= 1e-12` | `1.6e-14`; yes; `5.5e-8` (difference quotient, test limit `1e-6`) | yes |
| B3 | area mean of `|psi|^2` against 1, 5 rectangular and 22 circular modes | `<= 1e-5` | `5.3e-6` | yes |
| J1 | JSON round trip bit for bit, both kinds; six malformed inputs name the field | as stated | yes | yes |

Other numbers of the runs (not checks): the modes not driven stay below `4e-15` of the driven one on the Cartesian grids
and at `1.7e-3` on the warped grid; the two-wave fit's RMS residual is `3.1e-5` (S1) of an amplitude of 0.98;
`rate_max dt = 0.121` at the deepest cell centre (registered 0.124 for the profile's end); X2's section residual `7.1e-2`
is the two truncated modes.

What the numbers say: the measured K1 (`-1.855e-3`), K1b (`2.28e-3`), K0 (`-2.179e-3`) and S1c (`-7.140e-3`) are model M1's
predictions to three digits (`-1.854e-3`, `2.28e-3`, `-2.179e-3`, `-7.139e-3`), so the wall treatment, the driven boundary
and the fit add nothing visible to the interior dispersion of the scheme; the wavenumber error is second order and the
1 percent clause is met with a margin of 5 at 32 cells across. The evanescent rate came out better than M1's estimate
(`1.7e-3` against `2.9e-3`): M1 models a periodic transverse direction, the run has walls and a short fit range; not
diagnosed further. The layer reflects `9e-5`, 5.6 times its differential `R0^0.8 = 1.6e-5`, the size ADR 0237 found for the
compressible matched layer; it is the discrete layer. M1 (0.1092 against 0.1111) validates the measurement of `R` on a
reflection of known size; NC1 shows it sees a full one.

### Mutations

Once each, on a `git archive` copy of 4a020cf8 (`w49i-work/f333-scratch/tree`, own target directory; script `mutate.py`,
not committed), running the three F-333 test targets with `--no-fail-fast`. An unmutated control run of the copy first
(MU0): all 20 tests pass with the numbers above to every printed digit, which also confirms that the committed (formatted)
bytes give the results recorded here.

| id | mutation | caught by | against the expectation |
|---|---|---|---|
| MU1 | upwind terms of the internal-face flux dropped (central flux) | U5 only (`1.081e-2` against `1e-2`) | caught, **narrowly and not where expected**: no odd-even mode appears in the duct runs, which all pass; the loss per wavelength falls from `2.3e-3` to `3.5e-5` and K1 is unchanged (`-1.855e-3`). Diagnosis: with the linear reconstruction the upwind terms act on the face jump `pL - pR`, which is `O(theta^3)` for a smooth field, so they set only the small dissipation; a smoothly driven field does not excite the checkerboard mode they suppress. The duct checks have no lower limit on the loss, so they cannot see the change |
| MU2 | matched form: tangential force removed | S1 (`R = 3.85e-2`), S1c (`3.87e-2`), W1s (`3.85e-2`) | as expected; P1 (plain form) and the runs without a layer pass |
| MU3 | `Wall` treated as `Open` | U1 (`1.0`), U3, U5, U6, S1, S1c, P1, M1, NC1, W1, E1 (`-0.41`) | as expected, and wider (the fits of the duct runs leave their brackets) |
| MU4 | reconstruction removed (first order) | S1, S1c, P1, M1, NC1, W1: the fitted `ki` reaches the end of its bracket (`0.05 kz`), a failed fit by the registered rule (DR9), before K1b is read | caught; the registered route was K1b. U1 to U7 and E1 (`+8.7e-4`) pass: E1's decay is set by the cut-off, not by the dissipation |
| MU5 | `sqrt(eps_m eps_n)` dropped | B3 (`0.75`) only | caught, but **X1 does not see it**: X1 builds its synthetic field with the library's own mode shapes, so a wrong normalisation is consistent between field and fit; only the amplitudes in the file change scale (`|plus|` of S1 1.39 instead of 0.98). B3 is the check of the normalisation |
| MU6 | plus wave with `e^{+i kz x}` | X1 and every duct run (the two-wave system is singular, `fit_axial` and `split_axial` return an error) | as expected |
| MU7 | circular modes on the zeros of `J_m` | B2 (`0.52`), B3, J1, X2 | as expected |
| MU8 | DFT normalisation `1/N` | X1 (`0.50`) | as expected; the duct runs pass because K, R and E are ratios |

No mutation survived. After the mutations the copy's sources equal the worktree's (`diff -r --strip-trailing-cr`).

### fmt, clippy, baseline hashes

`cargo fmt -p rcfd-fv --check` and `cargo fmt -p rcfd-post --check`: clean (after `cargo fmt -p` reformatted the four new
files before the commit). `cargo clippy --release -p rcfd-fv -p rcfd-post --all-targets -- -D warnings`: clean (after two
`manual_range_contains` rewrites in the duct test, before the commit). `cargo test -p rcfd-api --release --test
baseline_hashes`: 3 passed, 1 ignored, as before; the file was not edited.

### Deviations and disclosures

- The checks were run on the source before `cargo fmt` and the two clippy rewrites (`x >= lo && x <= hi` to
  `(lo..=hi).contains(&x)`); the committed bytes were then run once as the mutation control MU0 with identical numbers.
- W1: the fixed step is `1.09` times `max_dt(0.48)` on the warped mesh (multi-dimensional number 0.52); the registration
  fixed the step from the Cartesian grid. It ran stably; recorded, not a check.
- E1: the amplitude over the fit range fell from 0.41 to `1.3e-2` (registered: from 1 to `e^{-3.47}`; the ratio is the
  same, the start is lower because the driven face does not impose the evanescent field exactly).
- X2 labels: the registration names the modes in the traditional way (radial count from 1 for `m != 0`, from 0 for `m = 0`);
  the library counts from 0 for every `m`. The physical modes are the same: `(4, 1)` of the text is the library's `(4, 0)`,
  `(+-1, 1)` is `(+-1, 0)`, `(0, 1)` and `(0, 2)` agree. The test asserts the library's basis of 8 modes and that both extra
  modes are cut off.
- B2: the registration gives no number for the Bessel-equation residual; the test uses `1e-6` for a second difference
  with step `1e-4` (measured `5.5e-8`). B3 came out at `5.3e-6` against an estimate of `~1e-6` and a limit of `1e-5`; the
  estimate did not take the worst mode (the midpoint error grows with `|psi(R)|^2`, about `1/(1 - m^2/j^2)`); not diagnosed
  further.
- In the duct runs the test sets the solver time to `(step + 1) dt` after each step so that the DFT samples are uniform;
  the solver's own `t0 + dt` differs by rounding only.
- The books and papers are cited from memory; each formula used is derived in DR1 to DR9 and the Bessel results are
  checked numerically. No vendor code or documentation was used.

### Not run

The whole workspace test suite (the lead runs the gate); tests of other crates apart from `baseline_hashes`; the other
tests of `rcfd-fv` and `rcfd-post` (no existing line of either crate changed except one `pub mod` line each); the debug
profile; a three-dimensional or circular duct through the solver; a mean flow; frequencies other than the one registered.

### Decision by the registered rule

K1, K1b, S1, X1 and X2 meet their limits; U1 to U7, B1 to B3 and J1 pass; NC1 holds; the baseline hashes are unchanged;
every mutation is caught (MU1, MU4 and MU5 by other checks than expected, each diagnosed above). **done**, library only
(D3). K0, P1, S1c, M1, W1k, W1s and E1 also meet their limits.

### Remaining questions (options and recommendation)

1. **Wiring.** (a) a case section `[acoustics.wave]` (patches to `wall`/`open`/`driven` with a mode, frequency and
   amplitude; an optional `sponge` table) and a CLI `rcfd duct-modes` that writes the export from a run; (b) the CLI only,
   reading a saved field; (c) stay library-only. Recommendation: (b) first (no case key or hash-table edit; its help text
   needs approval of the CLI golden), (a) when a fan case needs it. Not taken: both change goldens.
2. **Coupling to CFD.** (a) drive the wave solver from a sampled CFD surface (the `rcfd-surface-series` files of the FW-H
   chain) as a driven boundary; (b) a hybrid source term from an incompressible run; (c) none. Recommendation: (a), the
   smallest step, as a follow-up item.
3. **Circular and three-dimensional ducts through the solver.** A circular duct on straight-sided cells has polygonal
   cut-offs; the check needs a grid fine enough that the polygon's cut-off error is inside the budget, estimated first.
   Recommendation: a follow-up item on an O-grid duct.
4. **A lower limit on the loss per wavelength** would have caught MU1 in the duct runs. Recommendation: none now (U5
   catches it; the checkerboard mode is a robustness matter, not an accuracy one); note it for the wiring item.

### For the lead: roadmap entries

**F-333, proposed status: `done`.** Proposed note text:

> ADR 0351, 2026-11-12. The acceptance text is MET IN THE LIBRARY, not from a case file. Built: rcfd-fv `acoustic_wave.rs` (`AcousticWaveSolver`), the linear acoustic equations about a medium at rest on any FV mesh: exact linear Riemann flux, unlimited least-squares reconstruction, SSP-RK3; boundaries `Wall`, `Open` (characteristic), `Driven` (a ghost state for an entering wave); sponge `WaveSponge` plain or matched (the ADR 0237 layer in the linear unknowns) on `SpongeLayer`. rcfd-post `duct_modes.rs`: hard-walled rectangular and circular duct modes (Bessel functions and zeros of J' computed), `axial_wavenumber`, `HarmonicDft`, weighted least-squares section fit, axial split into the two directions (closed-form or fitted kz), modal power, the `rcfd-duct-modes` v1 JSON export with located read errors. Registered checks, all met at the first run, no amendment: a 0.12 m channel, 32 cells across, mode 1 at cos theta = 0.8 (2361 Hz): axial wavenumber 1.86e-3 below the closed form sqrt(k^2 - k_mn^2) (limit 1e-2; a Fourier analysis of the scheme predicted -1.854e-3), loss 2.3e-3 per wavelength (limit 1e-2); a 40-cell matched layer before a rigid end reflects 9.0e-5 (limit 1e-2; 7.9e-5 on a sheared mesh); a plain layer 9.8e-7 of the plane wave; the open end 0.109 against 1/9; the rigid end without a layer 1.003; a cut-off mode decays 1.7e-3 off the closed form; a synthetic rectangular field (five modes both ways and a cut-off one) through DFT, fit, split and JSON recovered within 4e-15 and a circular section with two truncated modes within 1.3e-4 (limit 1e-2). Eight mutations caught. Limits: medium at rest, linear, no impedance wall, not coupled to a CFD run (the source is a driven boundary); the solver was checked in two-dimensional channels only (circular modes in the export only); one planar layer; no case key or CLI (ADR 0351 question 1).

**F-243, note:** in its text, replace "Not built: wave-equation solver with sponge layers, modal export, quadrupole volume
term, comparison with a measured fan, calibration of the proxy constants, on-the-fly sampling." by:

> Not built: quadrupole volume term, comparison with a measured fan, calibration of the proxy constants, on-the-fly sampling. The wave-equation solver with sponge layers and the modal export were built under F-333 (ADR 0351, 2026-11-12, library only).

Proposed follow-up items (numbers for the lead to assign):

- **`rcfd duct-modes` command.** Deps: F-333. Size S. Acceptance:

> rcfd duct-modes reads a saved pressure time series on a duct section and writes the rcfd-duct-modes export; on the ADR 0351 X1 field it reproduces the library's export bit for bit; located errors for a section outside the duct and a non-integer number of periods; the CLI help golden updated with the maintainer's approval.

- **Wave solver driven by a sampled CFD surface.** Deps: F-333. Size M. Acceptance:

> A driven boundary built from an rcfd-surface-series file reproduces, in a duct, the modal amplitudes of a closed-form duct field sampled into that file within 1 percent; a series that does not cover a whole number of periods is refused.

- **Circular and three-dimensional ducts through the wave solver.** Deps: F-333. Size M. Acceptance:

> A spinning mode (1, 0) and a radial mode (0, 1) of a hard-walled circular duct on an O-grid propagate with the closed-form axial wavenumber within 1 percent after the polygonal cut-off error of the grid, estimated in the registration, is accounted for; a matched layer ends the duct with less than 1 percent reflection.
