# ADR 0015: Turbulence finishing: k-epsilon BFS (F-042), SST/BSL (F-043), near-wall treatment catalogue (F-203)

Status: accepted. The checks, numeric limits and decision rules below were written before any run of the new tests
(the code audit and the data-file reading described in "Diagnosis before running" happened first and are recorded; no new
test had been compiled or run). They are not changed, widened, replaced or deleted after a run. A miss is reported as
measured and moved to an ignored `*_known_miss` test with the registered limit intact. If a test-set-up defect is found, a
NEW corrected test is added and the old one stays as a record. Addenda (mesh sizes, file names) may be appended at the end
without touching the registered limits.
Roadmap: F-042, F-043, F-203. Owner of this ADR: turbulence agent T2 (`kepsilon.rs`, `sst.rs`, `wall.rs`, their tests).
Clean-room: published literature and NASA TMR only; vendor material and vendor installs are not read.

## Original registered criteria (kept, unchanged)

* F-042: channel Re_tau = 395 within 10 percent of DNS (centre-line, bulk, log layer within 10 percent of Reichardt):
  `tests/channel_kepsilon.rs`, passing. BFS reattachment x_r/h = 6.26 +- 0.10 (Driver and Seegmiller) for Standard, RNG,
  Realizable: `tests/step_kepsilon.rs`, ignored `*_known_miss` (6.402 / 6.373 / 5.359). Convergence of smooth/hysteresis
  wall treatments (max update < 1e-7, nu_t change < 1e-6): unchanged.
* F-043: channel `tests/channel_sst.rs` passing; flat plate `tests/flat_plate_sst.rs` (Cf(0.97) within 5 percent of the TMR
  CFL3D value 0.00268299 and file value 0.0026909; power law within 5 percent for x <= 1) passing; White band 5 percent at
  x = 0.97 `sst_flat_plate_cf_within_5pct_of_white_known_miss` (-7.36 percent), unchanged.
  Roadmap acceptance text: "Flat plate / BFS / NACA0012 validation passes; y+~1 meshes stable".
* F-203: "Flat-plate skin friction matches the Prandtl-Schlichting correlation within 5 percent at y+ near 30 and near 1
  for smooth walls; rough-wall Cf matches the fully-rough correlation within 10 percent". Title scope: enhanced and
  blended wall treatments, scalable and non-equilibrium wall functions, wall-omega options, equivalent-sand-grain
  roughness with velocity-shift law.

## Diagnosis before running (code audit and data reading; nothing new executed)

Method: compare the implementation with the published definitions, then compare the test set-up with the experiment.

k-epsilon (`kepsilon.rs`): constants and forms re-read against Launder and Spalding (1974), Yakhot and Orszag / Yakhot et
al. (1992) (C_mu 0.0845, C1 1.42, C2 1.68, sigma 0.7194, eta0 4.38, beta 0.012, C2* strain term) and Shih et al. (1995)
(variable C_mu = 1/(A0 + A_s U* k/eps), A0 4.04, A_s = sqrt(6) cos(phi), phi = acos(sqrt(6) W)/3, W = S_ij S_jk S_ki /
(S_ij S_ij)^1.5, U* = sqrt(S_ij S_ij + W_ij W_ij), C1 = max(0.43, eta/(eta+5)), C2 1.9, sigma_eps 1.2, destruction
C2 eps^2/(k + sqrt(nu eps))). Production P = nu_t S^2 with S = sqrt(2 S_ij S_ij). No defect found in the transport
terms. Two open modelling facts: (i) k and epsilon convection is first-order upwind in the step runs (the momentum scheme is
linearUpwind); (ii) standard wall functions are used on the step face and the floor inside the recirculation, where the
log-law equilibrium assumption is not met (a model limitation, not a bug).

Step set-up versus the experiment (TMR 2DBFS page https://tmbwg.github.io/turbmodels/backstep_val.html, fetched
2026-10-15, and the pinned experimental files of `validation-data/data/backward-step-tmr/`): the TMR page states
Re_H about 36000 (an earlier value 50000 was corrected by the experimenter), Re_theta 5000 at x/H = -4, reference
velocity at the centre line near x/H = -4, upstream boundary layer thickness "approximately 1.5H", reattachment
x/H = 6.26 +- 0.10, straight top wall, free-stream turbulence not stated on the page. The pinned file
`profiles.exp.dat` (zone "exp, x/H=-4") gives u/Uref, v/Uref and the Reynolds stresses 1000 u'u'/Uref^2 etc. from y/H = 1.10
(0.1H above the upstream floor y/H = 1; the channel is 8H high) upward; in the free stream (y/H 2.8 to 7.5):
u'u' = 0.37e-3, v'v' = 0.27e-3 (Uref^2), i.e. k about 4.6e-4 if w'w' = v'v' (intensity about 1.75 percent), and
u/Uref = 0.657, 0.79, 0.926, 0.982 at 0.1, 0.4, 1.0, 1.4 H above the floor. The existing test `step_kepsilon.rs` differs:
S1 Re_h = 37500 (TMR: 36000); S2 inlet plane at x/H = -5 (experimental profile: -4); S3 free-stream k = 1.5e-4
(1 percent intensity, measured about 4.6e-4); S4 a 1/7 power law with delta 1.5 (the delta matches the TMR value; the
shape differs from the measured points by up to 4 percent in u/U, e.g. 0.855 against 0.818 at 0.5H); S5 k(d) from
u_tau = 0.038 and a mixing-length epsilon, not from the measured stresses. These are test-set-up discrepancies, not
solver defects. They are corrected in a NEW test (BFS-C below); the old test stays unchanged as a record.

SST (`sst.rs`) against the TMR SST page (fetched 2026-10-15): beta* 0.09, a1 0.31, sigma_k1 0.85, sigma_w1 0.5, beta1 0.075,
sigma_k2 1.0, sigma_w2 0.856, beta2 0.0828, gamma 5/9 and 0.44, F1, F2, CDkw floor 1e-10, production limiter
10 beta* rho k omega, nu_t = a1 k / max(a1 omega, S F2) all agree; the cross-diffusion and blending code was re-read. The
TMR page says BSL differs "only in sigma_k1" from SST and has a different eddy viscosity expression (the page does not
print them): see "BSL" below. Flat plate: the pinned TMR file `cf_as_function_of_x.dat` carries three White curves (read
off at x = 0.5 / 0.97 / 1.5 by interpolation): theory 1 "6-112a" = 0.0592 Re_x^-0.2 (3.10981e-3 / 2.72379e-3 /
2.49637e-3), theory 2 "6-121" (3.16930e-3 / 2.88302e-3 / 2.70896e-3), theory 3 "6-134" = 0.455/ln^2(0.06 Re_x), the
formula behind the existing White miss (3.20314e-3 / 2.87459e-3 / 2.68528e-3). The TMR CFL3D SST-V file value is
2.96543e-3 / 2.69089e-3 / 2.53108e-3 at the same stations. So the reference CFL3D SST solution itself is 6.4 percent
below theory 3 at x = 0.97 and 7.4 percent below it at x = 0.5, and about 1 percent below theory 1 at 0.97: the
"White miss" is a property of the SST model against that correlation (the TMR implementation has it too), not of this
implementation, which reproduces the CFL3D value to 0.7 percent.
Possible implementation-specific contribution (to be measured, not assumed): the Cf spread of 3.6 percent across first-cell
y+ 0.5 to 2 recorded in the F-043 note; the wall treatment differs from the TMR boundary condition (k = 0 at the wall,
omega_w = 60 nu/(beta1 y1^2) with no wall function), see "SST wall options".

Corrected diagonal (ADR 0010): the step mesh has only `inlet`, `outlet`, `walls`, `frontAndBack` (empty) patches, no
symmetry plane and no mixed component condition, so the component diagonals are bit-equal and
`component_diagonal = corrected` must be a bitwise no-op there (check A4); the flat-plate runs have a symmetry top plane
and are tested with both options (check B6).

## Scope decisions (stated up front)

* NOT attempted: non-equilibrium wall functions (Kim and Choudhury 1995). The pressure-gradient sensitised log law and the
  two-layer cell-averaged production/dissipation need the wall-tangential pressure gradient in the turbulence update and
  the formulas could not be verified from an accessible published source here (they are known to me only from memory).
  F-203 therefore cannot be `done`.
* NACA0012: attempted only after everything else, and only if a body-fitted O-grid and a far-field condition can be built
  from the existing generators (`cylinder_ogrid` + `warp`, face-set splitting for inflow/outflow). Data are verified
  local files (`validation-data/data/naca0012-tmr/CLCD_Ladson_expdata.dat`: Re 6e6, M 0.15, tripped; at alpha 10.12 deg
  Cl 1.0707, Cd 0.01201 for the 80 grit zone, 1.0775 / 0.01175 at 10.10 for 120 grit). Registered if run: N1 Cl(alpha=10)
  within 10 percent of 1.07 (incompressible solver against M 0.15 data; the compressibility effect is about 1 percent),
  N2 Cd(alpha=10) within 25 percent of 0.0120, N3 Cl(alpha=0) in |Cl| < 0.01 (symmetry), N4 y+~1 stability: converged
  (max update < 1e-6) on three meshes with first-cell y+ about 2, 1, 0.5 and Cl spread across them <= 3 percent. If it is
  not run it is recorded as "not attempted" with the reason.
* The F-203 hook for the schema wiring lives in `crates/rcfd-api/src/turb.rs`, which this agent must not edit (another
  agent owns it). Wiring is done through a new file `crates/rcfd-api/src/wall_opts.rs` called from `case.rs` (validation)
  and `sim.rs` (application to the built model). If this cannot be done without editing `turb.rs` it is reported.

## Implementation decisions fixed before running

1. `WallFunction { Standard, Scalable, Enhanced }` for k-epsilon (field `wall_function`), orthogonal to the existing
   `WallTreatment { Switch, Hysteresis, Smooth }`:
   * Standard: unchanged (bit-identical default; baseline hashes must not move).
   * Scalable (Grotjans and Menter, ECCOMAS 1998, as I recall it, unverified source): in the wall-adjacent cell y* is
     replaced by max(y*, y*_lam) in the logarithm of the wall shear, in epsilon = C_mu^(3/4) k^(3/2)/(kappa y_eff)
     and in the log-law production, with y_eff = max(y, y*_lam nu/(C_mu^(1/4) sqrt(k))). The wall shear
     nu + nu_t,w = nu y* kappa/ln(E max(y*, y*_lam)) uses the true y* in the numerator.
   * Enhanced (two-layer, Wolfstein 1969, Chen and Patel 1988; blending after Jongen 1998; wall law of Kader 1981): in
     cells with Re_y = sqrt(k) d/nu < 200 the length-scale relations nu_t = C_mu sqrt(k) l_mu,
     eps = k^(3/2)/l_eps, l = c_l d (1 - exp(-Re_y/A)), c_l = kappa C_mu^(-3/4), A_mu = 70, A_eps = 2 c_l are blended
     with the transported values by lambda = (1 + tanh((Re_y - 200)/A_b))/2, A_b = 20/atanh(0.98); the wall shear is the
     Kader blend u+ = exp(Gamma) y+ + exp(1/Gamma) ln(E y+)/kappa, Gamma = -0.01 y+^4/(1 + 5 y+) (constants from memory,
     unverified). Roughness is not combined with Enhanced.
   * Roughness (all wall functions and SST): equivalent sand grain k_s, velocity shift u+ = ln(E y+)/kappa - Delta B(k_s+),
     k_s+ = k_s u_tau/nu (k-epsilon: u_tau replaced by C_mu^(1/4) sqrt(k)); Delta B = 0 for k_s+ <= 2.25,
     (1/kappa) ln[(k_s+ - 2.25)/87.75 + C_s k_s+] sin[0.4258 (ln k_s+ - 0.811)] for 2.25 < k_s+ < 90,
     (1/kappa) ln(1 + C_s k_s+) for k_s+ >= 90 (Cebeci and Bradshaw 1977, from memory, unverified). C_s default
     = exp(-kappa (8.5 - B)), B = ln(E)/kappa, so that the fully rough law reproduces Nikuradse's
     u+ = ln(y/k_s)/kappa + 8.5; the textbook constant 0.5 would give 7.2. This is a documented choice, not tuned on the
     checks. A warning is produced when the wall-cell distance is below k_s.
2. SST wall options (`wall_omega`): `automatic` (current, default, bit-identical), `low_re`: the TMR boundary condition
   (wall-face omega = 60 nu/(beta1 y1^2), k Dirichlet 0 on the wall face, wall shear nu u_P/y_P with no Spalding law and no
   cell-omega fixing) for y+ of order 1; reported with the same checks as `automatic`, never substituted for it.
3. BSL (`sst_variant = bsl`): eddy viscosity nu_t = k/omega (no a1 limiter, no F2), sigma_k1 = 0.5, beta* 0.09,
   sigma_w1 0.5, beta1 0.075, sigma_k2 1.0, sigma_w2 0.856, beta2 0.0828, gamma_i = beta_i/beta* -
   sigma_wi kappa^2/sqrt(beta*) with kappa = 0.41 (0.5532, 0.4403), the consistency relation of Menter (1994) which I
   re-derived rather than took from a table; Menter's sigma_k1 = 0.5 for BSL is from memory and unverified (TMR only says
   it differs). The production limiter and the F1 blend are the SST ones.
4. Corrected k-epsilon BFS test (BFS-C): Re_h = 36000 (TMR), inlet plane at x/H = -4, l_out 40, expansion ratio 1.125,
   inlet profile d (distance from the nearer of the two inlet-channel walls): u from the measured points (u = u(0.1)
   (d/0.1)^(1/7) below d = 0.1 and the same profile mirrored on the upper wall: the upper-wall profile is not
   tabulated except one point), k = (u'u' + 2 v'v')/2 x 1e-3 interpolated the same way (constant below d = 0.1),
   epsilon = C_mu^(3/4) k^(3/2)/l with l = min(kappa d, 0.09 x 1.5). Meshes: same grading as the old test with n =
   [16,107,27,13] (coarse), [24,160,40,20] (medium), [36,240,60,30] (fine). SIMPLEC 0.8, linearUpwind momentum, upwind
   k/epsilon, smooth wall treatment, convergence max update < 1e-7 and nu_t change < 1e-6.

## Checks and numeric limits

### F-042 (k-epsilon, BFS), `rcfd-turb/tests/step_kepsilon_corrected.rs` (ignored, heavy, release)

* A1: BFS-C, medium mesh, x_r/h in [6.16, 6.36] (the original window, not widened) for Standard, RNG, Realizable. A miss
  is recorded in `*_known_miss` tests.
* A2 grid-convergence statement: for each variant on BFS-C: "grid-converged" iff |x_r(fine) - x_r(medium)| <= 0.10 h and
  all runs satisfy the convergence criterion; the three values are reported. The same statement is made for the old set-up
  from its recorded values (5.20/5.36/5.46, 6.32/6.37/6.38, medium/fine 6.40/6.51).
* A3 sensitivities (reported, no pass/fail; a factor is called "responsible for a miss" iff changing it alone moves x_r by
  at least 0.10 h toward the window), all medium mesh: (a) old set-up with k/epsilon convection linearUpwind; (b) BFS-C
  with free-stream k 1.5e-4 instead of the measured one; (c) BFS-C with Re_h 37500; (d) old set-up with inlet plane at
  x/H = -4 (only l_in changed).
* A4: `component_diag_correction = true` versus false on the coarse BFS-C mesh, 200 iterations: max |u difference| = 0
  (bitwise).
* A5: no pass/fail for Standard against the literature statement that standard k-epsilon under-predicts BFS reattachment
  by 10 to 20 percent; the number is reported with the statement marked "from memory, unverified" in
  docs/reference-data.md.

### F-043 (SST and BSL), `rcfd-turb/tests/flat_plate_bsl_sst.rs`, `step_sst.rs` (ignored, heavy, release)

All flat-plate runs use the TMR set-up of `flat_plate_sst.rs` (140 x 110 cells, growth 1.1, 70 layers, target y+ 0.5
unless stated, defined at x = 2, same convergence criterion max update < 1e-9 and nu_t change < 1e-6 within 8000
iterations; a run that does not converge decides nothing and is reported).

* B1 (SST, automatic): Cf at x = 0.5, 0.97, 1.5 within 3 percent of the TMR CFL3D SST-V file values 2.96543e-3, 2.69089e-3,
  2.53108e-3.
* B2 (SST, automatic): Cf at x = 0.5, 0.97, 1.5 within 5 percent of each of the three TMR theory curves (values above).
  Expected before running: theory 1 passes, theories 2 and 3 miss at all stations because the reference solution itself is
  more than 6 percent below them; reported anyway, theory 3 at 0.97 is the original White miss.
* B3 (BSL): same stations, within 5 percent of the SST-V CFL3D values and (separately) within 5 percent of SST
  (this implementation, automatic). Hypothesis: BSL and SST differ by less than 5 percent on a zero-pressure-gradient plate
  (a1 limiter inactive).
* B4 (y+~1 stability on three meshes): first-cell y+ targets 2.0, 1.0, 0.5 (same number of layers), for SST automatic,
  BSL and SST low_re. Stable iff (i) each run converges by the criterion above and (ii) the Cf(0.97) spread
  (max - min)/mean over the three meshes <= 3 percent. (The F-043 note records a 3.6 percent spread for automatic; a
  miss is expected and will be reported, the 3 percent is not widened.)
* B5 (BFS, SST and BSL, automatic): BFS-C set-up on wall-resolved meshes (floor, step face and inlet-channel walls with
  `Grading::WallLayers`, first-cell y+ about 2, 1, 0.5 on the floor downstream, three meshes): (i) every run converges
  (max update < 1e-7 and nu_t change < 1e-6 within 8000 iterations), (ii) x_r/h of the y+~1 mesh in [6.16, 6.36],
  (iii) x_r spread across the three meshes <= 0.15 h. Convection of k/omega first-order upwind (linearUpwind of k/omega
  diverged in the F-043 note).
* B6: flat plate (y+ 0.5 mesh, automatic) with `component_diagonal = corrected` (symmetry top plane): Cf(0.97) changes
  by less than 1 percent relative; reported if more.
* B7: NACA0012 conditional, see scope decisions.
* Unit tests (default): BSL constants gamma1 = 0.5532 and gamma2 = 0.4403 to 5e-4; BSL with F1 = 1 reduces to
  Wilcox-type k-omega eddy viscosity k/omega; SST `automatic` untouched (the whole `flat_plate_sst`, `channel_sst`,
  `sst_schema` tests unchanged).

### F-203 (wall catalogue), `rcfd-turb/tests/near_wall_catalogue.rs`, unit tests in `wall.rs`, `kepsilon.rs`

Smooth flat plate against the Prandtl-Schlichting drag law C_F(L) = 0.455/(log10 Re_L)^2.58 (Schlichting and Gersten,
Boundary-Layer Theory; plate turbulent from the leading edge), applied to the plate-average coefficient
(sum of wall-face shear x width over [0, L]; the first-cell shear from the wall-face viscosity as in the SST test) with
L = 1.0 and 2.0 (Re_L = 5e6 and 1e7; P-S values 3.365e-3 and 3.003e-3; the integral includes the poorly resolved leading
edge, disclosed). Tolerance 5 percent each. A run is "applicable" only if the realized first-cell y+ for
x in [0.5, 2] lies in [20, 60] (y+~30 meshes: target 30, 40 layers, growth 1.1, ny 60, nx 140) or in [0.3, 2]
(y+~1 meshes: target 1.0, 70 layers, growth 1.1, ny 110); otherwise it is reported "not applicable" and the mesh is not
retuned.

* C1 (SST automatic): y+~30 and y+~1.
* C2 (k-epsilon, free stream k = 1.5e-4, eps = 1e-3 i.e. 1 percent intensity and nu_t/nu 10, field initialised to the
  free stream): Standard at y+~30; Scalable at y+~30 and y+~1; Enhanced at y+~1 and y+~30. The run is "turbulent" only
  if Cf(0.5) >= 0.5 x 0.0576 Re_x^-0.2; a laminarised run is a miss.
* C3 rough plate: nu = 1e-7, U = 1, L = 2, k_s = 1e-3 (k_s+ about 500 to 1200, fully rough), wall-cell centre at
  y = 2 k_s, local Cf at x = 0.5, 1.0, 1.5 within 10 percent of the fully rough correlation
  Cf = (2.87 + 1.58 log10(x/k_s))^(-2.5) (Schlichting, Boundary-Layer Theory, from memory, unverified), for SST
  automatic with roughness and k-epsilon Standard with roughness; the same plate without roughness is reported for contrast
  (must give lower Cf; the ratio is reported).
* C4 unit checks (default tests): Delta B continuous at k_s+ = 2.25 and 90 (jump < 1e-12), non-decreasing in k_s+,
  fully rough law within 0.05 of ln(y/k_s)/kappa + 8.5 at k_s+ = 1000; scalable identical to Standard to 1e-14
  where y* >= y*_lam and different (smaller wall shear) below; Kader blend: u+ = y+ within 1 percent for y+ <= 1 and
  ln(E y+)/kappa within 1 percent for y+ >= 50; Enhanced limits lambda -> 0 for Re_y << 200 (pure two-layer) and -> 1 for
  Re_y >> 200; default options bit-identical to the previous code in the existing `wall.rs` and `kepsilon.rs` tests.
* C5 schema (default tests, `rcfd-api/tests/wall_options.rs`): `turbulence.wall_function` in {standard, scalable,
  enhanced}, `turbulence.roughness_height` (>= 0), `turbulence.roughness_constant` (> 0, default derived), `turbulence.wall_omega`
  in {automatic, low_re}, `turbulence.sst_variant` in {sst, bsl}; located errors for: roughness with enhanced, wall_function
  on SST, wall_omega on k-epsilon, sst_variant on k-epsilon, negative roughness; documented in docs/case-schema.md. The
  run through the tree equals the library run to 1e-12 for each new option after 60 iterations.
* Status rule: F-203 `done` only if scope items (enhanced, blended, scalable, non-equilibrium, wall-omega, roughness) are
  all implemented and C1 to C5 pass. Non-equilibrium is not attempted, so the status stays `partial` or `todo` whatever
  the numbers.

### Hygiene

* H1: `baseline_hashes` bit-identical, existing turbulence tests unchanged and passing.
* H2: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
  (heavy tests `#[ignore]`), `CARGO_TARGET_DIR=<target dir>`.
* H3: every tuning is disclosed in the results; unverifiable references marked in docs/reference-data.md.

## Results

(filled in after the runs, without changing anything above)
