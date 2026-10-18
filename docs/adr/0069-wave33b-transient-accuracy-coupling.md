# ADR 0069: Wave 33B, transient accuracy and pressure-velocity coupling foundation (BDF2 order, PISO/PIMPLE stability)

Status: accepted. Everything under "Registered" was written BEFORE any code of the wave was changed and BEFORE any diagnostic run of the
new tests; the diagnostic matrix was not looked at beforehand (the only numbers known to the author are those already published in ADR
0043 and ADR 0060/0062, quoted below). Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured
and the assertion moves to an ignored `*_known_miss` test with the registered limit intact; a defective set-up gets a NEW corrected test
and the old one stays. Fix specifications that depend on the diagnosis are registered in an amendment BEFORE the solver code is touched.
Every fix is opt-in; the default behaviour stays bit-identical and `crates/rcfd-api/tests/baseline_hashes.rs` is never re-recorded.
Clean-room: first principles and published literature only; nothing from vendor software, nothing from vendor material.

Parents: ADR 0009 (BDF2 coefficients), 0037 (fractional step), 0043 (fractional-step BDF2 order 1.51), 0046 (PISO periodic partner), 0060
(cylinder K1/K3), 0062 (cylinder separation of errors, 181 x 141 blow-up), 0067 (divergence guard).

## What is known before the wave (quoted, not re-measured)

* ADR 0043: fractional-step (non-iterative) BDF2 on the periodic Taylor-Green vortex, N = 32, nu = 0.1, t = 1, reference dt/32, pair
  dt = 0.1, 0.05: order 1.513 after the periodic-partner extrapolation (pre-fix 1.040). Exploratory (unregistered) successive orders
  1.97, 1.45, 0.99, 0.93 for dt = 0.2 ... 0.0125 (the observed order FALLS as dt shrinks) and 2.00 at every pair with the dt-scaled
  collocated face term `dte (interp(grad p^n).S - snGrad p^n |S|)` switched off. Wall-bounded decaying mode: 2.07.
* ADR 0060/0062: cylinder Re 100, PIMPLE (n_outer 2, alpha 0.9, linear upwind, Euler), 128 x 100, dt 0.05: St 0.15878, C_D 1.30996;
  dt-Richardson limit on that mesh St 0.16536, C_D 1.3437; observed order in dt 0.87 (first order). 181 x 141 at dt 0.05 blew up
  (Courant 10 to 30, then growth) while dt 0.025 was stable.
* The iterative PISO/PIMPLE path (`PimpleSolver::step_with`) has the BDF2 weights, the BDF2 extrapolation of the convecting flux and of the
  deferred-correction state, implicit periodic partner links (ADR 0046) and a Choi-type transient Rhie-Chow carry-over over INTERNAL
  faces. Its BDF2 temporal order has never been measured by a registered test.

Consequence: two different BDF2 paths exist and every check below states which one it exercises. A fix confined to the fractional path
cannot move the cylinder (which runs the iterative path).

## Reading of the code (no run), first-principles review

1. Variable-step BDF2 weights (`ddt.rs`): `r = dt/dt_old`, `ddt phi = (c0 phi - c1 phi^n + c2 phi^{n-1})/dt` with
   `c0 = (1 + 2r)/(1 + r)`, `c1 = 1 + r`, `c2 = r^2/(1 + r)`. Derivation by differentiating the quadratic through `t^{n+1}, t^n, t^{n-1}`
   gives exactly these (check D0 below makes it a test on quadratics).
2. Startup: the first step is implicit Euler (local error O(dt^2), contribution to the global error O(dt^2)).
3. Convection: BDF2 steps extrapolate the convecting flux and the convected state `(1 + r) x^n - r x^{n-1}`.
4. Transient Rhie-Chow (iterative path): `ddt_defect[f] = c1 (phi^n - interp(u^n).S) - c2 (phi^{n-1} - interp(u^{n-1}).S)` over internal
   faces `f < n_internal` only; periodic coupled boundary faces get none. Added as `gamma_p/dt * defect`.
5. `ddt_history_weights` matches only `TimeScheme::Bdf2`. For `Bdf2Bounded` the matrix uses the blended BDF2/Euler time derivative while
   the Choi term and the extrapolation use the Euler weights `(1, 0)`. Whether this is a real inconsistency is a side check (S0).
6. Fractional path: predictor carries `grad p^n` (no pressure extrapolation), face flux carries `dte (interp(grad p^n).S - snGrad p^n |S|)`,
   no old-level flux defect carry-over; the hook of `step_with` is not available there.
7. The initial flux is the face interpolation of the initial cell velocity, which is not discretely divergence free; the first projection
   then removes a flux defect that scales with the step.
8. Every existing transient test uses a constant step (`r = 1`); the r-dependent coefficients have never been exercised in a solve.

## Registered

### Common set-ups

* **TG** (periodic Taylor-Green vortex, exact solution of the 2-D Navier-Stokes equations, any textbook): `box_2d(N, N, 2 pi, 2 pi, true, true)`,
  `u = (sin x cos y, -cos x sin y) exp(-2 nu t)`, `p = (cos 2x + cos 2y)/4 exp(-4 nu t)`, nu = 0.1, central convection, t_end = 1.
* **MODE** (wall-bounded decaying mode of ADR 0043 `f2c`): `box_2d(N, N, 1, 1, false, false)`, `v = sin(pi x) exp(-nu pi^2 t)`, no-slip at
  xmin, xmax, zero gradient u on ymin, ymax, `p = 0` fixed there; nu = 0.1, t_end = 1. No convection, no pressure: it tests ddt, diffusion and
  the wall boundary term.
* **MMS** (wall-bounded manufactured solution with convection, method of manufactured solutions, Roache 2002 UNVERIFIED reference for the name;
  the construction is elementary): unit square, no-slip on all four sides. `psi = g(t) X(x) Y(y)`, `X = x^2 (1 - x)^2`, `Y` the same in y,
  `u = g X Y'`, `v = -g X' Y`, `g(t) = 80 (1 + 0.5 sin(2 t + 0.3))`, pressure `p_ex = 0.5 (g/80)^2 cos(pi x) cos(pi y)`, nu = 0.05, t_end = 1.
  Source (acceleration, evaluated at the new time level) `f = u_t + u . grad u + grad p_ex - nu laplacian u` with
  `f_x = g' X Y' + g^2 (X X' Y'^2 - X X' Y Y'') - nu g (X'' Y' + X Y''') + dp_x`,
  `f_y = -g' X' Y + g^2 (-X X'' Y Y' + X'^2 Y Y') + nu g (X''' Y + X' Y'') + dp_y`
  (`X' = 2x - 6x^2 + 4x^3`, `X'' = 2 - 12x + 12x^2`, `X''' = -12 + 24x`, likewise Y). Cell values are point values at the cell centres, interior
  face values `f . S` at the face centres, and the BOUNDARY-face values of the source are set to `dp_ex/dn |S|` (the true wall pressure gradient;
  the solver's no-slip pressure condition is `dp/dn = body force . n`, which would otherwise be inconsistent with the viscous term). Hook-based
  (`step_with`), hence PISO/PIMPLE path only. The pressure level is pinned by the solver, so the pressure error is reported after removal of the mean.
* Solver tolerances for ALL order runs: momentum and pressure `tol_rel = 1e-12`, `tol_abs = 1e-14`, `max_iter` raised to 5000, so that the linear
  solver cannot form an error floor under the temporal error.
* Error measure: relative L2 norm (cell values, both components, volume weights uniform) of the difference of the velocity at t = 1 to a
  REFERENCE run of the same scheme on the same mesh with the constant step `dt_ref = 1/1280` (so at least 16 times finer than the finest pair
  member and 128 times finer than the coarsest). This isolates the temporal error at fixed h. The error against the exact solution is printed
  too. Observed order of a pair: `log2(e(dt)/e(dt/2))`.
* Pairs: **fine pair** (dt, dt/2) = (0.025, 0.0125) is the BINDING pair; **coarse pair** (0.1, 0.05) is registered with the same band (it was the
  pair of ADR 0043; the observed order of the fractional path falls with dt, so a pass on the coarse pair alone proves nothing).
* Paths: **PISO** = `PimpleSettings::piso(nu)` (n_outer 1, 2 correctors, `ddt_correction` on); **PIMPLE-C** = the cylinder's setting (n_outer 2,
  2 correctors, alpha_u = alpha_p = 0.9); **PIMPLE-4** = n_outer 4, 2 correctors, alpha 1 (outer loop to convergence); **FRAC** = fractional step,
  `FractionalSettings::default()` (incremental, Kim-Moin).

### Order checks (limits fixed now; band [1.9, 2.3] for BDF2, [0.9, 1.15] for Euler, on the observed order of the pair)

| id | problem | path | scheme | pair | band |
|---|---|---|---|---|---|
| D0 | quadratic `y(t)`, variable r in {0.5, 1, 1.7}: `add_ddt` BDF2 residual | - | BDF2 | - | <= 1e-12 (absolute, `y'` exact) |
| O1 | TG N = 32 | PISO | BDF2 | fine, coarse | [1.9, 2.3] each |
| O2 | TG N = 32 | PIMPLE-C | BDF2 | fine, coarse | [1.9, 2.3] each |
| O3 | TG N = 32 | PIMPLE-4 | BDF2 | fine | [1.9, 2.3] |
| O4 | TG N = 32 | FRAC | BDF2 | fine, coarse | [1.9, 2.3] each |
| O5 | MODE N = 32 | PISO and FRAC | BDF2 | fine, coarse | [1.9, 2.3] each |
| O6 | MMS N = 32 | PISO and PIMPLE-C | BDF2 | fine, coarse | [1.9, 2.3] each |
| O7 | TG N = 32 and MMS N = 32 | PISO | BDF2, VARIABLE step | fine | [1.9, 2.3] |
| O8 | TG, MODE, MMS | PISO, FRAC (TG, MODE) | Euler | fine | [0.9, 1.15] (control: the harness sees a first-order scheme as first order) |
| O9 | TG, joint refinement `(N, dt) = (16, 0.1), (32, 0.05), (64, 0.025)` against the EXACT solution | PISO | BDF2 | both successive pairs | order >= 1.9 (the scheme is second order overall; spatial and temporal error refine together) |

O7: step sequence alternating `dt (1.25)`, `dt (0.75)` with mean dt = 0.025 and 0.0125 (20 and 40 pairs of steps ending at t = 1.0); the reference is
the constant-step dt_ref run; the "order" is `log2` of the error ratio for the two mean steps. O9 uses constant steps with t_end = 1.

### Diagnostic matrix (prints, no limits; one factor at a time; each row run at the fine pair of the failing check)

Run for every O-check that misses, on the failing path; a factor is called "a cause" only if changing that factor ALONE moves the fine-pair
order into [1.9, 2.3], or by at least 0.3 towards it; several may be causes (all reported). A factor that does not move it is "cleared".

| id | factor | change |
|---|---|---|
| X1 | startup | start from an exact two-level history instead of the Euler first step: `snapshot()`, set `older`/`dt_old` of `hist_u` and `phi_hist` from the exact solution at `t = -dt`, `restore()` (no solver change) |
| X2 | initial compatibility | start from the exact face flux (`u_ex . S` at the face centres) instead of the interpolated cell velocity |
| X3 | Choi term | `ddt_correction = false` (PISO/PIMPLE) |
| X4 | correctors | `n_correctors` 3 instead of 2 |
| X5 | outer iteration | `n_outer` 4, alpha 1 (to convergence) |
| X6 | mesh dependence | TG at N = 16, 32, 64, dt in {0.05, 0.025, 0.0125}: fit `e = a dt^2 + b h^2 dt` by least squares on the nine errors (each against its own reference); the term `b h^2 dt` is "present" if it is at least 30 % of the fitted error at (N = 32, dt = 0.025), and its residual rms is reported |
| X7 | convection | `LinearUpwind` instead of central (fine pair only) |
| S0 | side check | `Bdf2Bounded` on TG PISO at the fine pair: observed order and the difference of the final field to the BDF2 run; the weight mismatch of review item 5 is recorded as a real inconsistency if the Bounded run differs from a run made consistent in the scratch tree by more than the BDF2/Bounded difference of the smooth TG (report only) |

The pressure-extrapolation and old-flux carry-over options of the fractional path are NOT diagnostics (they need code); they enter in the
amendment, together with the evidence of ADR 0043, if the matrix shows the face term to be the cause.

### Stability test (wall-bounded, impulsive start)

* Problem: lid-driven cavity, lid speed U = 1 (ymax), no-slip walls, fluid at rest at t = 0 (impulsive start), `t_end = 10` (ten lid
  transit times), Re = U L/nu in {100, 1000}, linear upwind convection, uniform pressure level pinned as usual, default solver tolerances.
* Meshes: **M-U** uniform 32 x 32 (`cavity_2d`) and **M-G** strongly graded 32 x 32 (`box_2d_graded`, `Grading::Symmetric(20)` in x and y, i.e.
  cells of ratio 20 clustered at all four walls; the cylinder blew up on strongly stretched near-wall cells and a uniform cavity may never show it).
* Time steps: `dt = Co_t h_min / U` with `h_min` the smallest cell width of the mesh. The achieved Courant number is the repo's `max_courant`
  (`0.5 sum|F| dt/V`) maximised over cells and steps; the table reports it, not only the target.
* Algorithms: **PISO** (n_outer 1, 2 correctors), **PIMPLE-2** (n_outer 2, 1 corrector, alpha_u 0.8, alpha_p 0.5), **PIMPLE-4** (n_outer 4, same), each
  with Euler and BDF2.
* **Required stable set** (must pass, on both meshes, both Re, both time schemes): PISO `Co_t` in {0.25, 0.5, 1.0}; PIMPLE-2 and PIMPLE-4 `Co_t` in
  {0.5, 1, 2, 4}. **Report-only scan above it** (never asserted): PISO `Co_t` in {2, 4, 8}; PIMPLE-2/4 `Co_t` in {8, 16}.
* **Stable** means all of: (a) no divergence-guard trip (`PimpleSolver::diverged` is `None`) and every field finite, (b) `max |u| <= 1.5 U` over all
  cells and all steps, (c) `max |p| <= 50 U^2` over all cells and steps, (d) the normalised continuity error of every step <= 1e-3, (e) the run
  reaches `t_end`.
* Classification rule (fixed now): a failure of the required set with PISO at achieved `Co <= 1` is a DEFECT (diagnosed by the X-matrix and
  the one-factor ablations `ddt_correction` off, `n_correctors` 3, upwind convection, tighter solvers; a fix is opt-in). A failure of the required PIMPLE
  set is a registered MISS (ignored `*_known_miss` test, with the diagnosis). A failure only inside the report-only scan is a SCHEME LIMIT: the explicit
  extrapolation of the convecting flux and the lagged coupling make BDF2-PISO conditionally stable, so a blow-up at high Co is reported as the remaining
  instability and not called fixed. For every configuration the largest stable and the smallest unstable `Co_t` of the scan are reported.

### Cylinder Strouhal check (ONE run, rule fixed now)

Set-up identical to `validation_cylinder_re100_w31.rs` (128 x 100, dt = 0.05, `cylinder_ogrid` r_out 20, Expansion(150), PIMPLE n_outer 2, alpha 0.9,
linear upwind, t = 160, statistics from t > 60, eight whole periods), with `time_scheme = Bdf2` and the fixes, if any, of the amendment switched on. Run
exactly once; the result is final whatever it is. Reference numbers (ADR 0062, Euler): St 0.15878, C_D 1.30996; dt-Richardson limits on this mesh St
0.16536, C_D 1.3437 (temporal deficits 0.00658 and 0.0337).

* **K1b (the registered K1)**: St within 3 % of 0.165, i.e. [0.16005, 0.16995].
* **K2b**: C_D in [1.2883, 1.3957] (the unchanged K2).
* **T1 (temporal error removed)**: `|St - 0.16536| <= 0.3 * 0.00658 = 0.001974`, i.e. St in [0.16339, 0.16733]; and `|C_D - 1.3437| <= 0.3 * 0.0337 = 0.0101`.
* K4: at least 9 zero crossings of the probe signal after t = 60.
* Verdicts: all met = "the first-order PIMPLE time error was the cause of the K1 miss and BDF2 removes it". K1b met but T1 missed = St recovered but the
  remaining deficit is not the temporal error alone (reported). K1b missed = not recovered (reported, the Euler-to-BDF2 shift is stated). A guard trip or no shedding
  = "BDF2 is unstable at Co ~ 10 on this mesh" (reported as the remaining instability; no second run, no other dt tried, the cell stays "not obtained").
* The test is `#[ignore]`d (cost above the 5 minute guideline), file `crates/rcfd-api/tests/validation_cylinder_re100_w33b.rs`.

### Fix policy and acceptance

* Only real defects are fixed (a cause named by the rules above AND a code mechanism that explains it). Collocation effects that are consistent
  perturbations of a dt-dependent spatial operator are stated as such even if an opt-in remedy is added.
* Every fix is a new field in `PimpleSettings` (default off; `FractionalSettings` is built by literals in tests and `sim.rs`, so it gets no new
  field) or a new `TimeScheme`-independent opt-in; library-level only, no `schema.rs` key unless a registered check needs it.
* Acceptance of a fix: with the flag on the failing check lies in its registered band; with the flag off the default is bit-identical (fingerprints of
  a TG run and a cavity run, recorded from the unchanged code in the amendment and asserted after), `baseline_hashes` unchanged, and the whole
  workspace test suite unchanged.

## Out of scope (stated)

* SIMPLEC at Ra 1e6 needing `alpha_u` 0.5: ADR 0062 found a linear instability of the lagged buoyancy coupling in the steady iteration (removed by
  `alpha_u` 0.5, SIMPLE, or the transient march), no solver defect; it is NOT re-diagnosed in this wave.
* Rotational pressure correction, Crank-Nicolson PISO, adaptive time stepping, turbulence-model transient coupling.

## Sources

Ferziger and Peric, Computational Methods for Fluid Dynamics, 3rd ed., sec. 6.3.2 (variable-step BDF2; cited by ADR 0009, not re-read);
Issa (1986) PISO; Choi (1999) transient Rhie-Chow; Zang, Street, Koseff (1994) collocated fractional step; Kim and Moin (1985); Guermond, Minev,
Shen (2006) review of projection methods on the effect of pressure extrapolation (all UNVERIFIED, recalled from memory, not read in this session).

## Amendment 1 (written after the first run of D0, O1 to O9, X1 to X7, S0 and the first run of the stability harness, BEFORE any solver change; nothing above was changed)

Order of events, for the record: the harness `crates/rcfd-fv/tests/w33b_transient_order.rs` was written and run once (all registered checks and the whole
diagnostic matrix), then the stability harness `w33b_stability.rs` (smoke subset first, then the full required set and scan, still running when this amendment
was written), then ONE extra exploratory probe (`Bdf2Bounded` against `Bdf2` on the three problems and both paths, scratch file, dt 0.1, 0.05, 0.025) after S0
had shown a gross failure; a scratch-only one-line change of `ddt_history_weights` (Bounded treated like BDF2) was tried once inside S0's registered
"consistent in the scratch tree" check and reverted. No repository solver file was changed.

### Measured (registered limits unchanged)

Orders of the pair (dt, dt/2) against the same-scheme dt = 1/1280 reference, N = 32; band [1.9, 2.3] (Euler [0.9, 1.15]):

| check | fine pair (0.025) | coarse pair (0.1) | verdict |
|---|---|---|---|
| D0 quadratic weights, r = 0.5, 1, 1.7 | residual 4.4e-15, 1.8e-15, 2.8e-15 | | met (<= 1e-12) |
| O1 TG PISO BDF2 | 1.624 (e 4.378e-5, 1.420e-5) | 2.290 | fine MISS, coarse met |
| O2 TG PIMPLE-C BDF2 | 1.223 | 1.587 | MISS, MISS |
| O3 TG PIMPLE-4 BDF2 | 1.031 | | MISS |
| O4 TG FRAC BDF2 | 0.965 (e 3.883e-4, 1.989e-4) | 1.460 | MISS, MISS |
| O5 MODE PISO and FRAC BDF2 | 2.016 | 2.070 | met, met (identical numbers on both paths: no convection, no pressure) |
| O6 MMS PISO BDF2 | 1.939 | 1.605 | fine met, coarse MISS |
| O7 variable step, TG PISO | 1.832 | | MISS |
| O8 Euler controls (TG, MODE, MMS PISO; TG, MODE FRAC) | 1.079, 1.040, 1.060, 1.044, 1.040 | | met |
| O9 joint refinement vs exact, TG PISO BDF2 | 3.814 (16 to 32), 3.254 (32 to 64) | | met |

(O6 PIMPLE-C and O7 MMS were run and are in the test output; they are reported in the final addendum.) Rule application (a factor is a cause only if it alone moves
the fine-pair order into the band or by >= 0.3 toward it): for TG PISO, X1 (1.044), X2 (1.623), X3 (0.854), X4 (1.358), X5 (1.031) move it AWAY or not at all at the
fine pair (the error at dt = 0.0125 does not fall below about 1.3e-5 in any variant, whereas the error at dt = 0.025 does), X7 (1.150); so by the registered rule NO
registered single factor is a cause for O1/O2/O3 and none is "cleared" either, because the floor is independent of all of them. X6 mesh dependence (e = a dt^2 + b h^2 dt):
PISO: h^2 dt part 29.0 % of the fit at (32, 0.025), BELOW the registered 30 % ("not present" by the rule; the relative rms misfit is 0.216, a poor fit), but the
individual errors show a strongly mesh-dependent floor (dt = 0.0125: 5.76e-5, 1.42e-5, 1.05e-5 for N = 16, 32, 64; observed orders at N = 64 are 2.29 and 2.36, at
N = 16 1.24 and 1.04); FRAC: 80.7 % ("present", misfit 0.192). Startup (X1) is cleared for all FRAC and MODE/MMS rows and is not the cause of the TG PISO floor either.

### Findings that were not registered diagnostics (stated as such)

1. `Bdf2Bounded` on the PISO path is grossly wrong (S0): error against the exact solution 6.4e-2, 1.7e-1, 2.2e-1 for dt 0.1, 0.05, 0.025 on TG, 8.1e-2, 3.7e-1,
   3.7e-1 on MODE (pure diffusion, no convection, no Choi term since the flux defect is exactly zero there), and 6.3e-3, 2.2e-2, 2.2e-2 on MMS, against 1.7e-3 and
   1.8e-3 and 5.7e-3 for `Bdf2`. The same scheme on the FRAC path is fine (MODE 8.7e-3, 2.9e-3, 1.3e-3). The registered S0 reading (Choi/extrapolation weights
   `(1, 0)` against the matrix weights) is NOT the cause: making the weights BDF2-like in the scratch tree made TG worse (error 0.47, 3.95), and MODE has no Choi term. The
   remaining difference between the two paths is the use of the mean diagonal `a_P = (1/n_active) sum_k a_k` in `HbyA = H/a_P` and `rAU = V/a_P` (PISO): the
   `Bdf2Bounded` diagonal `V/dt (theta c0 + 1 - theta)` has a per-cell, PER-COMPONENT `theta`, so `a_k != a_P`, and `H` omits the `(a_P - a_k) u_k` term that
   makes the corrector consistent with the component equation (the same family as ADR 0010). Hypothesis H-diag, tested by F3 below.
2. The PIMPLE smoke test of the stability harness (uniform 32 x 32, Re 100, Co_t = 4) is unstable for PIMPLE-2 (n_outer 2, 1 corrector, alpha_u 0.8, alpha_p 0.5) with both
   Euler and BDF2, stable for PIMPLE-4; PISO at Co_t = 1 is stable. The full stability sets follow in the addendum.

### Hypotheses and fixes registered before the code is touched (all opt-in, default off, `PimpleSettings` fields)

* **H-seam / F1 `ddt_correction_coupled`** (iterative path). The transient Rhie-Chow carry-over `ddt_defect` exists for internal faces only; the translationally periodic
  coupled boundary faces (the two seams of the doubly periodic vortex) are interpolated without it. Observations consistent with this: the floor appears only
  on the periodic problem (MODE and MMS fine pairs reach 2.0 and 1.94), is of the size expected when 2 of 32 face rows of the O(dt) `ddt_correction`-off error
  (X3: 1.9e-4 at dt = 0.0125) are unprotected (about 1.2e-5, measured floor 1.3e-5), and grows relative to the signal as N falls. F1 extends `ddt_defect` to coupled boundary faces with
  the face values of the solver's own periodic boundary interpolation. Acceptance: with F1 on, O1 (fine and coarse), O2 (fine and coarse), O3, O7 (TG) lie in [1.9, 2.3].
  If they do not, H-seam is falsified and the miss stays (ignored `*_known_miss` tests with the limits unchanged); F1 is kept only if it does not harm (O5, O6, O8, O9
  unchanged to 1e-12 with the flag on, because those problems have no coupled faces, and the TG errors vs exact within 5 % of the default).
* **H-frac / F2 `fractional_ddt_carryover`** (fractional path). The face term `dte (interp(grad p^n).S - snGrad p^n |S|)` is the O(dt h^2) mechanism of ADR 0043; the
  PISO path carries the old-level flux defects `d = phi - interp(u).S` over with the BDF2 weights and shows an error 9 times lower at the same dt (O1 against O4:
  4.4e-5 against 3.9e-4). F2 adds the same carry-over to the intermediate flux: `phi*_f += (c1 d^n_f - c2 d^{n-1}_f)/c0` with `c0 = c1 - c2` (so the factor is `dte/dt`, the
  fractional analogue of `gamma_p/dt`), on internal faces and (with F1's boundary definition) periodic coupled faces. Acceptance: with F2 on, O4 (fine and coarse) in [1.9, 2.3].
  The default Chorin/incremental behaviour, the Kim-Moin condition and the divergence-free projection are untouched.
* **H-diag / F3 `component_diagonal`** (iterative path, any scheme whose diagonal differs between components). With the flag on, `HbyA_k = (H_k + (a_P - a_k) u_k)/a_P` with `u_k` the
  current corrector velocity and `a_k` the diagonal of component `k` (the standard consistent form). Acceptance: `Bdf2Bounded` on the PISO path, errors against the exact solution at
  dt = 0.05 at most three times the `Bdf2` values: MODE <= 5.6e-3 (Bdf2 1.855e-3), TG <= 5.2e-3 (1.709e-3), MMS <= 1.7e-2 (5.686e-3). With the flag on, every `Bdf2`/`ImplicitEuler`
  result changes by less than 1e-10 relative (the diagonal is component independent there except at boundaries where the standard form is exact).
* **Default bit-identity (C1).** Fingerprints (rotate-xor-multiply over `u`, `p`, `phi`; test `w33b_default_bits.rs`), recorded from the UNCHANGED code: TG 32 x 32 nu 0.05 central dt 0.02 t = 1:
  PISO BDF2 `e9dca234e513bec8`; PIMPLE n_outer 2 (alpha 0.9) Euler `72d935a8d4993955`; fractional BDF2 `e46fbe511183e9fc`; cavity 16 x 16 nu 0.01 linear upwind dt 0.05 t = 1:
  PISO `Bdf2Bounded` `c21a6553acb2a7a6`; PIMPLE n_outer 2 (alpha 0.9) BDF2 `2c71144215f1d245`. Limit: zero differing bits with every new option off; `baseline_hashes` unchanged.
* **Not fixed, reported (no mechanism found or by design).** The PISO splitting error (X4: three correctors lower the MMS error by 30 %; X5: outer iteration to convergence lowers it
  by 10 times), the coarse-pair misses of O6 (pre-asymptotic) and the dt-dependence of `ddt_correction` off (X3) are properties of the algorithm, not defects.
