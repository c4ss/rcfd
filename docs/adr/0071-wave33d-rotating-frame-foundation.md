# ADR 0071: Wave 33D, rotating-frame (MRF) foundation checks

Status: accepted. Everything under "Registered" was written before any of the new test code was written or run. Limits are not changed, widened or deleted after a
run; a miss becomes an ignored `*_known_miss` test with the limit intact, a defective set-up gets a NEW corrected test and the old one stays ignored. Baseline hashes
(`crates/rcfd-api/tests/baseline_hashes.rs`) are never re-recorded; a change that would move one is made opt-in and reported. Clean-room: first principles and published
literature only. Parents: ADR 0008 (MRF, checks M1-M6), ADR 0055/0059 (F-126), ADR 0065 (fan sector, S3 seam miss), `docs/milestone-fan.md`.

Purpose: the fan milestone needs a steady rotating-frame solve whose correctness is established without any fan. Existing coverage (ADR 0008 M1-M6, `sources.rs` rotating
channel and fluid at rest, `periodic.rs` sixty-degree sector) is kept; this wave adds what is missing: a mesh sequence with observed order for the rotating-frame
formulations, a 3-D Coriolis-dominated analytic check, frame invariance across the three formulations, the rotational-periodic seam diagnosis, and a published
rotating-flow comparison. Two formulations are tested: `rotating_frame` (relative velocity, Coriolis and centrifugal accelerations) and `mrf` (absolute velocity, frozen
rotor, ADR 0008).

## Registered

Common: SIMPLEC, `linear_upwind` for the case-tree runs (as ADR 0008) and `Central` for the fv-level runs (as `periodic.rs`), residual targets 1e-10 or the iteration cap
stated, convergence required (a run that is not converged is a failure of the check, not a pass). Annulus `r_i = 0.5`, `r_o = 1.0`, `nu = 0.1`, `rho = 1`, `W_i = 1`,
`V_i = W_i r_i = 0.5`, analytic Taylor-Couette `u_theta = A r + B/r`, `A = -W_i r_i^2/(r_o^2 - r_i^2)`, `B = W_i r_i^2 r_o^2/(r_o^2 - r_i^2)` (derivation: ADR 0008). The
observed order between two meshes is `log2(e_coarse/e_fine)` (every refinement halves the spacing in every direction); an order is reported as n/a when the finer error is below
1e-7 V_i (round-off floor), which counts as a pass of the order criterion.

### C1 Solid-body rotation recovered (fluid at rest in the inertial frame)
Derivation: in a frame rotating at `Omega` the steady Navier-Stokes equations for the relative velocity have the rest-in-inertial-frame solution `u_abs = 0`, i.e.
relative velocity `-Omega x r`, with centrifugal pressure `p = Omega^2 r^2/2` (kinematic).
* C1a, `rotating_frame` (`annulus_3d`, `Omega = 2`, centrifugal on, walls inner and outer at relative velocity 0, i.e. a fluid rigidly rotating in the inertial frame, end planes
  symmetry, `lz = 0.4`), meshes (nx, ny, nz) = (24, 4, 2), (48, 8, 2), (96, 16, 2). Limits on every mesh: `max |u_rel| <= 1e-3 Omega r_o` (the wave 9 criterion of
  `sources.rs`); `max |(p - p_0) - Omega^2 (r^2 - r_0^2)/2| <= 1e-2` of the range `Omega^2 (r_o^2 - r_i^2)/2` (cell 0 is the reference).
* C1b, `mrf` zone `all` (`Omega = 2`, absolute formulation, walls moving at `Omega x r`, same meshes): `max |u - Omega x r| <= 2e-2 Omega r_o` and the same pressure
  difference criterion with limit `5e-2` of the range (these are the M1 limits of ADR 0008, which missed the pressure limit on the interface mesh).

### C2 Taylor-Couette, rotating frame, mesh sequence with observed order
Inner cylinder angular velocity `W_i = 1`, outer cylinder at rest in the inertial frame. Frame angular velocity `Omega = W_i` (the inner wall is at rest in the frame, the outer wall
moves with relative velocity `-Omega x r`). The error is `e = max over cells |u_abs,theta - (A r + B/r)| / V_i` with `u_abs,theta = u_rel,theta + Omega r` at cell centres.
* C2a, full annulus, `rotating_frame`, (nx, ny, nz) = (12, 4, 2), (24, 8, 2), (48, 16, 2): `e <= 5e-3` on the finest mesh; observed order of each refinement `>= 1.0` and the mean
  of the two `>= 1.5`.
* C2b, periodic 60-degree sector (`rcfd_mesh::gen::annulus_sector`, one cell in z, rotational-periodic pair `theta_min/theta_max`), the same frame source evaluated by
  `rcfd_fv::sources::RotatingFrame` in an fv-level loop, (n_theta, n_r) = (2, 4), (4, 8), (8, 16): the same limits.

### C3 Ekman layer (Coriolis-dominated, 3-D analytic)
Rotation `Omega` about the wall-normal `y`, no-slip wall at `y = 0`, geostrophic stream `U_g` along `x` far from the wall maintained by the body acceleration
`f = (0, 0, -2 Omega U_g)` (the pressure gradient that balances the Coriolis force of the geostrophic flow), flow independent of `x` and `z` (periodic in both). Derivation: with
`Omega = Omega e_y`, perturbation `(u', w') = (u - U_g, w)` obeys `nu u'' - 2 Omega w' = 0`, `nu w'' + 2 Omega u' = 0`; `V = u' + i w'` gives `V'' = -i (2 Omega/nu) V`, bounded solution
`V = V_0 exp(-(1 - i) y/delta)`, `delta = sqrt(nu/Omega)`, with `V(0) = -U_g`:
`u = U_g [1 - e^{-y/delta} cos(y/delta)]`, `w = -U_g e^{-y/delta} sin(y/delta)`. (The same layer in the usual literature is the Ekman layer; this derivation is made here from the
equations, no reference value is read from a book.) Set-up: `nu = 0.01`, `Omega = 1`, `U_g = 1`, `delta = 0.1`, height `8 delta = 0.8` with the velocity fixed to `(U_g, 0, 0)` at the top,
box of 0.1 x 0.8 x 0.1, (2, N, 2) cells with N = 16, 32, 64 uniform, fv-level with `RotatingFrame` (centrifugal off). Error `e = max over cells max(|u - u_e|, |w - w_e|) / U_g` at
cell centres. Limits: `e <= 2e-3` at N = 64, orders as in C2. Additional criterion: the spiral is present, i.e. the maximum of `|w|` is within 5 % of the analytic maximum
`U_g e^{-pi/4} sin(pi/4) = 0.3224 U_g` (checks the sign of the Coriolis force: a wrong sign grows the layer instead).

### C4 Frame invariance across formulations
Same Taylor-Couette problem (inner wall `W_i`, outer wall at rest in the inertial frame), mesh (48, 16, 2), solved (a) in the inertial frame without source, (b) `mrf` zone all with
`Omega = W_i`, (c) `rotating_frame` with `Omega = W_i`, (d) `rotating_frame` with `Omega = 0.5 W_i`, (e) `mrf` with `Omega = 0.5 W_i`. Absolute velocity of (c), (d) is
`u_rel + Omega x r`. Limits: each run `max |u_abs,theta - analytic| <= 1.5e-2 V_i` (the M2 limit); maximum cell difference of the absolute velocity vector of each of (b)-(e)
against (a) `<= 1e-2 V_i` (the M3 limit); `max(|u_r|, |u_z|)` of the absolute velocity `<= 1e-3 V_i` in each run (the M3 limit; the MRF runs of M3 are known to be marginal).

### C5 Rotational-periodic seam, diagnosis
Background: ADR 0065 S3 found the two seam patches of the fan sector do not carry exactly opposite mass flow (relative 3.5e-6 to 2.7e-5 of `|m_lo|`), the cause was not investigated.
Derivation of what must hold: the pressure equation couples the two faces of a pair with one shared `gamma |S|` and the pair `phiHbyA` uses the distance-weighted mean of the cell
`HbyA` values with the rotation applied, so `phi_lo = -phi_hi` face by face if and only if `S_hi = -R S_lo` (the seam faces are exact rotation images of each other) and the extra face
terms (Coriolis, centrifugal) are equally antisymmetric.
* C5a, analytic sector: the C2b sector (60 degrees, n_theta = 8, n_r = 16, depth 0.1) with the frame source at `Omega` in {0, 1, 5, 20} (inner wall at `W_i` in the inertial frame,
  outer wall at rest in the inertial frame). Limits: `|m_lo + m_hi| / |m_lo| <= 1e-10` (with `m_x = sum of phi over the faces of patch x`), final cell continuity measure `<= 1e-9`,
  seam mass flow of the relative flow within 2 % of the analytic `depth * integral (u_theta - Omega r) dr` (`u_theta = A r + B/r`), and seam face congruence
  `max |S_hi + R S_lo| / |S| <= 1e-12` over the pairs.
* C5b, fan sector mesh (example `cases/axial_fan_sector`, the S3 set-up of ADR 0065, no flow solve): the same congruence measure and the largest relative centre mismatch
  `|c_hi - R c_lo| / pitch-chord` over the seam pairs. Decision rule, fixed now: if C5a passes and C5b's congruence exceeds 1e-9, the cause is the seam geometry of the mesh
  generator (the periodic matcher `periodic_map` accepts centre mismatches up to `1e-6 * total_volume^(1/3)`); the defect is then fixed in the mesh generator (seam nodes of the
  upper seam made exact rotation images of the lower seam), and S3 is re-run once: pass limit `|m_in + m_out| <= 1e-6 |m_in|` (the registered S3 limit). If C5a fails, the cause is in
  the solver and is diagnosed in C5a with the individual terms (no tuning of any limit). If both are below 1e-9, the cause is open and is reported as such.

### C6 Published rotating-flow comparison (free rotating disk, von Karman 1921 / Cochran 1934)
Reference actually read: Wikipedia "Von Karman swirling flow" (secondary source, fetched 2026-10-18): similarity ODEs `2F + H' = 0`, `F^2 - G^2 + F'H = F''`, `2FG + G'H = G''`,
with `u = r Omega F`, `v = r Omega G`, `w = sqrt(nu Omega) H`, `eta = z sqrt(Omega/nu)`, `F(0)=0, G(0)=1, H(0)=0, F(inf)=G(inf)=0`; published numbers: inflow `w_inf = -0.886 sqrt(nu Omega)`
(attributed there to Cochran 1934) and torque of both sides `T = -1.94 R^4 rho sqrt(nu Omega^3)` (attributed there to Schlichting 1960). The primary papers (von Karman 1921, Cochran 1934,
Schlichting) were NOT read: those attributions are UNVERIFIED beyond the Wikipedia text. This is not the F-126 acceptance case (a public compressor), see the F-126 statement below.
* C6a (reference self-check, no CFD): the ODE solved here by shooting (RK4, Newton on `F'(0), G'(0)`, `eta_max = 12`). Limits: `|w_inf/sqrt(nu Omega) + 0.886| <= 1e-3`; the two-side torque coefficient
  `pi G'(0)` within 0.5 % of `-1.94`.
* C6b (rcfd): `annulus_3d`, `r_in = 0.2`, `r_out = 1.0`, `lz = 0.5`, `Omega = 10`, `nu = 0.01` (`delta = sqrt(nu/Omega) = 0.0316`, `Re = Omega r_o^2/nu = 1000`, laminar), inertial frame, bottom plane
  `zmin` a wall rotating at `Omega` (velocity expression), `inner` wall rotating at `Omega`, `outer` and `zmax` open (zero-gradient velocity, fixed pressure 0). Meshes (nx, ny, nz) =
  (24, 24, 40) and (32, 32, 56), uniform in z (the generator grades the radial direction only), `linear_upwind`, up to 6000 iterations, convergence required. Metrics at the ring of cells nearest
  `r = 0.6` (`r/r_o = 0.6`, away from the hub and the outer edge): (i) `max |u_theta/(Omega r) - G(eta)|` over the cells with `z <= 6 delta`, (ii) relative error of the axial velocity
  `w/(sqrt(nu Omega) H(eta))` at the cell nearest `z = 8 delta`. Limits on the finest mesh: (i) `<= 0.08`, (ii) `<= 0.10`. The coarse mesh is reported only. The finite radius (R/delta = 31,
  open edge, hub) is a modelling difference to the infinite-disk similarity solution and is part of the measured deviation.

F-126 statement, fixed now: the roadmap acceptance of F-126 is a public low-speed compressor or rotor case reproducing overall pressure ratio and efficiency within published scatter. No such
dataset has been read in this wave and none is attempted; F-126 stays partial and its acceptance NOT MET. C6 is a rotating-flow benchmark of the existing solver, not that acceptance.

### Code policy
Tests only unless a registered check shows a defect. Fixes are opt-in where a baseline could move; the mesh-generator fix of C5b is confined to `axial_fan_sector` seams.
Test files: `crates/rcfd-api/tests/rotating_foundation.rs` (C1, C2a, C4, C5b, C6), `crates/rcfd-fv/tests/rotating_foundation.rs` (C2b, C3, C5a). The decision rule for a miss: the
limit stays, the test is `#[ignore]`d as `*_known_miss`, and the measured value is recorded in the addendum.

## Amendments registered before the runs they govern (no limit changed)

Written after the first runs of C1-C5 and the first, failed C6 run, before C6c was run.
* Corrected set-ups (new tests, the old ones stay ignored): C2a and C4 add `solver.component_diagonal = "corrected"` (the ADR 0010 end-plane requirement that the
  registered set-up omitted; the first run of C4 (a) did not converge and showed the damping). Limits unchanged.
* C3: the registered convergence criterion (normalised momentum residual 1e-10) is ill-conditioned for a nearly uniform free stream (the residual stalls at ~6e-3 while the
  velocity change per iteration is 7e-7); the corrected test c3b converges on `max_du < 1e-11` and continuity < 1e-9, limits unchanged.
* C1a: the fluid at rest has no meaningful normalised residual either; the run uses the wave 9 criterion of `sources.rs` (2500 iterations, then the fields must change by < 1e-6
  of their scale over 200 more).
* C5: the decision rule predicted a mesh-generator cause for the seam imbalance. The measured congruence of the fan seam is 6.5e-14, so the rule gave "cause open"; the cause was
  then looked for in the flux itself, per pair (diagnosis test, no limit), and found in `SimpleSolver::iterate` (see the addendum). The fix is opt-in
  (`coupled_body_pair_mean`, off by default, switched on with the rotating-frame wall treatment). The S3 limit of ADR 0065 (1e-6) is re-run unchanged.
* C6a: the shooting solver is started from F'(0) = 0.51, G'(0) = -0.616 (a starting guess only, UNVERIFIED from memory; the first guess 0.5, -0.6 converged to a spurious
  root) and uses `eta_max = 8` (not 12); `w_inf` is read at eta = 7.5. Limits unchanged.
* C6b as registered (open top and outer boundary) did not converge in 6000 iterations on either mesh; it stays as an ignored known miss. C6c, the corrected set-up, is
  registered now: identical geometry and meshes, but the top plane `zmax` is a fixed velocity `(0, 0, w_inf)` with `w_inf = sqrt(nu Omega) H(7.5)` of the similarity solution
  (the published far-field axial velocity, the number checked in C6a) instead of an open boundary; the outer boundary stays open. Because the axial velocity is now prescribed,
  metric (ii) is replaced by the radial velocity: `max |u_r/(Omega r) - F(eta)|` over the same cells, limit `0.1 max F` (F is computed by the shooting solver). Limits on the finest mesh
  (32, 32, 56): G error <= 0.08, F error <= 0.1 max F, convergence (`tolerance_u = tolerance_p = 1e-7`, 6000 iterations); the coarse mesh is reported only.

## Addendum (measured values)

All runs are release builds, one run per set-up (no set-up was run twice to pick a favourable one; the corrected set-ups named in the amendments are new tests).
Tests: `crates/rcfd-fv/tests/rotating_foundation.rs` (6 tests: 3 pass, 3 ignored known misses) and `crates/rcfd-api/tests/rotating_foundation.rs` (18 tests: 10 pass, 8 ignored).

| Check | Measured | Limit | Result |
|---|---|---|---|
| C1a `rotating_frame`, rest in the inertial frame, 3 meshes | max abs(u_rel)/(Omega r_o) 1.0e-13, 4.9e-13, 3.3e-12; pressure error 1.7e-3, 5.2e-4, 1.4e-4 of range | 1e-3; 1e-2 | pass |
| C1b `mrf`, walls at Omega x r, 3 meshes | velocity 1.58e-2, 4.2e-3, 1.1e-3; pressure 3.14e-2, 1.92e-2, 1.06e-2 | 2e-2; 5e-2 | pass (the M1 pressure miss of ADR 0008 was on the interface mesh, a different set-up) |
| C2a Taylor-Couette, `rotating_frame`, full annulus (registered set-up) | not converged on 12 x 4 x 2 in 4000 iterations (end-plane correction of ADR 0010 missing) | converged | MISS of the set-up, ignored `c2a_taylor_couette_in_the_rotating_frame_mesh_sequence_known_miss` |
| C2a corrected (`component_diagonal = corrected`) | errors 9.76e-2, 2.61e-2, 6.76e-3 V_i; orders 1.90, 1.95 | finest <= 5e-3; orders >= 1.0, mean >= 1.5 | orders PASS; finest-mesh error MISS 6.76e-3 (ignored `c2a_corrected_finest_error_known_miss`) |
| C2b periodic sector (fv level) | errors 9.74e-2, 2.61e-2, 6.76e-3 V_i; orders 1.90, 1.95 | same | orders PASS; finest MISS 6.76e-3 (ignored `c2b_finest_error_known_miss`). Sector and full annulus agree to 4 digits |
| C3 Ekman (registered residual criterion) | not converged at N = 16 in 4000 iterations (res_u stalls at 6e-3 while max_du is 7e-7) | converged | MISS of the criterion (ignored), normalisation defect |
| C3b Ekman, converged on the velocity change | errors 4.04e-2, 1.30e-2, 3.58e-3 U_g (N = 16, 32, 64); orders 1.64, 1.86; max abs(w) 0.3227 against 0.3224 | finest <= 2e-3; orders; abs(w) within 5 % | orders and spiral amplitude PASS (the sign of the Coriolis force is right); finest error MISS 3.58e-3 (ignored `c3b_ekman_finest_error_known_miss`) |
| C4 frame invariance (registered, no `component_diagonal`) | inertial run not converged, error 3.06e-2 V_i; `rotating_frame` runs 6.4e-2, 2.1e-2 V_i | 1.5e-2 | MISS of the set-up (ignored) |
| C4 corrected: analytic error of runs (a)-(e) | 6.76e-3 V_i in all five | 1.5e-2 | pass |
| C4 corrected: absolute-velocity difference to the inertial run | mrf 1.0: 8.8e-4; rotating 1.0: 6.43e-3; rotating 0.5: 1.58e-3; mrf 0.5: 4.3e-4 V_i | 1e-2 | pass |
| C4 corrected: spurious radial/axial speed | inertial 1.41e-3, mrf 1.0 5.3e-4, rotating 1.0 6.43e-3, rotating 0.5 1.58e-3, mrf 0.5 9.7e-4 V_i | 1e-3 | MISS for the inertial and the two `rotating_frame` runs (ignored `c4_corrected_spurious_speed_known_miss`). Diagnosis: the `rotating_frame` value is a discretisation error, 7.9e-2, 2.4e-2, 6.4e-3 V_i on the three C2a meshes (orders 1.7, 1.9) |
| C5a analytic sector, seam, Omega = 0, 1, 5, 20 | abs(m_lo + m_hi)/abs(m_lo) 0, 0, 1.6e-16, 2.9e-15; continuity <= 1.6e-13; seam flow within 0.66 %, 0.31 %, 0.37 %, 0.004 % of analytic; congruence 2.4e-15 | 1e-10; 1e-9; 2 %; 1e-12 | pass: the solver is exact on an axisymmetric sector |
| C5b fan sector seam geometry | 644 pairs, abs(S_hi + R S_lo)/abs(S) 6.5e-14, centre mismatch 1.7e-13 | threshold 1e-9 | below the threshold: the mesh generator is NOT the cause |
| C5b fan sector stored flux per pair, before the fix | Omega 2: sum 1.1e-7, worst pair 1.8e-6; Omega 20: sum 2.4e-6, worst pair 9.5e-6 (max abs(phi) 3.8e-3, 2.7e-2) | diagnosis | real per-face non-antisymmetry |
| C5b after the fix | sum 6.7e-18 and 6.2e-17, worst pair 5.0e-18 and 4.8e-17; S3 abs(m_in + m_out)/abs(m_in) = 3.1e-7 | 1e-6 (ADR 0065 S3) | pass |
| C6a torque coefficient (both sides) | pi G'(0) = -1.9347 (G'(0) = -0.61585, F'(0) = 0.51012) | within 0.5 % of -1.94 | pass (0.27 %) |
| C6a axial inflow read at eta = 7.5 | -0.8744 | within 1e-3 of -0.886 | MISS (ignored): defective reading point |
| C6a corrected, eta_max = 12, read at 11 | -0.8840 | within 1e-3 of -0.886 | MISS (ignored): the solved far-field value differs from the published number as read by 2.0e-3; the secondary source rounds or misquotes, or its attribution is wrong; the primary sources were not read |
| C6b open top (registered) | not converged in 6000 iterations on (24, 24, 40) and (32, 32, 56) (res_u 3.6e-2, 9.2e-3); entrainment wrong | converged | MISS (ignored); the zero-gradient open top is the suspected cause, not tested separately; that run used the first (wrong-root) reference solution, the miss is the convergence |
| C6c top inflow of the similarity solution (the eta = 7.5 value -0.8744 sqrt(nu Omega) imposed), rcfd vs ODE at r = 0.6 | coarse: G error 0.0061, F error 0.0142; fine: G error 0.0067, F error 0.0083 (limit 0.0181); converged in 116 and 172 iterations; axial velocity at z = 8 delta within 0.5 % | G <= 0.08, F <= 0.1 max F | pass |

Defect found and fixed (C5): `SimpleSolver::iterate` added the body-force term of the face flux as `rAU_f f_f . S_f` with `rAU_f` equal to the owner value on every boundary
face. On the two faces of a periodic pair the owner values differ whenever the solution is not the same on both sides of the seam, so the Coriolis and centrifugal face terms made
`phi_lo + phi_hi` nonzero (S3 of ADR 0065: 3.5e-6 to 2.7e-5 of the seam flow; per face up to 3.6e-4 of the largest face flux). The fix uses the distance-weighted mean of the two cells of
the pair (the convention of `boundary_hby_a` and `coupled_gamma`) in the new field `coupled_body_pair_mean`, off by default; the API switches it on together with `wall_pressure_trapezoid`
(rotating-frame source), so every other case is bit-identical. Not an error of the mesh generator (deviation from the registered decision rule, which predicted that cause).

Deviations: see the amendments. The C2/C3 finest-mesh limits were registered without a prior mesh study and missed; the observed orders show the solver is second order in all three
formulations. The `rotating_frame` formulation needs `solver.component_diagonal = "corrected"` with symmetry end planes (ADR 0010); only `mrf` enables it by itself. This is a
usability finding, not changed here (it would move the rotating-channel behaviour). The ignored `s3_laminar_rotating_passage_known_miss` in `fan_sector.rs` now passes when run
(3.1e-7) and is left as it is.

F-126: the roadmap acceptance (a public low-speed compressor or rotor case reproducing pressure ratio and efficiency within published scatter) is NOT MET and was not attempted; no
compressor data was read. C6 compares the existing solver with a classical rotating-flow solution (von Karman disk) and passes with the corrected set-up; it is not the F-126 acceptance.

Unverified references: the von Karman similarity ODEs and the numbers -0.886 and -1.94 were read only from the Wikipedia article "Von Karman swirling flow" (secondary; its attributions to
Cochran 1934 and Schlichting 1960 are not verified; the primary papers were not read). The starting guess F'(0) = 0.51, G'(0) = -0.616 is from memory and only seeds the Newton iteration.
The Taylor-Couette and Ekman solutions are derived in this ADR from the equations. A search for a published critical Taylor/Reynolds number for the onset of Taylor vortices
(eta = 0.5) did not yield a verified primary value (a secondary snippet gave Re_c 68.2 to 68.4, not used); Marcus (1984, JFM 146, read on the Berkeley site) was read for its torque
statements but gives no table usable as a pass/fail number, so no Taylor-vortex comparison was made.
