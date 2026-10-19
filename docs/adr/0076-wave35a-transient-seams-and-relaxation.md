# ADR 0076: Wave 35A, closing the transient misses of wave 34B: the periodic-seam convection lag (F-S), the consistent momentum interpolation (F-G) re-evaluated with it, the PIMPLE-C relaxation anchor (F-R, conditional) and rotationally periodic seams

Status: accepted (registration). Everything under "Registered" was written BEFORE any solver code of this wave was changed and BEFORE any acceptance or
diagnostic run of this wave. Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion moves to an
ignored `*_known_miss` test with the registered limit intact; a defective set-up gets a NEW corrected test and the old one stays. Every fix is opt-in; the
default stays bit-identical (`w33b_default_bits` fingerprints) and `crates/rcfd-api/tests/baseline_hashes.rs` is never re-recorded. Clean-room: first
principles and published literature only.

Parent: ADR 0073 (wave 34B: F-G `PimpleSettings::rhie_chow_consistent` kept opt-in although not accepted; misses C-O2 1.237, C-S1 0.995, N1 one comparison
at 1.0504; diagnostic Y3 identified H-SEAM-CONV; rotationally periodic faces outside F-G).

## What was known before this registration (and how)

* The numbers of ADR 0073 and its addendum (quoted, not re-measured). In particular: F-G C-O1 2.232 / 2.260, C-O2 (PIMPLE-C) 1.237 (default 1.223), C-O3 2.011,
  C-O7 2.194, C-S1 0.995, C-S2 2.011, C-O9 3.816 / 3.237, N1 TG-S velocity ratio 1.0504 at dt 0.0125; Y3 (F-G plus the extrapolated coupled-face coefficients on the
  first BDF2 outer iteration, scratch code): TG-S PISO fine pair 4.322e-5 / 9.197e-6, order **2.232**. So the expected value of C-S1 with F-G + F-S is known.
* A reading of the code (no run): on the iterative path the coupled-face coefficients of the momentum predictor come from `self.u` (= `u^n` on the first outer
  iteration), while the convected cells are extrapolated, `(1 + r) u^n - r u^{n-1}`. The translational seams move the lagged partner part of the upwind and
  Laplacian terms back into the matrix (`PeriodicLinks`, ADR 0046), so on them the lag is left only in the deferred (central minus upwind) correction; the
  rotationally periodic seams have no implicit links, so the WHOLE partner term (upwind, Laplacian, deferred correction) is lagged to `u^n` there. F-G's
  `seam_faces` skips rotationally periodic patches, so the carry-over and `C(G p - b)` are absent on them.
* PIMPLE-C on TG (`n_outer` 2, `alpha_u = alpha_p = 0.9`, two correctors) is the only registered path with under-relaxation: PIMPLE-4 of the harness inherits
  `alpha = 1` from `PimpleSettings::piso`. The difference between C-O3 (2.011) and C-O2 (1.237) therefore confounds the number of outer iterations with the
  relaxation. The relaxation of the first outer iteration anchors at `u_iter = u^n` (`lk.relax(&mut m, alpha_u, &cells)`) and the pressure relaxation at `p^n`.
  Algebra (no run): the relaxed first outer iterate is `u* ~ u_conv - (1 - alpha_u)(u_conv - u^n)`, an O(dt) perturbation per step; the one remaining outer
  iteration takes its convecting flux and deferred correction from it, so the converged-step error per step is O(dt^2), O(dt) globally (hypothesis **H-RELAX**).
  With the anchor at the extrapolated velocity the perturbation is `(1 - alpha_u)(u_e - u_conv) = O(dt^2)`. The pressure anchor enters only through the
  predictor of the last outer iteration and is reduced by two correctors (factor about `D^2`, R5 of ADR 0073); the model says it is minor. Not measured yet.
* The cylinder of ADR 0069/0073 A5 runs exactly this PIMPLE-C setting (n_outer 2, alpha 0.9, BDF2, linear upwind). `cylinder_ogrid` closes its theta wrap
  conformally (no periodic patch), so F-S cannot change the cylinder; F-G and F-R can.
* **Prediction written before any run (rule c of the wave preamble):** TG has `u.n = 0` on its seams and MMS/MODE have no periodic faces, so F-S leaves C-O1,
  C-O2, C-O3, C-O7, C-O9 and the TG, MMS and MODE rows of N1 at their ADR 0073 F-G values (round-off). In particular C-O2 stays at 1.237 with F-G + F-S, the
  combination of the task's two fixes fails the registered set on C-O2, and the cylinder would not run. This is why F-R and its falsifier Y4 are registered
  below, in addition to the two fixes the task names. "No fourth fix" is registered too (below), so the set cannot grow until it passes.
* **Smoke runs of the new rotational set-ups (default code, all options off, this session, scratch tree; rule d).** Set-ups as registered in Part R below.
  - Eigenvalue of SWIRL `k = 6.393156761621` (RK4 shooting with 40 000 and 80 000 steps agree to all printed digits; largest table difference 1.7e-14).
  - SWIRL 16 x 16, PISO BDF2: errors against the dt = 1/1280 reference 2.1480e-2, 9.4630e-3, 4.3091e-3 at dt 0.05, 0.025, 0.0125 (orders **1.183, 1.135**); against the
    exact solution 2.4487e-2, 1.2656e-2, 7.7675e-3; the reference's exact error 4.1973e-3. rms of `R(p)` on the seam faces 7.2e-5, 3.9e-5, 2.1e-5 (reference 1.6e-6):
    the lag breaks the rotational symmetry of the discrete solution at the seam.
  - SWIRL 16 x 16, PIMPLE-4 (n_outer 4, alpha 1): 8.4743e-4, 2.0126e-4, 4.8528e-5 (orders **2.074, 2.052**); seam `R(p)` 1.3e-7 to 2.9e-9 (reference 1.3e-13) and seam
    face defect `phi - interp(u).S` 1.3e-9 to 1.5e-11: for a converged step the F-G seam terms VANISH by rotational symmetry on SWIRL (the same artefact that made
    ADR 0069's F1 meaningless on TG). SWIRL therefore tests F-S on rotational seams (PISO row) and the rotational F-G parts only as a no-harm check.
  - SECT-M 16 x 16 (non-axisymmetric manufactured flow, below), PISO BDF2: errors against the reference 1.2802e-2, 4.6636e-3, 1.9762e-3, 9.6010e-4 at dt 0.1, 0.05,
    0.025, 0.0125 (orders **1.457, 1.239, 1.041**); PIMPLE-4: 1.3846e-3, 8.1532e-4, 6.5924e-4, 4.5413e-4 (orders **0.764, 0.307, 0.538**); exact error of the
    references 1.52e-2 (spatial, one azimuthal wavelength over 16 cells); seam defect rms 1.2e-6 to 2.4e-6 and seam `R(p)` 5e-5 to 3e-4, i.e. NOT zero: SECT-M
    tests the rotational F-G parts.
  - Exactness check of SECT-M: the exact velocity and source rotated by 60 degrees equal the values at the partner position to 1e-9 (source by fourth-order
    finite differences of the analytic velocity; its accuracy is about 1e-9, five orders below the discretisation errors above).
  - Spatial convergence (PIMPLE-4, dt = 1/640, n_theta = n_r = 8, 16, 32, 64): SWIRL exact errors 1.5453e-2, 4.0220e-3, 1.0208e-3, 2.5697e-4 (orders 1.942, 1.978, 1.990);
    SECT-M 6.2960e-2, 1.5210e-2, 4.1395e-3, 1.0928e-3 (orders 2.049, **1.878**, 1.921). A joint-refinement limit of 1.9 on SECT-M would be failed by the spatial error
    alone at 16 -> 32; its limit below is set from this study (rule a).
* Literature read in this session: none new. References of ADR 0073 apply with their verification status.

## The fixes (specified at code level before they are written)

**F-S `PimpleSettings::seam_extrapolation` (default false; iterative path, non-ALE).** On the first outer iteration of a BDF2 step (the existing condition
`outer == 0 && c2 != 0`, which already extrapolates the convecting flux and the convected cells), the boundary coefficients `b` and `d` of every coupled face
(translational AND rotational) are taken from a clone of `u` whose cell values are the extrapolated `(1 + r) u^n - r u^{n-1}` (exactly the code of the
fractional path, `fractional.rs` lines 106-135; `boundary_coeffs` rotates the partner vector on rotational patches). The lagged values that `PeriodicLinks::fix_rhs`
removes from the right-hand side are then the extrapolated cells as well (otherwise the translational seams get a new O(dt) mismatch). Nothing else changes
(later outer iterations, `ub`, the pressure conditions). Consistency check: F-G + F-S reproduces Y3 on C-S1 (4.322e-5 / 9.197e-6, 2.232) to three digits.

**F-G extended to rotationally periodic seams (same key `rhie_chow_consistent`).** `seam_faces` no longer skips patches with `periodic_rot`; it returns the
rotation of the patch with the face. In `rc_commutator` the partner vector `x[q]` and in `seam_defects` the partner velocities of both old levels are rotated
with that matrix (as `boundary_hby_a` does) before the interpolation. The scalar `rAU`, the seam weights and the face `gamma` (the coupled mean) are unchanged.

**F-R `PimpleSettings::relax_extrapolated` (default false; iterative path, non-ALE; written only if Y4 says so, see below).** On the first outer iteration of a
BDF2 step (`outer == 0 && c2 != 0`) the implicit under-relaxation of the momentum predictor anchors at the extrapolated velocity
`(1 + r) u^n - r u^{n-1}` instead of `u^n`. The pressure relaxation is unchanged (the solver keeps no pressure history; anchoring the pressure would add state
to `PimpleState`, snapshots and restarts and is not attempted). Euler steps and later outer iterations are unchanged.

## Registered

### Common

Set-ups, tolerances, error measures, pairs (fine 0.025/0.0125 binding, coarse 0.1/0.05) and paths PISO, PIMPLE-C, PIMPLE-4 are those of ADR 0073/0069
(harness `w34b_floor.rs`). **K** = the candidate: F-G + F-S, plus F-R if Y4 lets it be written. "Default" = every option off.

### Part D: falsifier of H-RELAX (run FIRST, after F-S and the rotational F-G are written, before F-R code and before any acceptance run; prints)

**Y4** TG, N = 32, fine pair, PIMPLE (n_outer 2, two correctors) with F-G + F-S on: Y4a `alpha_u = alpha_p = 1`; Y4b `alpha_u = 0.9, alpha_p = 1`;
Y4c `alpha_u = 1, alpha_p = 0.9`. Reading rule: H-RELAX **stands** if Y4a is in [1.9, 2.3], otherwise it is falsified. F-R is written if and only if H-RELAX
stands AND Y4b is outside [1.9, 2.3] (the velocity anchor alone reproduces the floor). Otherwise no F-R: C-O2 stays a miss with Y4 reported, K = F-G + F-S.
**No fourth fix** is tried in this wave, whatever the results.

### Part A: the ADR 0073 set with K (limits unchanged)

* **C-O1** TG PISO fine and coarse; **C-O2** TG PIMPLE-C fine; **C-O3** TG PIMPLE-4 fine; **C-O7** TG PISO variable step fine; **C-S1** TG-S PISO fine;
  **C-S2** TG-S PIMPLE-4 fine: all orders in [1.9, 2.3] against the same-scheme dt = 1/1280 reference.
* **C-O9** joint refinement against the exact solution, TG PISO, (16, 0.1), (32, 0.05), (64, 0.025): both orders >= 1.9.
* **N1** exact velocity AND pressure error with K <= 1.05 x the default at dt in {0.05, 0.025, 0.0125}, N = 32, PISO, on TG, TG-S, MMS, MODE (12 velocity and
  9 pressure comparisons; MODE pressure is zero and skipped, as in ADR 0073).
* **N2** with every option off the five `w33b_default_bits` fingerprints are identical; `baseline_hashes` unchanged.
* **C-O6r** (reported, no limit): MMS PISO and PIMPLE-C fine and coarse, TG PIMPLE-C coarse, TG-S PISO and PIMPLE-4 default.
* N3 (A1 decomposition, prints, no limit) is NOT rerun: by the prediction above F-S does not touch TG and F-R does not touch PISO/PIMPLE-4 (alpha 1), so it
  equals ADR 0073's F-G N3. If the implementation check below fails, this is void and the reason is reported.
* **Implementation check (prediction, not a limit of the fixes):** the TG errors of C-O1, C-O3 and C-O7 with K equal the ADR 0073 F-G errors to the printed
  four digits. A difference means F-S touches something it should not; it is then investigated before any other acceptance run, and reported.

### Part P: F-R (only if written)

* **N5** PIMPLE-C no-harm: exact velocity AND pressure error with K <= 1.05 x the default (all options off, same PIMPLE-C settings) at dt in {0.05, 0.025, 0.0125},
  N = 32, on TG, TG-S and MMS (9 velocity and 9 pressure comparisons). The errors of F-G + F-S without F-R are printed alongside (information).

### Part R: rotationally periodic seams (K on; new set-ups)

Mesh `annulus_sector(n, n, 0.5, 1.0, pi/3, Uniform, 0.1)` (60-degree sector, `theta_min`/`theta_max` a rotational pair, walls `inner`/`outer` with zero velocity,
`frontAndBack` empty), central convection, BDF2, nu = 0.02, t_end = 1, tolerances of the Common section. Paths PISO and PIMPLE-4 (n_outer 4, alpha 1).

* **SWIRL** (exact unforced Navier-Stokes solution): `u = U(r) e^{-nu k^2 t} e_theta`, `p = P(r) e^{-2 nu k^2 t}` with `U'' + U'/r - U/r^2 + k^2 U = 0`,
  `U(0.5) = U(1) = 0`, `k` the first eigenvalue, `max |U| = 1`, `P' = U^2/r`, `P(0.5) = 0`. U, U' and P from RK4 shooting (40 000 steps, bisection on k), cubic
  Hermite interpolation; accuracy 1e-14 (smoke). This is an independent solution (no Bessel tables used).
* **SECT-M** (manufactured, non-axisymmetric, no-slip): stream function `psi = g(t) F(r) cos(6 theta)`, `F = 32 (r - 0.5)^2 (1 - r)^2`, `g = 1 + 0.5 sin(2t + 0.3)`,
  `u_r = (1/r) d psi/d theta`, `u_theta = -d psi/dr`; `p = 0.25 g^2 cos(6 theta)`; source `f = u_t + (u.grad) u + grad p - nu lap u` (velocity derivatives by
  fourth-order central differences, h = 1e-4 for first and 1e-3 for second derivatives), balanced-force face values `f.S` on internal and coupled faces and
  `grad p.S` on the walls (as the ADR 0069 MMS), applied through the `step_with` hook at `t + dt`.
* Errors: relative L2 of the cell velocity against the same-scheme dt = 1/1280 reference on the same mesh (orders) and against the exact solution; pressure error
  as in ADR 0073 (volume means removed).

Acceptance (K on):
* **R1** SWIRL 16 x 16 PISO fine pair in [1.9, 2.3] (default 1.135); **R1b** SWIRL PIMPLE-4 fine pair in [1.9, 2.3] (default 2.052; no-harm row).
* **R2** SECT-M 16 x 16 PISO fine pair in [1.9, 2.3] (default 1.041); **R2b** SECT-M PIMPLE-4 fine pair in [1.9, 2.3] (default 0.538).
* **R3** joint refinement against the exact solution, PISO, (n, dt) = (16, 0.05), (32, 0.025), (64, 0.0125): SWIRL both orders >= 1.9; SECT-M both orders >= 1.8
  (the spatial orders of the smoke study are 1.878 and 1.921 on this sequence; 1.8 leaves the temporal part no room to lower the order by more than 0.08).
* **R4** no-harm on the sector, 16 x 16 PISO, dt in {0.05, 0.025, 0.0125}: exact velocity and pressure error with K <= 1.05 x the default, SWIRL and SECT-M (6 + 6).
* The default rows (all options off) of R1, R1b, R2, R2b, R3 are printed with them.

### Decision rules (fixed now)

* **K is accepted** if every row of Parts A (C-O1, C-O2, C-O3, C-O7, C-S1, C-S2, C-O9, N1, N2), P (if F-R is written) and R holds. The implementation check is
  not an acceptance row.
* **Per fix, if K is not accepted** (rule b of the wave preamble: a fix that fails its acceptance is removed): each fix is judged on the rows it is meant to move,
  all measured with K:
  - F-G: C-O1, C-O3, C-O7, C-S2, C-O9, N1, N2, R2b, R4;
  - F-S: C-S1, R1, R2, R3, N1 TG-S rows, R4;
  - F-R: C-O2, N5.
  A fix whose rows all hold is kept opt-in (default false, no schema key); a fix with a missed row is REMOVED from the code together with its tests. F-S and F-R
  were measured only together with F-G, so if F-G is removed they are removed as well. Removal of F-G touches: the field `rhie_chow_consistent`, `seam_faces`,
  `rc_commutator`, `seam_defects`, the F-G block of `step_with`, and every test of `w34b_floor.rs` that sets the field (including the default
  `c_o9_fg_joint_refinement`); the changed gate totals are reported against 1822 / 0 / 448.
* **Cylinder (A5 of ADR 0073 with K; ONE run, only if K is accepted).** `validation_cylinder_re100_w33b.rs` set-up (128 x 100, dt 0.05, PIMPLE n_outer 2, alpha 0.9,
  linear upwind, BDF2, t = 160, statistics as there) with every key of K on, run exactly once. K does no harm on the cylinder if K1b (St in [0.16005, 0.16995]),
  K2b (C_D in [1.2883, 1.3957]), K4 (>= 9 crossings) and T1 (`|St - 0.16536| <= 0.001974` and `|C_D - 1.3437| <= 0.0101`) all hold. The St and C_D shift against
  the wave-33B BDF2 run (St 0.16480, C_D 1.35247) is reported whatever it is. A failure keeps K opt-in with the cylinder result stated as a caveat; no second
  run, no other set-up. If K is not accepted, the cylinder is not run.
* Keys stay library-level (no `schema.rs` / `sim.rs` wiring; exposing them is the user's decision).

### Out of scope

Pressure anchoring of the relaxation (needs pressure history); the fractional path (F-K was rejected in ADR 0073); ALE; turbulence coupling; the
transposed viscous stress on rotational seams (`transposed_stress_source` keeps skipping them, a named limitation).

## Sources

No new literature. SWIRL is an exact solution derived here (azimuthal flow: the theta momentum equation is linear and the radial one is balanced by
`dp/dr = u_theta^2 / r`); SECT-M is manufactured. Every limit is either an order band, an order derived from the smoke study above, or a ratio to the default.

## Amendment 1 (after Y4; BEFORE F-R was run and before any acceptance run; nothing above was changed)

Order of work so far: F-S and the rotational F-G were written; the five `w33b_default_bits` fingerprints were checked with every option off (identical, N2);
then Y4 ran once (F-G + F-S on, F-R not yet written), TG N = 32, PIMPLE n_outer 2, two correctors, fine pair against the dt = 1/1280 reference:

| row | alpha_u, alpha_p | errors vs reference | vs exact | order |
|---|---|---|---|---|
| Y4a | 1, 1 | 1.7822e-5 / 4.3243e-6 | 1.7208e-3 / 1.7238e-3 | 2.043 |
| Y4b | 0.9, 1 | 7.6707e-5 / 3.2492e-5 | 1.7080e-3 / 1.7169e-3 | 1.239 |
| Y4c | 1, 0.9 | 1.7476e-5 / 4.2990e-6 | 1.7209e-3 / 1.7238e-3 | 2.023 |

By the registered rule **H-RELAX stands** (Y4a in band) and Y4b is outside the band (the velocity anchor alone reproduces the floor; the pressure anchor, Y4c,
does not), so F-R (velocity anchor only) was written as specified. Observation recorded before the F-R runs: with the `u^n` anchor the error against the
exact solution is 0.7 % / 0.4 % SMALLER than without relaxation (the O(dt) term partly cancels the spatial error), so N5 (F-R against the default PIMPLE-C, limit
1.05) is not a foregone conclusion.
