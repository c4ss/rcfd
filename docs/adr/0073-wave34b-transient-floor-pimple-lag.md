# ADR 0073: Wave 34B, the Taylor-Green BDF2 order floor (momentum-interpolation commutation) and the PIMPLE one-corrector pressure-lag instability

Status: accepted (registration). Everything under "Registered" was written BEFORE any code of this wave was changed and BEFORE any new run. Limits are
not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion moves to an ignored `*_known_miss` test with
the registered limit intact; a defective set-up gets a NEW corrected test and the old one stays. Every fix is opt-in; the default stays bit-identical
(`w33b_default_bits` fingerprints) and `crates/rcfd-api/tests/baseline_hashes.rs` is never re-recorded. Clean-room: first principles and published
literature only.

Parent: ADR 0069 (wave 33B: harnesses `w33b_transient_order.rs`, `w33b_stability.rs`, `w33b_default_bits.rs`; misses O1-O4, O6, O7, PIMPLE-2 at Co_t = 4).

## What was known before this registration (and how)

* Numbers of ADR 0069 and its run logs (quoted, not re-measured): TG N = 32 fine pair (0.025, 0.0125), errors against the same-scheme dt = 1/1280 reference:
  PISO 4.378e-5 / 1.420e-5; PIMPLE n_outer 4 (X5) 2.688e-5 / 1.315e-5 (order 1.031); ddt_correction off (X3) 3.411e-4 / 1.887e-4. MMS PISO with n_outer 4 (X5):
  2.326e-5 / 1.853e-5 (ratio 1.26). X6 PISO: N = 16: 2.793e-4, 1.181e-4, 5.760e-5; N = 32: 2.010e-4, 4.378e-5, 1.420e-5; N = 64: 2.632e-4, 5.391e-5, 1.053e-5
  (dt 0.05, 0.025, 0.0125). Fractional: N = 32 3.883e-4 / 1.989e-4 (order 0.965). Stability: the full required set and scan of ADR 0069 (PIMPLE-2 unstable at
  Co_t = 4 at Re = 100 on both meshes and schemes; PIMPLE-4 unstable at Co_t = 8 at Re = 100 except graded BDF2; PIMPLE-3 (X8b) uniform Euler unstable at Co_t = 4,
  the other three X8b cases stable; X8a (two correctors) stable).
* No new run of any kind was made before this registration. The analysis below is a reading of the code plus algebra. Because the cavity outcomes above were
  known when the pressure-lag model was written, the agreement of the model with them is a POSTDICTION and is reported as such; the falsifiable parts are the
  new runs registered in Part B.
* Literature read in this session: Komen, Hopman, Frederix, Trias, Verstappen (2021), "A symmetry-preserving second-order time-accurate PISO-based method",
  Computers & Fluids 225, 104979, arXiv:2010.03931 (read through an HTML rendering of the arXiv version): with the total pressure in the projection the
  kinetic-energy error of the compact-Laplacian collocated projection is O(dt h^2) and the observed temporal order drops to about one; with the pressure-correction
  (increment) form it is O(dt^2 h^2). Choi (1999, Numer. Heat Transfer A 36, 545) and Tukovic, Peric, Jasak (2018, Computers & Fluids 166, 78) could NOT be
  obtained in full; what is used of them below (Choi: the old FACE velocity enters the face equation multiplied by the interpolated coefficient, with the transient
  term excluded from the interpolated cell quantity) is from memory and is UNVERIFIED; the derivation below does not depend on it.

## Reading of the code (no run)

### R1. The deferred-correction "central" scheme makes the momentum diagonal spatially variable

`convection()` assembles every non-upwind scheme as implicit UPWIND plus an explicit deferred correction (`convection.rs`, lines 182-249). The diagonal is
therefore `a_P = c0 V/dt + a_diff + sum_out F` and the convective part `sum_out F` varies from cell to cell (on the Taylor-Green vortex it is of the same size as
the viscous part at N = 32). Hence `rAU = V/a_P` is NOT uniform, also on a uniform periodic mesh. (ADR 0069 assumed a uniform `a_P` for TG when it reasoned
about the Choi term.)

### R2. The transient Rhie-Chow carry-over of the iterative path is consistent only for a uniform rAU

`HbyA = H/a_P` contains the old-level part `rAU (c1 u^n - c2 u^{n-1})/dt` (the BDF2/Euler source is in `b`). The face flux is
`phiHbyA = interp(HbyA).S + (gamma/dt) [c1 (phi^n - interp(u^n).S) - c2 (phi^{n-1} - interp(u^{n-1}).S)]`, `gamma = interp(rAU)`. Its old-level part is

    (1/dt) [c1 interp(rAU u^n) - c2 interp(rAU u^{n-1})].S + (gamma/dt) [c1 (phi^n - interp(u^n).S) - c2 (...)]
      = (gamma/dt) (c1 phi^n - c2 phi^{n-1})  +  (1/dt) [c1 C(u^n) - c2 C(u^{n-1})],   C(x) = interp(rAU x).S - interp(rAU) interp(x).S.

The commutator `C` vanishes when `rAU` is uniform; otherwise, for linear interpolation, `C = w (1 - w) (rAU_N - rAU_P)(x_N - x_P).S`. With `rAU = dt/(c0 + a dt)`
(`a` = non-temporal diagonal per volume) `rAU_N - rAU_P = -dt^2 (a_N - a_P)/((c0 + a dt)^2)` to leading order, so `(c1/dt) C(u^n)` is
`O(dt (a_N - a_P)(u_N - u_P)/(c0 + a dt)^2)`: FIRST order in dt, proportional to products of cell-to-cell differences (h^2 for smooth fields), and it saturates
(falls) once `a dt` exceeds `c0`. It is not removed by more correctors or outer iterations (it is part of the converged discrete equations). This matches the
qualitative facts of ADR 0069: the floor survives X1-X5, converging the outer loop (X5) makes the order WORSE (it removes the dt^2 splitting error that masked it),
it is h-dependent and strongest on the coarse mesh (X6), and the MMS X5 ratio 1.26 is in the range expected from a saturating term at `a dt ~ 5` (MMS N = 32,
nu = 0.05: a ~ 200). This is hypothesis **H-COMM**.

Consequence (algebra): if instead the carry-over is formed as `(c1/dt)(gamma phi^n - interp(rAU u^n).S) - (c2/dt)(gamma phi^{n-1} - interp(rAU u^{n-1}).S)`, the
old-level part of `phiHbyA` is exactly `(gamma/dt)(c1 phi^n - c2 phi^{n-1})` (the old face flux times the interpolated coefficient, the transient term excluded from
the interpolated cell quantity). The remaining dt-dependence is the interpolation `interp(rAU H')` of the non-transient part `H'`, whose commutator is
`O(dt^2 ...)`. This is fix candidate **F-C** below.

### R3. Translationally periodic faces get no carry-over at all

On coupled faces `phiHbyA` is the distance-weighted mean of the owner and partner `HbyA` (`boundary_hby_a`) and no carry-over is added (the loop of the Choi term
runs over internal faces). The face defect there is `gamma R(p)` every step, `R(p) = interp(grad p).S - snGrad(p)|S|`, i.e. the dt-dependent Rhie-Chow term of
ADR 0069 X3 on the seam rows. On the Taylor-Green vortex of ADR 0069 the seams x = 0 and y = 0 are symmetry lines: `u.n = 0` and `R(p) = 0` there (p is even about
them), so the defect is zero by symmetry. ADR 0069's F1 measurement ("defect 0 to 6e-16 on every seam face") is explained by this and does NOT show that the seams
are harmless in general; it was not a test of H-seam. A phase-shifted vortex (TG-S below) is.

### R4. The fractional path uses the total-pressure face term without a carry-over

`phi* = interp(u*).S + dte (interp(grad p^n).S - snGrad(p^n)|S|)` and the projection with the increment `q` give `phi^{n+1} - interp(u^{n+1}).S = dte R(p^{n+1})`
every step: the total-pressure collocated form whose error Komen et al. (2021) analyse as O(dt h^2). `rAU = dte` is uniform there, so R2 does not apply. Dropping the
`dte R(p^n)` predictor face term gives `phi^{n+1} - interp(u^{n+1}).S = dte R(q)` with `q = O(dt)`: their pressure-correction form, O(dt^2 h^2). ADR 0043 reported
(exploratory, unregistered) order 2.00 at every pair with that term off, but never measured the error against the exact solution. This is hypothesis **H-K**,
fix candidate **F-K**.

### R5. One pressure corrector propagates the pressure lag with the factor -(off-diagonal part)/(temporal diagonal)

Linear Stokes model, one Fourier mode with full-operator symbol `lambda` (per volume) and diagonal part `lambda_d` (per volume, `lambda_d = sum |a_nb|/V` for the
diffusion stencil), temporal diagonal `c0/dt`. The predictor solves with the lagged pressure error `e`; its irrotational part is `-G e/(c0/dt + lambda)`; the
corrector builds `HbyA = (b - N u*)/a_P` with the off-diagonal symbol `N = lambda - lambda_d`, so the new pressure error is `kappa e`,
`kappa = (lambda - lambda_d)/(c0/dt + lambda)`; for smooth modes (`lambda << lambda_d`) `kappa = -D`, `D = lambda_d dt/c0` (the diagonal-part diffusion number). A
second corrector starts from a projected (irrotational part removed) velocity and returns zero lag error in this model. With implicit under-relaxation `alpha_u`
(diagonal `a_P/alpha_u`) the non-final outer factor is `kappa_r = -D/((1 + D)/alpha_u - D)` and the pressure update is relaxed with `alpha_p`. Per step, with one
corrector and `n_outer` outer iterations:

    G = kappa * [(1 - alpha_p) + alpha_p kappa_r]^(n_outer - 1),        (instability when |G| > 1)

`alpha_u = 0.8`, `alpha_p = 0.5` (the PimpleSettings::pimple defaults): `|G| = 1` at D* = 1.000 (n_outer 1), 3.333 (2), 4.650 (3), 5.654 (4). On the uniform 32 x 32
cavity at rest (`lambda_d = 4 nu/h^2`, Re = 100: `D = 1.28 Co_t / c0`) this gives onset Co_t* = 2.60 (PIMPLE-2 Euler), 3.91 (PIMPLE-2 BDF2), 3.63 / 5.45 (PIMPLE-3
Euler / BDF2), 4.42 / 6.63 (PIMPLE-4 Euler / BDF2). Postdiction (data known, see above): all fourteen Re = 100 uniform outcomes of ADR 0069 for PIMPLE-2/3/4 at the
measured Co_t agree with the sign of `|G| - 1` (e.g. PIMPLE-2 BDF2 at Co_t = 4: G = 1.06, the slowest blow-up of its group). Hypothesis **H-LAG**. Convection adds
`sum_out F/V` to `lambda_d` (outside the model's Stokes derivation; Re = 1000 outcomes are reported, not predicted).

## Registered

### Common

Set-ups, tolerances (`tol_rel` 1e-12, `tol_abs` 1e-14, `max_iter` 5000), the error measure (relative L2 of the cell velocity at t = 1 against a same-scheme reference
with dt_ref = 1/1280 on the same mesh), the pairs (fine 0.025/0.0125 binding, coarse 0.1/0.05) and the paths PISO, PIMPLE-C, PIMPLE-4, FRAC are those of ADR 0069.
New set-up **TG-S**: the Taylor-Green vortex of ADR 0069 shifted by `s = 0.6` in x and y (`u = (sin(x+s) cos(y+s), -cos(x+s) sin(y+s)) e^{-2 nu t}`,
`p = (cos 2(x+s) + cos 2(y+s))/4 e^{-4 nu t}`), same mesh, nu = 0.1, central, t_end = 1. Exact error = relative L2 against the exact cell-centre velocity; pressure
error = relative L2 of the cell pressure against the exact one after removal of the volume mean of both.

### Part A: the order floor

**A1. Registered decomposition (prints, fitted; no limit).** TG, BDF2, paths PISO and PIMPLE-4 and FRAC (defaults), N in {16, 24, 32, 48, 64},
dt in {0.05, 0.025, 0.0125, 0.00625}, self-referenced errors `e(N, dt)` (reference dt_ref = 1/1280 per N). Fit **M** on all 20 points:
`e = a dt^p + c h^s dt^r`, `h = 2 pi/N`, least squares on `log e`; `a, c >= 0` by an inner non-negative linear least-squares (weights 1/e) for every `(p, s, r)` of the grid
p in [1, 3] step 0.02, s in [0, 4] step 0.05, r in [0.5, 2.5] step 0.02. Printed: a, p, c, s, r, the rms of `log(model/e)`, and the share of the c-term at (N = 32, dt = 0.0125).
Also per N the two-term fit `e = A_N dt^2 + B_N dt` (four points, weights 1/e), and the spatial error `e_s(N)` of the reference runs against the exact solution with
its fitted exponent `e_s = b h^q` (independent space and time refinement). Reading rule: an O(dt h^s) floor term is **present** if its share at (32, 0.0125) is >= 30 %
and `r <= 1.5`; it is **identified as O(dt h^2)** if in addition `|s - 2| <= 0.5` and `|r - 1| <= 0.35`. A reference-adequacy row **A0** (prints): at N = 16 and 32 the
PISO and PIMPLE-4 errors at dt = 0.0125 recomputed against dt_ref = 1/2560; adequate if they differ from the 1/1280 values by <= 5 %.

**A2. Falsifier of H-COMM, run BEFORE F-C is written (scratch tree only, prints).** Y1 "uniform-rate split": on the iterative path replace `a_P` in `HbyA`, `rAU` and
`gamma` by the spatially uniform `a_bar = V_c max_k(a_P,k/V_k)` with the consistent split `HbyA = (H + (a_bar - a_P) u)/a_bar` (the F3 algebra with a uniform
diagonal; with it `rAU` is uniform and `C = 0` exactly). Run on TG PIMPLE-4 and TG PISO at N = 16, 32, 64 (fine pair and the A1 dt set at N = 32). Reading rule:
H-COMM is **falsified** if with Y1 the PIMPLE-4 fine-pair order at N = 16 or at N = 32 stays below 1.9 AND the share of the fitted O(dt) part (two-term fit, N = 32) is
not reduced by at least a factor 3 against the default; otherwise H-COMM **stands** and F-C is implemented. Y2 (prints): the norm of the commutator flux
`(1/dt)[c1 C(u^n) - c2 C(u^{n-1})]` against the norm of the carried defect term and of `gamma R(p)` at the last step, TG PISO N = 32, dt in {0.025, 0.0125, 1/1280}.

**A3. Fix F-C `PimpleSettings::ddt_correction_consistent` (default false; iterative path; non-ALE).** The carry-over becomes
`(c1/dt)(gamma phi^n - interp(rAU u^n).S) - (c2/dt)(gamma phi^{n-1} - interp(rAU u^{n-1}).S)` on internal faces AND on translationally periodic coupled faces
(owner/partner distance weights, `gamma` of the face = the coupled mean used by the pressure equation; rotationally periodic faces unchanged, stated), with the
`rAU` of the current outer iteration. Requires `ddt_correction = true`. Acceptance (all, with F-C on; band [1.9, 2.3]):
* **C-O1** TG PISO fine and coarse; **C-O2** TG PIMPLE-C fine; **C-O3** TG PIMPLE-4 fine; **C-O7** TG PISO variable step (ADR 0069 O7 sequence) fine;
  **C-S1** TG-S PISO fine and **C-S2** TG-S PIMPLE-4 fine. (The coarse pair of PIMPLE-C and the MMS rows carry the PISO/PIMPLE splitting error that ADR 0069 found
  to be pre-asymptotic; they are run and reported, **C-O6r**, not part of the acceptance.)
* **C-O9** joint refinement against the exact solution (ADR 0069 O9 set-up) with F-C: both orders >= 1.9.
* **No-harm N1**: exact velocity error AND pressure error with F-C <= 1.05 x the default at dt in {0.05, 0.025, 0.0125}, N = 32, for TG PISO, TG-S PISO, MMS PISO,
  MODE PISO (12 velocity and 12 pressure comparisons; MODE pressure is zero and is skipped).
* **N2**: with every new option off the five fingerprints of `w33b_default_bits` are identical (zero differing bits); `baseline_hashes` unchanged.
* **N3** (decomposition with F-C, prints): A1 rerun for PISO and PIMPLE-4 with F-C on N in {16, 32, 64}; the reading rule of A1 applied.
If C-O1/C-O3 fail with Y1 having passed, the remaining floor is reported with the A1 decomposition of the F-C runs; no second fix is tried in this wave.

**A4. Falsifier of H-K and fix F-K `PimpleSettings::fractional_increment_flux` (default false; fractional path; incremental form only).** The predictor face term
`dte (interp(grad p^n).S - snGrad(p^n)|S|)` is not added, so `phi* = interp(u*).S`. Run first (diagnostic): FRAC TG N = 32, fine and coarse pairs, and the A1
dt set at N = 16, 32, 64. Reading rule: H-K is **falsified** if the fine-pair order stays below 1.9 with F-K. Acceptance of F-K: **C-O4** FRAC TG fine and coarse in
[1.9, 2.3]; **C-O4S** FRAC TG-S fine in [1.9, 2.3]; **No-harm N4**: exact velocity and pressure errors with F-K <= 1.05 x the default at dt in {0.05, 0.025, 0.0125} on
TG, TG-S and MODE (N = 32), AND a checkerboard check on the lid cavity (16 x 16, Re = 100, linear upwind, FRAC BDF2, dt = 0.01, t = 5): the cell-to-cell
checkerboard indicator `chi = max |p_P - (mean of the 4 face neighbours' p)| / (max p - min p)` with F-K <= 1.5 x its default value; and the cavity velocity with F-K
differs from the default by <= 2 % of the lid speed (max over cells). F-K is kept only if all hold (ADR 0069's F2 lesson: an order gain that moves the converged
solution away from the exact one is rejected).

**A5. Cylinder (only if F-C is accepted; ONE run).** `validation_cylinder_re100_w33b.rs` set-up with `time_scheme = Bdf2` and F-C on, run exactly once. Rule:
F-C does no harm on the cylinder if K1b (St in [0.16005, 0.16995]), K2b (C_D in [1.2883, 1.3957]), K4 (>= 9 crossings) and T1 (`|St - 0.16536| <= 0.001974` and
`|C_D - 1.3437| <= 0.0101`) all still hold. The St and C_D shift against the wave-33B BDF2 run (St 0.16480, C_D 1.35247) is reported whatever it is. A failure keeps
F-C opt-in with the cylinder result stated as a caveat; no second run, no other set-up.

### Part B: the PIMPLE one-corrector instability

**B1. Model (fixed above, R5).** Thresholds D* and Co_t* as stated.

**B2. Verification on the actual solver (new runs, prints and limits).** Periodic unit square 32 x 32, nu chosen so that `D = (4 nu/h^2) dt / c0` takes the values
{1, 2, 3, 4, 6} at dt = 0.01 (one nu per D), fluid at rest, initial pressure `p0 = 1e-6 cos(2 pi x) cos(2 pi y)`, linear upwind, PIMPLE-2 and PIMPLE-4 (one corrector,
alpha_u 0.8, alpha_p 0.5), Euler and BDF2 (BDF2: D uses c0 = 1.5), 12 steps. Measured growth factor `g = (||p_12|| / ||p_6||)^(1/6)` (mean removed). Model value: the
exact-symbol G of R5 with `lambda` of this mode (`lambda = nu (4/h^2)(2 sin^2(pi h))`, `lambda_d = 4 nu/h^2`). Limits: **B2a** for every case with model
`|G| >= 1.15` the run grows (`g > 1`) and `|g - |G|| <= 0.10 |G|`; **B2b** for every case with `|G| <= 0.85` the run does not grow (`g < 1`). Cases with `|G|` in
(0.85, 1.15) are reported only.

**B3. New cavity predictions (uniform 32 x 32, Re = 100, registered harness of ADR 0069, one run each).** Predicted STABLE: PIMPLE-2 Euler Co_t 2.3 (G = 0.71),
PIMPLE-2 BDF2 Co_t 3.4 (0.68), PIMPLE-4 BDF2 Co_t 6.0 (0.69). Predicted UNSTABLE: PIMPLE-2 Euler Co_t 3.0 (1.42), PIMPLE-2 BDF2 Co_t 4.6 (1.49), PIMPLE-4 Euler
Co_t 5.0 (1.55), PIMPLE-4 BDF2 Co_t 7.5 (1.55). Limit: all seven classifications correct (stability criteria of ADR 0069). A wrong one is a miss of the model.

**B4. Run-time check `PimpleSettings::pressure_lag_check: Option<LagCheck>` (default None).** `LagCheck { every, iterations, action: Warn | Raise }`: at step 1 and
every `every` steps, the spectral radius `rho` of the linearised pressure-lag map of one step is estimated by power iteration (`iterations`, default 20; estimate
`(||e_k|| / ||e_{k/2}||)^(2/k)`, mean removed when the level is not fixed) with the matrices of the final outer iteration of that step frozen: per outer iteration the
momentum solve with the pressure-error gradient (relaxed copy with alpha_u on non-final outer iterations, its relaxation source fed by the velocity error of the previous
outer iteration), then the configured number of correctors on the error (homogeneous pressure conditions, no carry-over, no body force), the alpha_p relaxation on
non-final ones; the velocity error is zero at the step start. `rho >= 1`: `Warn` appends a record (step, rho, cell of the largest eigenvector entry) to
`PimpleSolver::lag_warnings`; `Raise` stops the run through the divergence guard with stage "pressure-lag check", quantity "lag amplification", that cell and rho, and
a message naming the remedy (`n_correctors >= 2` or a smaller dt). The settings rule documented with it: one pressure corrector with a lagged pressure is unstable
once the smooth-mode diffusion number exceeds the D* of R5.
Acceptance:
* **B4a** (B2 set-up): the check's `rho` (step 1) within 10 % of the measured `g` for every B2 case with model `|G| >= 1.15`, and `rho < 1` for every case with `|G| <= 0.85`.
* **B4b** (cavity, ADR 0069 harness, Warn mode, every = 20): all PIMPLE-2 and PIMPLE-4 runs of the required set and the scan (2 meshes x 2 Re x 2 schemes x
  Co_t {0.5, 1, 2, 4, 8, 16}, 96 runs, the stability verdict re-measured in the same run): every UNSTABLE run is warned at or before the step of its first violated
  criterion; at most 4 STABLE runs are warned. The Re = 1000 unstable runs are inside this limit (no exemption).
* **B4c**: PIMPLE-2 with `n_correctors = 2` at Co_t = 4, Re = 100, both meshes and schemes: `rho < 1` at step 1 (the X8 remedy is recognised).
* **B4d** (Raise): uniform Re = 100 PIMPLE-2 Euler Co_t = 4 stops at step 1 with the located message; the state at the stop is finite and max|u| <= 1.5.
* **B4e**: with the check off, fingerprints identical (N2).

### Out of scope

Rotationally periodic faces in F-C; a SIMPLEC-type consistent corrector as a stability fix (not tried in this wave); turbulence-model coupling; the sim.rs / schema keys
(the new options are library-level; the API wiring is the user's decision and stays off).

## Sources

Komen et al. (2021), arXiv:2010.03931 (read: O(dt h^2) for the total-pressure collocated projection, O(dt^2 h^2) for the pressure-correction form, order drop to about
one). Choi (1999); Tukovic, Peric, Jasak (2018); Issa (1986); Rhie and Chow (1983): recalled, UNVERIFIED (not read in this session). Van Doormaal and Raithby (1984)
SIMPLEC: recalled, UNVERIFIED, mentioned only as out of scope. The Fourier model of R5 is derived here from first principles.

## Amendment 1 (written after A0, A1, A2 (Y1, Y2), the F-C acceptance runs and the first F-K runs; BEFORE the code of F-G below exists; nothing above was changed)

### What was run, in order

A0 and Y2, then A1 (PISO, PIMPLE-4, FRAC) together with Y1 (one process), then F-C was written and its acceptance tests (C-O1 to C-S2, C-O9, N1, N3) ran
together with the F-K tests (A4, N4) and the lag tests B2, B3, B4c (one process, still running when this amendment was written). Results so far (all as
registered; no limit was changed):

* A0: the dt = 0.0125 errors change by 2.8 % to 3.6 % with the 1/2560 reference: adequate.
* A1 (N = 16, 24, 32, 48, 64): fit M finds the floor PRESENT on all three paths: PISO c-share 42.6 % (s 2.90, r 0.54), PIMPLE-4 79.4 % (s 2.65, r 0.82), FRAC
  83.5 % (s 2.50, r 0.84, "identified as O(dt h^2)" by the registered rule). Fine-pair orders PIMPLE-4: 1.005, 0.961, 1.031, 1.719, 2.010 for N = 16 ... 64.
* Y1 (uniform-rate split; the registered falsifier of H-COMM): PIMPLE-4 fine-pair order 2.012 (N = 16), 2.007 (32), 2.007 (64); the O(dt) share of the two-term
  fit at N = 32 falls from 89.7 % to -1.3 %; PISO 2.113, 2.233, 2.326; fit M share 1.4 % (PIMPLE-4) and 0.5 % (PISO), "not present". By the registered rule
  **H-COMM stands**: the spatial variation of rAU is the cause of the iterative-path floor.
* Y2 (TG PISO N = 32): the commutator flux of the old levels is 1.85e-4, 1.11e-4, 8.4e-6 at dt 0.025, 0.0125, 1/1280 (O(dt)), against 2.13e-4, 1.18e-4, 8.2e-6
  for `gamma R(p)`.
* **F-C failed** (rejected, the code is removed): C-O1 0.930 / 1.370, C-O2 0.938, C-O3 0.917, C-O7 0.938, C-S1 0.984 (all MISS), errors 5 times LARGER than the
  default (TG PISO fine pair 2.38e-4 / 1.25e-4 against 4.38e-5 / 1.42e-5); N1 failed (TG velocity error ratio 1.16 to 1.24, TG-S 1.14 to 1.28); N3 floor present
  (share 98.8 %). C-O9 3.628 / 3.197 met.

### Why F-C was the wrong fix (algebra, written down after the F-C runs)

R2 tracked the old-level part of `phiHbyA` alone. What the converged equations constrain is the face defect `d = phi - interp(u).S`, and `interp(u^{n+1})`
contains the same `interp(HbyA)` as the flux. With `u^{n+1} = HbyA - rAU (G p - b)` (`b` the body-force cell value) and
`phi^{n+1} = phiHbyA - gamma snGrad(p)|S| + gamma b_f`:

    default:  d^{n+1} = (gamma/dt)(c1 d^n - c2 d^{n-1}) + gamma R(p) - gamma R_b + C(G p - b),
    F-C:      d^{n+1} = (gamma/dt)(c1 d^n - c2 d^{n-1}) - (1/dt)(c1 C(u^n) - c2 C(u^{n-1})) + gamma R(p) - gamma R_b + C(G p - b),

with `C(x) = interp(rAU x).S - interp(rAU) interp(x).S` and `R_b = b_f - interp(b).S`. The default carry-over is the right one for `d`; the inconsistent
source is `C(G p - b)`, which is O(dt) because `rAU = dt/(c0 + a dt)` has an O(dt) relative spatial variation (Y1 makes it vanish and removes the floor). F-C
ADDS a second O(dt) source of the same size. The textbook momentum interpolation `u_f = interp(u) - gamma (snGrad p - interp(G p))` (Rhie and Chow 1983,
recalled, UNVERIFIED as to the exact published form) differs from the code's `interp(HbyA) - gamma snGrad p` exactly by `C(G p)`.

### F-G `PimpleSettings::rhie_chow_consistent` (registered now; default false; iterative path, non-ALE)

Deviation from the registration: Part A3 said "no second fix is tried in this wave". This amendment overrides that sentence (the deviation is reported), because the
registered falsifier Y1 identified the mechanism and F-C failed on an algebra mistake, not on the mechanism. F-G is specified before it is written or run:

* Every corrector subtracts `C(G p_lag - b)` from `phiHbyA` before the pressure solve, `p_lag` = the cell pressure at the start of that corrector (the one of the
  previous corrector or outer iteration, or of the previous step), `G` the Green-Gauss gradient the corrector uses for the velocity, `b` the effective body-force
  cell value (zero without a body force), `rAU` and `gamma` those of the current outer iteration. Faces: internal faces, and translationally periodic coupled faces
  with the seam weights (distance weights of `boundary_hby_a`, `gamma` the coupled mean).
* On translationally periodic coupled faces the default carry-over `(gamma/dt)[c1 (phi^n - u^n_f.S) - c2 (phi^{n-1} - u^{n-1}_f.S)]` is added as well
  (`u_f` the seam interpolation), because R3 shows those faces carry `d = gamma R(p)` every step otherwise (ADR 0069 F1 revisited; TG cannot test it, TG-S can).
  Rotationally periodic faces: unchanged (stated).
* With the outer iteration converged `p_lag = p^{n+1}` and `d` obeys `(c0 d^{n+1} - c1 d^n + c2 d^{n-1})/dt = R(p) - R_b - a_f d^{n+1}` with `dt/gamma = c0 + a_f dt`;
  the dt-dependence left is in `a_f` (second order in cell-to-cell differences of `a`) and, for PISO, the corrector lag `C(G(p_new - p_lag))`.
* Acceptance: exactly the registered A3 set with F-G in place of F-C (C-O1 fine and coarse, C-O2 fine, C-O3 fine, C-O7 fine, C-S1 fine, C-S2 fine, all in
  [1.9, 2.3]; C-O9 >= 1.9; N1 at 1.05; N2 zero differing bits; N3 prints; C-O6r reported), and the A5 cylinder rule with F-G in place of F-C. The orders of Y1
  were seen before this registration; F-G is a different discretisation (it keeps the variable `rAU` of the cell momentum and corrects the face flux), so its
  numbers are not known.
* If F-G fails, the iterative-path floor stays a miss with its mechanism identified (H-COMM, Y1) and no third fix is tried.

## Amendment 2 (after B2, B3, B4c and the F-K acceptance runs; BEFORE the F-G runs and before B4b; nothing above was changed)

* F-K: C-O4 2.002 / 1.997 and C-O4S 1.997 met, A4 decomposition "floor not present" (fine-pair orders 2.007, 2.002, 1.984 at N = 16, 32, 64), so H-K is NOT
  falsified; but N4 FAILED (TG velocity error 1.17 to 1.21 x the default at dt 0.05 to 0.0125, TG-S 1.17 to 1.21 x; cavity max velocity difference 0.036 > 0.02;
  the checkerboard indicator 0.298 against 0.823 for the default, met). By the registered rule F-K is not kept (code removed); the fractional floor stays a miss with
  its mechanism identified.
* B3: all seven new cavity predictions of the model correct. B2a/B2b met on every row. B4c met (rho 0.133 uniform, 0.206 graded with two correctors).
* **B4a set-up defect.** For the BDF2 rows the registered B4a compares the check's estimate at STEP 1 with the growth measured over steps 6 to 12; step 1 of a BDF2
  run is an implicit Euler step (c0 = 1 instead of 1.5), so the step-1 estimate is that of a different scheme (it equals the Euler estimate at 1.5 D: e.g. BDF2 D = 4
  gives rho 2.702 = the Euler D = 6 value). The registered test is kept, renamed `b2_b4a_periodic_stokes_box_known_miss` (ignored, limits unchanged). New corrected test
  `b2_b4a_step6_corrected`: the same runs with the check at step 1 and every 6 steps, the estimate made at step 6 is compared with the same B2/B4a limits (10 % for
  |G| >= 1.15, rho < 1 for |G| <= 0.85), all rows. Not changed: B4b keeps `every = 20` with the step-1 estimate included (for BDF2 runs the step-1 estimate is the
  Euler one, which is the larger; any false positive this causes is counted).

## Amendment 3 (after the F-G acceptance runs C-O1 to C-S2, C-O9, N1 and N3-PISO; BEFORE the diagnostic Y3 below is written or run; nothing above was changed)

F-G so far (registered limits): C-O1 2.232 / 2.260 met; C-O2 (PIMPLE-C) 1.237 MISS (default 1.223); C-O3 2.011 met; C-O7 2.194 met; C-S1 (TG-S PISO) 0.995 MISS
(errors 6.12e-4 / 3.07e-4, default 6.73e-4 / 3.43e-4); C-S2 (TG-S PIMPLE-4) 2.011 met (default 0.823; the errors equal the TG PIMPLE-4 ones to all printed
digits); C-O9 3.816 / 3.237 met; N1 MISS on one of 24 comparisons (TG-S velocity at dt 0.0125: ratio 1.0504 against the limit 1.05; all pressure ratios
<= 1.003, TG ratios 0.987 to 0.996, MMS <= 1.002); N3 PISO: floor "not present" (share 0.6 %). F-G is therefore NOT accepted by the registered rule, and the
A5 cylinder run (conditional on acceptance) is not made.

The TG-S PISO miss with the converged-outer TG-S row met points at a lag that the outer iteration removes and that only exists where the periodic seam carries flux:
on the first outer iteration of a BDF2 step the convection term takes the periodic partner value of the coupled faces from the boundary coefficients of the
CURRENT velocity (`u^n`), while the convected cells are extrapolated (`(1 + r) u^n - r u^{n-1}`); the partner part of the central face value is then first order
in time (the fractional path extrapolates it, ADR 0043 D1; the iterative path does not). Hypothesis H-SEAM-CONV.

**Y3 (diagnostic, scratch tree only, prints).** On the first outer iteration of a BDF2 step, the boundary coefficients of the coupled faces (convection, Laplacian
and the periodic links) are those of the extrapolated velocity, as in the fractional path. Run: TG-S PISO with F-G, fine pair. Reading rule: H-SEAM-CONV stands if the
fine-pair order is in [1.9, 2.3]; otherwise it is falsified. No fix follows in this wave either way (amendment 1: no third fix); the result is reported.
