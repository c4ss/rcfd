# ADR 0391: F-399 registration: harmonic response by mode superposition, modal damping, static correction, effective mass, force harmonics

Status: **REGISTERED 2026-11-13, written before any code of F-399 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-399 was written, compiled or run: the code listed under "What was read" was read, and
the arithmetic below was done by hand and with a short scratch script (`f399-scratch/predict.py`, numpy only, disclosed under
"Arithmetic"; it does not call rcfd). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parent: [ADR 0302](0302-harmonic-response-by-mode-superposition.md) (SI-12, the stub with the acceptance draft), plan
[ADR 0290](0290-structural-solver-intermediate-tier-plan.md) (section 2.2: module `harmonic`; dynamics in this tier: mode
superposition with modal damping and a static correction, effective masses, Fourier harmonics of a sampled force history, the
forcing of fan item E3). Roadmap item F-399 (deps F-388, F-374, F-383). Size M. Branch `f399-harmonic-response` from main
ae92ee27, worktree `w49i-work/f399`, scratch `w49i-work/f399-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (this item
fills the stub module `harmonic.rs`, which today holds two doc lines and no item, and adds a test file; no other library line
changes); no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited; no new
dependency, no unsafe code; clean-room. `modal.rs` and `mpc.rs` (another package works there in parallel) are not edited; their
public items (`ModalResult`, `modal_analysis`, `mpc::uses_transformation`) are used read only.

## What F-399 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. A load proportional to M phi_1 at omega_1 gives the
> amplitude of the static response divided by 2 zeta within 1e-8 and a phase lag of 90 degrees within 1e-6 rad; the half-power
> width of that peak is 2 zeta omega_1 within 1 percent (zeta = 0.01). Fixed-free bar with nu = 0 under a harmonic tip force,
> undamped, at 0.5 and 1.5 times the first frequency: tip amplitude F tan(k L)/(E A k), k = omega/c, within 0.5 percent with 10
> modes and the static correction, the same without the correction being the registered control (error above 1 percent at
> 0.5). Effective mass: the first axial mode of the bar carries 8/pi^2 of the mass within 0.5 percent, the first bending mode of
> a cantilever 0.613 (recomputed in the registration) within 1 percent, and the sum over all modes of a small model equals the
> unconstrained mass within 1e-10. Force harmonics of a sampled signal with three harmonics: amplitudes and phases within
> 1e-12; a record that is not a whole number of periods is refused.

Clauses: (h1) load `M phi_1` at `omega_1`: amplitude = static / (2 zeta) within 1e-8; (h2) phase lag 90 degrees within 1e-6
rad; (h3) half-power width `2 zeta omega_1` within 1 percent, `zeta = 0.01`; (h4) fixed-free bar, `nu = 0`, undamped, tip force
at `0.5` and `1.5 omega_1`: tip amplitude `F tan(kL)/(EAk)` within 0.5 percent with 10 modes and the static correction; (h5)
the control: without the correction the error at `0.5` is above 1 percent; (h6) effective mass of the bar's first axial mode
`8/pi^2` of the mass within 0.5 percent; (h7) of a cantilever's first bending mode 0.613 (recomputed here: 0.6130761) within 1
percent; (h8) the sum over all modes of a small model equals the unconstrained mass within 1e-10; (h9) force harmonics of a
sampled signal with three harmonics, amplitudes and phases within 1e-12; (h10) a record that is not a whole number of periods is
refused.

## What was read (no run)

- ADR 0302 (whole), ADR 0290 (the dynamics lines of sections 1 to 3 and the module table), ADR 0389 (whole: the transient
  integrator, its style and its bar), ADR 0384 (head).
- `crates/rcfd-fem/src/harmonic.rs` (stub, two doc lines), `lib.rs` (module declared), `modal.rs` (`ModalResult`: public
  `eigenvalues`, `modes` M-orthonormal over the model's `DofMap` with 0 at prescribed equations, `dofs()`; `modal_analysis`,
  `ModalOptions::{new, tolerance, residual_test}`), `transient.rs` (`Integrator::new`, `LoadHistory`, `LoadTable` piecewise
  linear and exact at its points, time `t0 + n dt`, the private `factor` with the pivot rule `d_i <= PIVOT_TOLERANCE a_ii`),
  `mass.rs` (`consistent_with`, `rigid_motions`), `constraint.rs` (`prescribed_equations`, `reduce`), `solve.rs`
  (`solve_static_with`, `PIVOT_TOLERANCE = 1e-10`), `recover.rs` (`IntegrationPointField::compute`, which subtracts the model's
  thermal strain), `mpc.rs` lines 40 to 60 and 1265 to 1280 (`uses_transformation`, `modal_analysis_mpc`), `model.rs`
  (`Load::nodal_force`: the same force on each node of a set), `hex20.rs` (`upgrade_to_hex20`), `rcfd-linalg/src/eigen.rs`
  (`generalized_eigen_dense`: Cholesky of `M` and cyclic Jacobi, all pairs, M-orthonormal; `ResidualTest::Backward`),
  `rcfd-linalg/src/skyline.rs` (`SkylineLdlt::factor`, `solve`), `tests/si_transient.rs` (the bar, `Checks`),
  `tests/si_scaffold.rs` (X1 uses `harmonic` by path only; no refusal row of `harmonic` exists, so none is retired).

## Design

### D1. Mode superposition with modal damping

The reduced system (prescribed unknowns eliminated, zero values only) `M x'' + C x' + K x = Re(F e^{i omega t})`, `F = F_re + i
F_im` a complex load vector. With the M-orthonormal modes `phi_r`, `K phi_r = lambda_r M phi_r`, `omega_r = sqrt(lambda_r)`, and
damping that is diagonal in the modes (modal damping ratios `zeta_r`: `phi_r^T C phi_s = 2 zeta_r omega_r delta_rs`, the
definition of modal damping), `x = sum_r phi_r q_r` decouples into `q_r'' + 2 zeta_r omega_r q_r' + lambda_r q_r = phi_r^T F
e^{i omega t}`, whose steady state is (first principles, substitute `q_r = Q_r e^{i omega t}`)

    Q_r = H_r(omega) phi_r^T F,   H_r(omega) = 1 / (lambda_r - omega^2 + 2 i zeta_r omega_r omega).

The response is `x(t) = Re(X e^{i omega t})`, `X = sum_r phi_r Q_r`; at equation `j` the amplitude is `|X_j|` and the phase
`arg X_j` (`x_j(t) = |X_j| cos(omega t + arg X_j)`; a phase lag behind a real load is `-arg X_j`).

Damping input `ModalDamping`: `Ratio(zeta)` (every mode), `PerMode(Vec<f64>)` (one per mode of the basis), `Rayleigh { mass:
alpha, stiffness: beta }` (`C = alpha M + beta K` is diagonal in the modes with `2 zeta_r omega_r = alpha + beta lambda_r`; the
modal form used is `2 zeta_r omega_r = alpha + beta lambda_r` directly, so a zero eigenvalue needs no division). Ratios finite and
`>= 0`, refused with key `"damping"` otherwise or for a wrong length.

**Single-mode resonance (h1, h2), closed form.** `F = M phi_1` (real), `omega = omega_1`: `phi_r^T M phi_1 = delta_r1`, so `X =
phi_1 / (2 i zeta omega_1^2) = -i (phi_1/lambda_1)/(2 zeta)`; the static response is `K^{-1} M phi_1 = phi_1/lambda_1`. Amplitude
`|X_j| = |x_s,j|/(2 zeta)` and phase `arg(X_j/x_s,j) = -pi/2` exactly (lag 90 degrees). The static correction (D2) adds `K^{-1}F
- sum phi_r phi_r^T F/lambda_r = 0` in exact arithmetic.

**Half-power width (h3), closed form.** `|H_1|^2 = 1/((lambda_1 - omega^2)^2 + 4 zeta^2 lambda_1 omega^2)`; with `x = (omega /
omega_1)^2` the maximum is at `x = 1 - 2 zeta^2`, value `1/(4 zeta^2 (1 - zeta^2))`; the half-power points solve `(1 - x)^2 + 4
zeta^2 x = 8 zeta^2 (1 - zeta^2)`, `x = 1 - 2 zeta^2 +- 2 zeta sqrt(1 - zeta^2)`, width `omega_1 (sqrt(x_+) - sqrt(x_-))`. For
`zeta = 0.01` (arithmetic 5): `0.020002000700 omega_1`, `1.0e-4` above `2 zeta omega_1`.

### D2. Static correction (residual flexibility)

From the spectral expansion of the inverse (first principles: `K^{-1} = sum_{all r} phi_r phi_r^T / lambda_r` for M-orthonormal
modes of a positive definite `K`, since `Phi^T K Phi = Lambda`, `Phi^T M Phi = I`, so `K = M Phi Lambda Phi^T M` and `K^{-1} =
Phi Lambda^{-1} Phi^T`), the response with all modes is `X = sum_{r <= m} phi_r H_r phi_r^T F + sum_{r > m} phi_r H_r phi_r^T F`.
For the truncated modes `omega^2 << lambda_r` gives `H_r ~ 1/lambda_r` (their damping and inertia neglected), and their sum is
`K^{-1} F - sum_{r <= m} phi_r phi_r^T F / lambda_r`. So, with `m` retained modes,

    X = sum_{r <= m} phi_r H_r(omega) phi_r^T F + ( K^{-1} F - sum_{r <= m} phi_r phi_r^T F / lambda_r ).

This is the mode-acceleration form of the static correction (Craig and Kurdila, Fundamentals of Structural Dynamics, from memory;
the formula is derived above, nothing else is used). The bracket is real-linear in `F`: computed once per load (two solves of the
reduced `K`, one for `F_re` and one for `F_im`) and reused across a sweep. The error of the truncation is `sum_{r > m} phi_r
phi_r^T F (H_r - 1/lambda_r)`, `H_r - 1/lambda_r ~ omega^2/lambda_r^2` for small damping: second order in `omega/omega_{m+1}`.

`K` reduced by the model's zero prescribed equations is factored once by `SkylineLdlt::factor` with the pivot rule of the static
solve (`d_i <= PIVOT_TOLERANCE a_ii` refused); a model with a free rigid motion or on the transformation path
(`mpc::uses_transformation`: linear constraints, rigid sets, elastic supports, cyclic) asks for the static correction and is
refused with key `"static correction"` (a limit of this item; without the correction such models are taken).

### D3. The basis and the solver

`ModalBasis { eigenvalues, modes, dofs }`: `ModalBasis::from_result(&ModalResult)` (any modal result, also of
`mpc::modal_analysis_mpc`, since the modes are expanded to the model's equations), `ModalBasis::new(eigenvalues, modes, dofs)`
(validated: equal counts, each mode of `dofs.n_equations()` finite values, eigenvalues finite; key `"modal basis"`), and
`ModalBasis::all_modes(model, threads)`: every mode of a small model (at most `MAX_DENSE_UNKNOWNS = 600` free unknowns, else key
`"modes"`) by `rcfd_linalg::eigen::generalized_eigen_dense` of the reduced dense `K` and `M` (Cholesky of `M`, Jacobi; works for a
free body, `K` singular), expanded with 0 at prescribed equations; the transformation path and non-zero prescribed values are
refused (key `"constraint"`).

`HarmonicOptions { damping: ModalDamping, static_correction: bool (default true), threads: Option<usize> }`.
`HarmonicSolver::new(model, basis, &options)`: checks the basis against `model.dof_map()` (`FieldLength`, key `"modal basis"`),
the damping, and builds the static correction when asked. `HarmonicLoad::new(re, im)`, `::real(re)`, `::from_model(model)` (the
model's own loads by `loads::nodal_load_vector` as the real part), `::from_harmonic(pattern, &ForceHarmonic)` (`pattern A e^{i
phi}`). `response(&load, omega) -> HarmonicResponse { omega, re, im }` (full vectors, 0 at prescribed equations; `amplitude(eq)`,
`phase(eq)`); `sweep(&load, &omegas)` (the bracket of D2 computed once; each entry has the bits of `response` at that `omega`);
`modal_amplitudes(&load, omega)` (the `Q_r`). `omega` must be finite and `>= 0` (key `"frequency"`); an undamped mode with
`|lambda_r - omega^2| <= 1e-12 lambda_r` (a resonance without damping) is refused with key `"frequency"`. A load of the wrong
length: `FieldLength` key `"harmonic load"`.

Stress: `HarmonicResponse::stress(model) -> HarmonicStress { re, im }` at the integration points of
`recover::IntegrationPointField`, each part `sigma(x_part) - sigma(0)` (the stress is affine in `x`; subtracting the zero field
removes a thermal strain of the model, which is not harmonic). `amplitude(point, component)`, `phase(point, component)`.

### D4. Participation factors and effective masses

For a unit direction `d`, `r_d` the rigid translation (`mass::rigid_motions`, translational components; the vector is
normalised), the full consistent `M` (all equations, the support rows included): `Gamma_r = phi_r^T M r_d` (the modal force of
the base-acceleration load `-M r_d a_g`; `phi_r` is zero at the prescribed equations, so `Gamma_r = phi_{r,f}^T (M_ff r_f + M_fs
r_s)`), effective mass `m_r = Gamma_r^2`, total mass `m = r_d^T M r_d` (the whole model, no constraint). `participation(model,
basis, d, threads) -> Participation { direction, factors, effective_masses, total_mass }`, `fractions()`.

**Completeness (h8), first principles.** With all modes of the free unknowns `sum_r phi_{r,f} phi_{r,f}^T = M_ff^{-1}` (`Phi^T
M_ff Phi = I`, `Phi` square). So `sum_r Gamma_r^2 = g^T M_ff^{-1} g`, `g = (M r_d)_f`. For a model without any constraint (a free
body; the rigid modes are in the basis) this is `r_d^T M M^{-1} M r_d = r_d^T M r_d`, the total (unconstrained) mass: the
literal clause. For a constrained model it is `g^T M_ff^{-1} g`, which the test computes by a dense solve; both are registered
(A8a, A8b).

**Continuum references.** Fixed-free bar, mode `sin(pi x/2L) sqrt(2/(rho A L))`: `Gamma = rho A sqrt(2/(rho A L)) 2L/pi`,
`Gamma^2 = 8 rho A L / pi^2`: fraction `8/pi^2 = 0.8105694691`. Euler-Bernoulli cantilever, mode `phi = cosh bx - cos bx - s
(sinh bx - sin bx)`, `b L = 1.8751040687`, `s = (cosh bL + cos bL)/(sinh bL + sin bL)`: fraction `(int phi)^2/(L int phi^2)`;
recomputed (arithmetic 4, Simpson on 200001 points, `int phi = 2 s/b` checked against the closed form to 1e-10): **0.6130761**
(the stub's 0.613 confirmed); the second mode 0.1883004.

### D5. Force harmonics of a sampled record

`force_harmonics(samples, t0, dt, period, orders) -> ForceSpectrum { mean, periods, harmonics: Vec<ForceHarmonic { order,
omega, amplitude, phase }> }`: samples `f_j = f(t0 + j dt)`, `j = 0 .. N-1`, of a signal of period `T`; the record spans `P = N
dt / T` periods, which must be a whole number: `|P - round(P)| <= 1e-9 max(1, P)` and `round(P) >= 1`, else refused (key `"force
record"`). The harmonic of order `h` is the DFT bin `k = h P`, required below the Nyquist bin (`2 h P < N`, else refused). With
`f(t) = a_0 + sum_h A_h cos(h Omega t + phi_h)`, `Omega = 2 pi / T` (orthogonality of the discrete exponentials over a whole number
of periods, first principles):

    c_h = (2/N) sum_j f_j exp(-2 pi i (k j mod N) / N) = A_h exp(i (phi_h + h Omega t0)),   a_0 = (1/N) sum_j f_j,

exact for a band-limited signal (no harmonic at or above the Nyquist bin). The angle uses the integer `(k j) mod N` so the
twiddle factors carry no growing rounding; `phi_h = arg c_h - h Omega t0` wrapped to `(-pi, pi]`. `ForceHarmonic::complex()` is
`(A cos phi, A sin phi)`, so `Re(A e^{i phi} e^{i h Omega t}) = A cos(h Omega t + phi)`: the load convention of D1. Refused
(key `"force record"`): fewer than 2 samples, a non-finite sample, `dt` or `period` not finite and `> 0`, `t0` not finite,
`orders = 0`.

### D6. The set-ups

Material `E = 2e11`, `nu = 0`, `rho = 7850`, `c = sqrt(E/rho)`.

**Bar B40.** `hex_box([0; 3], [1.0, 0.04, 0.04], [40, 1, 1])` (Hex8); constraints `("all", 1, 0)`, `("all", 2, 0)`, `("xmin",
0, 0)`: 160 free axial unknowns. Tip load `Load::nodal_force("xmax", (0.25, 0, 0))` (`F = 1` N on the four tip nodes), `A =
0.0016`. Modes: `modal_analysis(&b40, &ModalOptions::new(10).tolerance(1e-12).residual_test(Backward))`. The lowest ten modes
are axial modes uniform over a section: a mode with `u_x` varying across the section has `omega^2 >= 12 G/(rho b^2) = 6 c^2/b^2`
(arithmetic 6), `omega >= 61.2 c`, against `(2r - 1) pi c / 2 = 29.8 c` for `r = 10`; checked as C1. For a uniform section motion
with `nu = 0` and the transverse components fixed the Hex8 chain is the 1D linear bar with consistent mass, which arithmetic 1 to
3 compute. Tip equation: node `xmax[0]`, component 0.

**Bar B10.** The same with `[10, 1, 1]` (40 free unknowns), for the transient cross-check (C3) with `ModalBasis::all_modes`.

**Cantilever C40.** `upgrade_to_hex20(hex_box([0; 3], [1.0, 0.025, 0.05], [40, 1, 1]))`, clamped `("xmin", 0..3, 0)`;
`modal_analysis(&c40, &ModalOptions::new(3).tolerance(1e-12).residual_test(Backward))`. The section `0.025 (y) x 0.05 (z)`:
bending in `y` is the lowest mode (`omega ~ h_y`), bending in `z` twice its frequency, so mode 1 is not a degenerate pair;
`L/h_y = 40` keeps the shear and rotary-inertia corrections near `(h/L)^2 ~ 6e-4`. Effective mass in `y`.

**Small models S.** `hex_box([0; 3], [1.0, 0.5, 0.5], [2, 1, 1])` (12 nodes, 36 unknowns): S-free without constraints, S-clamped
with `("xmin", 0..3, 0)` (24 free unknowns); `ModalBasis::all_modes`.

**Record R.** `T = 0.01`, `P = 2`, `N = 96`, `dt = P T / N`, `t0 = 1.3e-3`, `f(t) = 0.7 + 1.0 cos(Omega t + 0.3) + 0.45 cos(2
Omega t - 2.1) + 0.2 cos(3 Omega t + 2.9)`, the samples by that formula at `t_j = t0 + j dt`; `orders = 5` (orders 4 and 5 have
amplitude 0).

### Arithmetic before this registration (disclosed)

Scratch script `f399-scratch/predict.py` (numpy; the 1D consistent-mass linear bar standing in for B40, see D6):

1. B40: discrete `omega_1 = 7.9291740656e3` rad/s, continuum `7.9286645975e3` (`+6.4e-5`).
2. Tip response at `s omega_1` (discrete `omega_1`, `k = s omega_1 / c`), relative to `F tan(kL)/(EAk)`: `s = 0.5`: full discrete
   solve `-9.2e-6`; **10 modes without correction `-1.509e-2`; with the static correction `-1.25e-5`**. `s = 1.5`: full `8.26e-4`;
   10 modes without `4.62e-2`; **with `9.15e-4`** (the discrete dispersion of 40 linear elements, about `5.7 (kh)^2/24`).
3. Effective mass of B40's first mode (full `M`, support coupling included): `0.8105694504`, relative to `8/pi^2` `-2.3e-8`; modes
   2 and 3: `0.0900631`, `0.0324223`; the sum over all 40 modes `0.992783121635` equals `g^T M_ff^{-1} g / m` to 12 digits (the
   completeness of D4; the rest is the support's share).
4. Cantilever (D4): 0.6130760900 (mode 1), 0.1883003611 (mode 2). The 3D model C40 differs by its shear and rotary inertia
   (order `(h_y/L)^2 = 6.3e-4` times a factor of order 1 to 5) and by the clamped face; estimate `|fraction/0.6130761 - 1| <=
   3e-3`.
5. Half-power width, `zeta = 0.01`: `0.020002000700 omega_1`, `(width/(2 zeta omega_1)) - 1 = 1.0004e-4`; the peak at `omega =
   0.999899995 omega_1`.
6. Lowest section-varying (warping) mode of a one-element section, `b = 0.04`: `u_x = +-1` across `y`, `gamma_xy = 2/b`, energy
   `G 4/b^2` against the consistent mass form `rho/3`: `omega^2 = 12 G/(rho b^2) = 6 c^2/b^2`, `omega = 61.2 c` (hand).
7. Bilinear (trapezoidal) warp for C3: Newmark with `beta = 1/4`, `gamma = 1/2` is the trapezoidal rule on `(x, v)` (ADR 0389
   D1), so with the load sampled at the steps its steady state is exactly the continuous transfer function at `i omega_hat`,
   `omega_hat = (2/dt) tan(omega dt/2)` (substitute `z = e^{i omega dt}` in `s = (2/dt)(z - 1)/(z + 1)`, first principles). At
   200 steps per period: `omega_hat/omega - 1 = 8.2255e-5`; the tip response at `0.5 omega_1` moves by about `2 (omega/omega_1)^2
   /(1 - (omega/omega_1)^2) 8.2e-5 = 5.5e-5` relative between `omega` and `omega_hat` (N2).
8. C3 decay: Rayleigh `from_ratio(0.1, omega_1, omega_1)` (`alpha = 0.1 omega_1`, `beta = 0.1/omega_1`), every mode decays at
   least as `exp(-alpha t/2)`; after 39 forcing periods at `0.5 omega_1` (`78 T_1`) `exp(-0.05 omega_1 78 T_1) = exp(-24.5) = 2e-11`.
   The highest mode of B10 (`omega ~ 4.4e5`, overdamped) has its fast root at `|s| = beta omega^2 ~ 2.4e6`, `|s| dt/2 ~ 9.5`, so
   the trapezoidal factor `(|s| dt/2 - 1)/(|s| dt/2 + 1) = 0.81` per step: gone after the run.
9. Record R: the DFT of a band-limited signal is exact; rounding about `eps sqrt(N) max|f|` `~ 2e-15` on `c_h`, phases `~ 1e-14`
   rad for amplitudes `>= 0.2`.

## Registered checks

All in `crates/rcfd-fem/tests/si_harmonic.rs`, release profile, run once (`CARGO_BUILD_JOBS=4 cargo test -p rcfd-fem --release
--test si_harmonic -- --nocapture --test-threads=1`). Relative differences are `|x - y|/|y|`.

### A. The acceptance

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| A1 | h1 | B40, 10 modes, `Ratio(0.01)`, static correction, `F = M phi_1` (full `M`, real), `omega = sqrt(lambda_1)`: at the tip `abs(abs(X_tip) / (abs(x_s,tip)/(2 zeta)) - 1)`, `x_s` from `solve_static_with` with the nodal forces of `F` | <= 1e-8 | ~1e-11 (eigen residual) |
| A1v | h1 | the same over every equation: `max_j abs(X_j - x_s,j/(2 i zeta)) / (max_j abs(x_s,j)/(2 zeta))` | <= 1e-8 | ~1e-11 |
| A2 | h2 | the same run: `abs(arg(x_s,tip) - arg(X_tip) - pi/2)` (wrapped) | <= 1e-6 rad | ~1e-12 |
| A3 | h3 | the same solver and load: `abs(X_tip(omega))`, peak by golden section on `[0.99, 1.01] omega_1` (200 iterations), the two half-power points by bisection (200 iterations) on `[0.98 omega_1, omega_peak]` and `[omega_peak, 1.02 omega_1]`: `abs(width/(2 zeta omega_1) - 1)` | <= 0.01 | 1.0e-4 (arith. 5) |
| A4 | h4 | B40, 10 modes, `Ratio(0)`, static correction, tip load: `abs(X_tip / (F tan(kL)/(EAk)) - 1)` at `s = 0.5` and `1.5`, `omega = s sqrt(lambda_1)`, `k = omega/c` | each <= 0.005 | 1.25e-5, 9.15e-4 (arith. 2) |
| A5 | h5 | the same without the static correction at `s = 0.5` | > 0.01 | 1.509e-2 |
| A6 | h6 | B40, the 10 modes, direction `x`: `abs(m_1/m / (8/pi^2) - 1)` | <= 0.005 | 2.3e-8 (arith. 3) |
| A7 | h7 | C40, direction `y`: `abs(m_1/m / 0.6130761 - 1)` | <= 0.01 | <= 3e-3 (arith. 4) |
| A8a | h8 | S-free, all 36 modes, directions `x`, `y`, `z`: `abs(sum_r m_r / m - 1)` | each <= 1e-10 | ~1e-14 |
| A8b | h8 | S-clamped, all 24 modes, `x`, `y`, `z`: `abs(sum_r m_r / (g^T M_ff^{-1} g) - 1)`, the reference by a dense Cholesky solve in the test | each <= 1e-10 | ~1e-14 |
| A9 | h9 | record R, orders 1 to 5: `max_h abs(A_h - A_h,exact)` (orders 4, 5 exact 0) and `abs(a_0 - 0.7)`; `max_{h <= 3} abs(phi_h - phi_h,exact)` (wrapped) | <= 1e-12 each | ~2e-15, ~1e-14 |
| A10 | h10 | R's samples with `period` such that `N dt = 2.5 T` and `N dt = (2 + 1e-6) T`: refused, key `"force record"`; with `N dt = 2 (1 + 1e-13) T` accepted | as stated | D5 |

### C. Companions and further checks

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| C1 | B40: each of the 10 modes uniform over each section: `max abs(u_x,a - u_x,b)` over the four nodes of a section, divided by `max abs(phi_r)` | <= 1e-6 | ~1e-12 (arith. 6) |
| C2 | B40, A1's solver with `F` the tip load: `omega = 0`: `max abs(X_re - x_s)/max abs(x_s)` against `solve_static`; `X_im` all exactly 0; stress `sigma_xx` of the real part against the static stress (`IntegrationPointField::compute`): `max abs / max abs` | <= 1e-10; bits 0; <= 1e-10 | ~1e-13 |
| C3 | B10, `ModalBasis::all_modes`, `ModalDamping::Rayleigh` of `from_ratio(0.1, omega_1, omega_1)`, no static correction, tip load times `cos(omega t)`, `omega = 0.5 omega_1`, against `transient::Integrator` (average acceleration, the same Rayleigh damping, `dt = 2 pi/(200 omega)`, a table with a point at every `n dt`, from rest, 8000 steps): the tip samples of steps 7800 to 7999 analysed by `force_harmonics` (`t0 = 7800 dt`, one order): amplitude against `abs(X_tip(omega_hat))` relative, phase against `arg X_tip(omega_hat)` | <= 1e-8; <= 1e-8 rad | ~2e-11 (arith. 8) |
| C4 | B40, A4's solver: `sweep` over `[0.2, 0.5, 1.5, 2.5] omega_1` equals `response` at each, bits | bit-equal | D3 |
| C5 | B40 built with `threads` 1 and 8 (modes, solver, participation): A4's response and A6's factors | bit-equal | the assembly is bit-identical |
| C6 | refusals, each with its key: `omega` of -1, NaN, inf (`"frequency"`); `Ratio(0)` at `omega = sqrt(lambda_3)` (`"frequency"`); `Ratio(-0.1)`, `Ratio(NaN)`, `PerMode` of 9 values for 10 modes (`"damping"`); a load of the wrong length (`"harmonic load"`); a basis of B10 for B40 (`"modal basis"`); the static correction on S-free (`"static correction"`) and on a model with a nodal spring (`"static correction"`); `all_modes` of B40's mesh with 3 components free (`hex_box [200, 1, 1]`, 2412 free unknowns, `"modes"`); `force_harmonics` with 1 sample, a NaN sample, `dt = 0`, `period = -1`, `orders = 0`, and order 24 of R (`2 h P = 96 = N`, `"force record"`) | each refused as stated | D3, D5 |
| C7 | B40: the sum of the effective-mass fractions in `x` over the 10 modes (truncated, so below the complete sum `g^T M_ff^{-1} g / m = 0.992783`, arith. 3) | in `[0.95, 0.99279]` | 0.973 (`0.9928 - (8/pi^2) sum_{r > 10} 1/(2r - 1)^2`) |
| C8 | B40, A4's solver at `s = 0.5`, a complex load `F_re + i F_im` (`F_re` the tip load, `F_im` = 0.5 times the tip load on node `xmax[1]` only): against `response(F_re) + i response(F_im as a real load)` (linearity in the imaginary part): `max abs difference / max abs` | <= 1e-14 | ~1e-16 |

### N. Negative controls

| Id | Control | Expected |
| --- | --- | --- |
| N1 | A4's measure without the static correction at `s = 1.5` | `> 0.005` (4.62e-2: the A4 check discriminates at 1.5 too) |
| N2 | C3's amplitude measure against `abs(X_tip(omega))` (the unwarped frequency) | `> 1e-6` (5.5e-5, arith. 7: C3 can see the warp) |
| N3 | A8b with the mode of the largest `m_r` left out of the sum | `> 1e-3` |
| N4 | A9 with the record analysed as if `t0 = 0` (the phase reference dropped) | phases off by `> 1e-3` |

### S. Existing tests

| Id | Measure | Limit |
| --- | --- | --- |
| S1 | `git diff main --stat -- crates` lists only `harmonic.rs` and `tests/si_harmonic.rs`; `cargo test -p rcfd-fem --release` (every test of the crate); `cargo check -p rcfd-post --tests` | 0 failed (no shared solver file touched, so no baseline-hash run is owed) |

### Mutations (scratch copy of `harmonic.rs`, not committed)

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| Mu1 | damping term `2 zeta_r omega omega` instead of `2 zeta_r omega_r omega` (and for Rayleigh `(alpha + beta lambda_r) omega^2/omega_r`) | C3 |
| Mu2 | static correction `+ sum phi phi^T F / lambda` (sign) | A4, C2 |
| Mu3 | participation `phi_r^T r` (no `M`) | A6, A7, A8 |
| Mu4 | DFT scale `1/N` for the harmonics | A9 |
| Mu5 | the whole-period test removed | A10 |
| Mu6 | the phase reference `h Omega t0` dropped | A9 |
| Mu7 | `sweep` reuses the first frequency's modal factors | C4 |
| Mu8 | the imaginary part of the load ignored in the static correction | C8 |

## Decision rule

- **done**: every A check met (A1 to A10) and every C and S check met, the negative controls as expected.
- **partial**: any A check missed with a diagnosed cause, kept as a `*_known_miss` record; or a C or S check missed.
- **todo**: the checks could not be run.

## Limits of this registration

- Classical (modal) damping only: no complex modes, no non-proportional damping, no direct (full) harmonic solve (ADR 0302,
  advanced tier). Every response level scales with the assumed damping ratio; no ratio for a moulded plastic blade is in the
  repository.
- The static correction neglects the inertia and damping of the truncated modes (second order in `omega/omega_{m+1}`); it is
  refused for a free body (no inertia relief) and on the transformation path (linear constraints, rigid sets, supports, cyclic).
  Without the correction those bases are taken (any `ModalResult`).
- The pre-stressed modes of ADR 0299 (F-395) can be passed as a `ModalBasis`; the static correction then uses the elastic `K`
  only (no geometric stiffness), so with pre-stress it is not consistent with the modes; not checked here.
- `ModalBasis::all_modes` is dense (at most 600 free unknowns).
- Force harmonics: one channel per call, uniformly sampled, whole periods only (no windowing, no leakage correction), orders
  below the Nyquist bin.
- Stress amplitudes are per component at integration points (the real and imaginary parts); a von Mises or principal amplitude
  of a harmonic stress (not a sinusoid in general) is not offered.
