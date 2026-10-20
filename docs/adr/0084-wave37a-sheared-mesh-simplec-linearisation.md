# ADR 0084: Wave 37A, the SIMPLEC instability on uniformly sheared meshes by linearisation of the outer iteration, and an opt-in fix if a mechanism is found

Status: registered before any run it governs and before any solver code of this wave was written. Limits are not changed, widened or deleted after a
run; a miss becomes an ignored `*_known_miss` test with the limit intact, a defective set-up gets a NEW corrected test and the old one stays ignored.
Baseline hashes are never re-recorded; every change is opt-in. Clean-room: first principles and published literature only. Written at HEAD 6f97dcb
(scratch clone `<scratch dir>`, target `<target dir>`).

Parent: ADR 0081 part X (wave 36B). Its outcome: SIMPLEC (alpha_u 0.9, alpha_p 1.0, `upwind`, no non-orthogonal correctors) on the uniformly sheared
channel B overshoots from 50 degrees and diverges from 55; the doubly periodic Kolmogorov box V7 (nu 0.5, cell Peclet 0.2, no wall, no open boundary)
is UNSTABLE from 60 degrees; lower `alpha_p` raises the channel threshold (0.7: 60, 0.5: 70, 0.3: > 79); the hypotheses H-X1 (SIMPLEC compact/wide
compensation) and H-X2 (omitted non-orthogonal pressure part, by its registered rule) were falsified; H-X6 "start-up only" stood on the evidence of V9 /
V9b (channel B at 60 degrees, nu 0.01: increments from the converged state decay, 0.982 and 0.889 per iteration). The mechanism was not identified.

## What was run before this registration (disclosed; rule (d))

Scratch files `crates/rcfd-fv/tests/w37a_smoke.rs` (not committed) and the harness `crates/rcfd-fv/tests/w37a_jacobian.rs` (its two smoke tests),
release build of 6f97dcb. Classification = the ADR 0081 one (1000 iterations, `SteadyGuard`, UNSTABLE iff trip, or max speed over 10..1000 above
3 U_ref, or continuity non-finite, or max continuity over 900..1000 above its value at 10).

* **SM1 V7 (40 x 10 on 4 x 1), alpha_p 1.0, default linear tolerances**: at 50, 55, 57.5 degrees the speed reaches 1.03-1.04 by iteration 20 and stays
  there; at 50 `res_u` falls slowly (1.2e-2 at 1000), at 55 and 57.5 it stays at 0.2-0.3 for 1000 iterations (bounded, not converging). At 60 the speed
  is 1.03-1.05 up to iteration 100, then grows (21.6 at 300, maximum 37.4 at 845); at 65 and 70 growth starts after 20-50 iterations. No guard trip.
  **So on V7 the growth starts from a state close to the converged one; it does not look like a start-up transient.** This contradicts the transfer of
  the ADR 0081 H-X6 reading (measured on channel B only) to V7.
* **SM2 the same with linear solves to 1e-13 relative** ("tight"): the same picture and threshold (60), the same numbers to two digits up to 100.
* **SM3 box size** (default tolerances, alpha_p 1.0): 10 x 10 on 1 x 1 and 8 x 8 on 1 x 1 converge (res below 1e-6) at every angle 50-70; 20 x 10 on
  2 x 1 is STABLE to 55, bounded but not converging at 57.5 (`res_u` 0.41 at 1000), unstable from 60. A box of length 1 admits x-wavenumbers 2 pi m
  only; length 2 also admits pi.
* **SM4 V7 alpha_p 0.7**: 55, 60, 65 STABLE (60: `res_u` 2.7e-2 at 1000, slow), 70 and 75 grow (from about iteration 50).
* **SM5 harness**: on a 6 x 3 box at 60 degrees the map (one `SimpleSolver::iterate` from a set state) is bit-identical on repeated evaluation
  (stateless); the central-difference Jacobian (n = 99) builds and the analysis script runs (at that non-converged state two eigenvalues near 1 with
  x-uniform velocity eigenvectors, 1.000089 and 0.99958: the mean-velocity modes are only neutral at a fixed point, see the deflation rule below).
* **SM6 converged states (tight solves) at 0.9/0.3**: channel B converges to res 1e-12 in 911 (55 deg), 887 (60), 836 (70) iterations; one base-map
  iteration (0.9/1.0) moves those states by at most 1.8e-14. V7 at 60 and 70 does not reach res 1e-12 within 20 000 iterations (a residual floor), but one
  base-map iteration moves the final state by 5.6e-16 and 4.4e-16 (max abs over the state): a fixed point to round-off.
* **SM7 channel B, tight against default tolerances** (alpha_p 1.0): identical classifications (45 STABLE, 50 and 52.5 overshoot to 6.2 / 14.3 U,
  55-60 trip at iterations 39-48).

## Method (registered)

**The map.** State x = (active cell velocity components, cell pressure, flux of every non-empty face); F(x) = one `SimpleSolver::iterate` from x.
Nothing else that `iterate` reads changes between iterations in these set-ups (no wall with a body force, no face jump, no driver, no pseudo time;
SM5). All linearised maps use the tight linear solves (1e-13 relative, 1e-16 absolute) so that F is a smooth function of x (SM2, SM7: same
classifications as the default tolerances). The Jacobian J = dF/dx is formed column by column by central differences with step h times the scale of the
column's type (velocity: max(1, max|u|); pressure: max(max|p|, U^2); flux: U times the largest face area), h = 1e-6. The eigenvalues are those of
D^-1 J D (D = the scales), by `numpy.linalg.eig` (`scripts/w37a_jacobian_analysis.py`).

**Neutral modes.** An eigenvalue with |lambda - 1| < 1e-6 whose eigenvector's velocity block is a mean velocity (|sum u_k| / sqrt(N sum u_k^2) > 0.9
for a component) is neutral (the free mean velocity of a doubly periodic box) and excluded; rho = the largest |lambda| of the rest. All excluded
eigenvalues are listed. Transient growth: max over k in {1, 2, 5, 10, 20, 50, 100, 200, 400} of ||J_d^k||_2, J_d = J with the neutral modes
projected out (spectral projector), in the scaled variables.

**Eigenvector description** (reported for the leading modes): energy fractions of the u, v, p, phi blocks; the dominant x-Fourier index m of the velocity
block (cells labelled (i, j) along and across the sheared rows; for the 40-cell row m = 0..20); the energy fraction in the seam rows/columns against its
uniform value; the pressure checkerboard index |sum (-1)^(i+j) p'| / sum |p'|.

**States.** x*: the 0.9/0.3 tight iteration (in this code the fixed point does not depend on alpha_p, nor on SIMPLE/SIMPLEC at the same alpha_u except on
periodic seams, see S below), stopped at res 1e-12 or 20 000 iterations, accepted as a fixed point of the base map iff one base-map iteration moves it by at
most 1e-12 (max abs, raw units; SM6). x0: the initial state (fields at rest, p = 0, flux from `SimpleSolver::new`). x_k: the k-th iterate of the base map
(tight) from x0.

**Set-ups.** K40: V7 as ADR 0081 (40 x 10 on 4 x 1, nu 0.5, body force nu k^2 sin(k y), k = 2 pi, exact u = (sin(k y), 0)). K20: 20 x 10 on 2 x 1. K10:
10 x 10 on 1 x 1. KS: K40 with nu = 50 (the same forcing law, so the same exact solution; cell Peclet 0.002: the Stokes limit). KC: K40 with `central`
convection. B: channel B of ADR 0081 (40 x 10, nu 0.01, inlet 1, walls, outlet p = 0). Base map: SIMPLEC, alpha_u 0.9, alpha_p 1.0, `upwind`, no
correctors (unless stated).

**Jacobians (each formed once):**

| id | set-up | angle(s), deg | state | map |
|----|--------|---------------|-------|-----|
| J1 | K40 | 50, 55, 57.5, 60, 65, 70 | x* | base |
| J2 | K40 | 60, 70 | x0 | base |
| J3 | K40 | 60, 65, 70 | x* | alpha_p 0.7 |
| J4 | K40 | 60, 70 | x* | alpha_p 0.5 |
| J5 | K20, K10 | 60 | x* | base |
| J6 | KS | 60 | x*, x0 | base |
| J7 | KC | 60 | x* (its own) | base with `central` |
| J8 | B | 50, 55, 60 | x* | base |
| J9 | B | 55 | x0, x1, x5, x10, x20 | base |
| J10 | K40 | 0, 30 | x* | base (controls without shear) |

**Checks of the method (registered; a failed check makes the readings that use that Jacobian NOT TESTABLE, reported as such):**
* M1 (step size): J1 at 60 and J8 at 55 repeated with h = 1e-7: relative Frobenius difference <= 1e-4 and |rho(1e-6) - rho(1e-7)| <= 1e-4.
* M2 (map statelessness): F evaluated twice from the same x bit-identical (as SM5) at every state used.
* M3 (prediction of the real iteration): for every K40 / B Jacobian with rho > 1.005: from x* + 1e-8 w (w: u_x of cell c multiplied by sin(c), mean
  velocity removed), the base map (tight); the geometric-mean growth of max|x_k - x*| over the iterations where it lies in [1e-7, 1e-3]: within 1 % of rho.
* M4 (agreement with the measured classifications): for every K40 angle / alpha_p pair with a classification (ADR 0081 V7 at 1.0: STABLE 45-57.5,
  UNSTABLE 60-79; SM4 at 0.7: STABLE 55-65, UNSTABLE 70-75): rho(J(x*)) > 1 iff UNSTABLE. A disagreement is reported with the pair (it would mean the
  linearisation at x* does not explain that run).

## Hypotheses (registered with their falsifiers; each reading below is final)

| id | hypothesis | stands iff | falsified iff |
|----|------------|------------|---------------|
| H1 | V7 is a linear instability of the converged state (not a start-up transient) | rho(J1, 60) > 1 and M3 holds for it | rho(J1, 60) <= 1 |
| H2 | V7 needs no convection (the Stokes part of the SIMPLEC iteration is unstable) | rho(J6, x*) > 1 and rho(J7) > 1 | rho(J6, x*) <= 1 |
| H3 | the box-size effect is the x-wavenumber of the unstable mode | the leading mode of J1 at 60 has m not a multiple of 4 and rho(J5, K10) <= 1 < rho(J5, K20) | its m is a multiple of 4 (allowed on the 1 x 1 box), or rho(K10) > 1 |
| H4 | channel B is a start-up instability: the converged state is linearly stable but the linearisation along the start-up is not | rho(J8, 55) <= 1 and rho(J8, 60) <= 1, and rho(J9) > 1 for some k | first clause: rho(J8) > 1 at 55 or 60 (then B is also a linear instability of x*); second clause: rho(J9) <= 1 at every k (then the start-up is transient growth; reported with ||J^k||) |
| H5 | the controls: no shear, no instability | rho(J10) < 1 at 0 and 30 degrees | rho(J10) >= 1 |

### Attribution switches (scratch diagnostics, fixed now; every one keeps the fixed point unless stated)

Formed at K40 60 x* and K40 70 x* with alpha_p 0.7, and at B 55 (x* and the first k of J9 with rho > 1, if any), only where the base rho > 1:

* **P(m) non-orthogonal part of the pressure correction, sub-iterated**: after the pressure solve, q = p_new - p_old; m times: the least-squares
  gradient of q (boundary values: those of p_new minus those of p_old), the face term n_f = rAtU_f k_f . grad(q)_f on internal faces (k = S - Delta, the
  non-orthogonal part of the face vector, as the existing correctors), and the pressure equation re-solved with phiHbyA - n; the flux is the corrected flux of
  phiHbyA - n with the last p_new (exactly divergence-free). n vanishes at a fixed point, so the fixed point is unchanged. m in {1, 2, 4}.
* **R cell-velocity correction reconstructed from the face correction**: the cell velocity is `HbyA - rAtU grad(p_old) + (1/V) sum_f s_cf dphi_f (x_f -
  x_c)` instead of `HbyA - rAtU grad(p_new)`, with dphi_f the part of the corrected flux that depends on q (internal: -rAtU_f lap_f (q_N - q_P); periodic:
  the coupled form; fixed-pressure faces: the boundary form; fixed-velocity faces: 0) and s_cf the outward sign. Zero at a fixed point.
* **S SIMPLEC compensation on periodic seam faces**: the term `(rAtU - rAU)_f snGrad(p_old) |S|` that internal faces get is added on coupled boundary
  faces too (the code adds it to internal faces only, so at a SIMPLEC fixed point the seam flux carries `rAtU` where internal faces carry `rAU`; found
  reading the code for this registration). S CHANGES the fixed point on periodic meshes; its Jacobian is formed at its own x* (0.9/0.3 with S).
* **alpha_p** 0.7 / 0.5 (J3, J4).

Reading (per switch; 0.005 is a protocol threshold): **drives** iff the switch brings rho to <= 1 where the base rho > 1; **contributes** iff it lowers
rho by more than 0.005 without reaching 1; **does not drive** otherwise. Also reported: the eigenvector description of the leading mode with and without
the switch.

## Fix (registered now; the choice is fixed in an amendment written after the diagnosis and before any acceptance run)

Candidates: P(m) (m <= 4) or R, as an opt-in solver key of the steady incompressible SIMPLE/SIMPLEC path (name and schema fixed in the amendment;
default value = the existing behaviour, bit-identical by construction), a located error elsewhere. **Choice rule**: a candidate qualifies iff it
"drives" at K40 70 x* alpha_p 0.7 AND at K40 60 x* alpha_p 1.0; of the qualifying ones the one with the smaller rho at K40 70 alpha_p 0.7 (ties: R, the
cheaper one); none qualifies: no fix, the outcome is documented. S is not a candidate (it changes the fixed point); if S "drives", it is reported as a
defect finding and proposed for a later wave.

**Acceptance (all required; run once on the frozen fix, default linear tolerances unless stated, the ADR 0081 classification unchanged, Theta = {45, 50,
52.5, 55, 57.5, 60, 65, 70, 75, 79}):**

| id | set-up | limit |
|----|--------|-------|
| A1 | K40, alpha_u 0.9, alpha_p 0.7, fix on, over Theta | theta_c >= 75 (every angle up to 70 STABLE) |
| A2 | channel B, alpha_p 0.7, fix on, over Theta | theta_c >= 75 |
| A3 | hold-out, never run before the fix is frozen: V7 on 30 x 8 cells on 3 x 1, alpha_p 0.7, fix on, over Theta | theta_c >= 75 |
| A4 | no regression: K40 and B at alpha_p 1.0 and 0.5, fix on, over Theta | theta_c(on) >= theta_c(off) per set-up (off: K40 1.0 = 60 and B 1.0 = 50, B 0.5 = 70 from ADR 0081; K40 0.5 off run in this check) |
| A5 | solution unchanged: K40 and B at 45, 60, 70 degrees, fix on at alpha_p 0.7 (tight solves) iterated until `max_du` <= 1e-13 (cap 20 000), against x* (fix off, 0.9/0.3, tight) | max abs velocity difference <= 1e-8 (U = 1); max abs pressure difference <= 1e-8 max(1, max abs p); K40: the RMS error of u_x against sin(k y) equal on and off to a relative 1e-6 (the error value reported). A fix-on run that does not reach `max_du` <= 1e-13 within the cap fails A5 at that angle |
| A6 | defaults | key absent and key = default bit-identical on the ADR 0077 A4 set-up (`cavity-simple`, 60 iterations: u, p, phi `to_bits`); `baseline_hashes.rs` untouched and passing; the full suite as at 6f97dcb plus this wave's tests |

Justification (rule (a)): A1-A4 are stability classifications (decision thresholds of the ADR 0081 protocol); A5 is a fixed-point identity, so its
limits are round-off / convergence-floor limits (1e-13 increments; 1e-8 leaves five decades for the conditioning of the fixed-point problem), plus the
comparison with the exact Kolmogorov solution required by rule (b). **Decision**: all of A1-A6 met: the key is kept opt-in and documented
(`docs/case-schema.md`). Any missed: the fix is REMOVED (rule (b)); its tests stay as ignored `*_known_miss` records with their limits.

## FAN-01 laminar oscillation (reported, not a gate)

The ADR 0083 P10 set-up (FAN-01-C, construction `camber_seam`, laminar, rotating frame, SIMPLEC, `upwind`; registered counts) through the case, with the
fix key on (if a fix exists after the acceptance), 300 and 1500 iterations, measured as ADR 0083 part A (c10, L over 200-300, L / c10, `res_u` and
`res_p` at 300 and 1500, largest relative speed). **"Converges"** (fixed now) iff `res_u` and `res_p` fall below 1e-4 within 1500 iterations; **P10 met**
iff L / c10 <= 1 over 300. Reference without the fix: ADR 0083 A0 (L / c10 8.36, `res_u` 0.35-0.43 throughout, not converging). If no fix is kept, the
best candidate (the chosen one before its acceptance) is run once on FAN-01 in the scratch tree and reported the same way, with the statement that it was
removed. Observation for the reading (SM1): the FAN-01 A0 signature (bounded, `res_u` 0.35-0.43 not falling) is also what V7 shows at 55-57.5 degrees.

## Not claimed

No accuracy claim for the non-orthogonal discretisation (the fix leaves the fixed point unchanged by construction). No fan performance claim. Wall
treatments and turbulence are not part of the linearisation.

## Process

Own scratch clone and target, one cargo at a time, two test threads; commits of own hunks only. `docs/case-schema.md` (the sheared-mesh paragraph, the
key if kept), `docs/milestone-fan.md` (robustness row). Final gate on a fresh `git archive` of the HEAD at hand-back.

### Unverified references

That neglecting the non-orthogonal terms of the pressure-correction equation leaves the converged solution unchanged but needs smaller pressure
under-relaxation as the non-orthogonality grows (Ferziger and Peric, Computational Methods for Fluid Dynamics, the chapter on complex geometries;
recalled, UNVERIFIED). The Kolmogorov-flow linear stability bound used by ADR 0081 for nu 0.5 (recalled there, UNVERIFIED).

## Amendment 1 (written after J1, J10 and the J1 checks ran, before J2-J9 were analysed and before any attribution switch ran; no limit changed)

What I knew when writing this: J1 (K40 x*): rho 0.9505 (50 deg), 0.9472 (55), 0.9919 (57.5), **1.0612 (60)**, 1.2375 (65), 1.4425 (70); from 57.5 deg
on the leading eigenvalue is real and NEGATIVE (-0.9919, -1.0612, -1.2375, -1.4425: a period-two oscillation), its eigenvector is 99 % pressure, x-Fourier
index m = 1 (the next, m = 2), no checkerboard; at 50-55 the leading mode is a slow positive one (m = 1, mixed u/v). Two neutral modes (both mean
velocities) at 1 +- 3e-9 at every angle. M2 held everywhere. M3: measured growth 1.0586 / 1.2287 / 1.4343 against rho 1.0612 / 1.2375 / 1.4425 (60 / 65
/ 70; within 0.25 / 0.7 / 0.6 %). J10: rho 0.9684 (0 deg), 0.9612 (30 deg). J10 ran in the same invocation as J1 (the selection `W37A_ONLY=J1` matched
`J10` as a prefix); it was a registered run and was formed once.

**M1, an ambiguity in my registration.** The registration says "relative Frobenius difference <= 1e-4" without saying in which variables. In the raw
variables J(h = 1e-6) and J(1e-7) at K40 60 differ by **4.26e-4 (above the limit)**; in the scaled variables in which every eigenvalue of this ADR is
computed (D^-1 J D) by **1.65e-5 (below)**; rho agrees to 1e-6 (1.061189 both). The difference is concentrated (65 % of it) in the columns of the
horizontal faces, whose flux is zero at x* (-1.9e-12): there the upwind face value switches sides, F is not differentiable, and a central difference
returns a step-dependent mix of the two one-sided derivatives. Reading (stated now, applied to every M1): M1 is evaluated in the scaled variables (the
space of the registered eigenvalues); the raw figure is reported beside it as a miss of the literal text. M3 (the real nonlinear iteration) is the
independent support of every rho > 1 reading.

**An additional fix candidate, P(implicit)** (added to the attribution switches and to the candidates of the choice rule, same rule, same acceptance):
the correction equation with its non-orthogonal part, (L_o + N) q = L_o q0 (q0 = the correction of the existing solve), solved by restarted GMRES (40
vectors, relative 1e-8, at most 400 iterations) in the form (I - L_o^-1 N) q = q0, each L_o^-1 a pressure solve to 1e-8 relative; then one L_o solve
with the final N q so that the corrected flux is exactly divergence-free. It keeps the fixed point (N q = 0 at q = 0). Why it is added: the sub-iteration
P(m) is the Richardson iteration of the same equation; on the continuous level its contraction factor for a smooth mode is the non-orthogonal fraction of
the operator, which for a uniform shear is at most sin(theta) (0.87 at 60 deg, 0.94 at 70; derived from the symbols of the orthogonal over-relaxed part
and of the cross-derivative term, first principles), so P(m <= 4) can leave most of the non-orthogonal part unconverged. Choice rule, extended: of the
qualifying candidates the one with the smallest rho at K40 70 x* alpha_p 0.7; ties to the cheaper (R, then P(m) with the smallest m, then P(implicit)).
Cost per outer iteration (pressure solves) is reported for every candidate.

## Amendment 2 (written after all registered Jacobians and attribution runs, before any acceptance run and before the key was wired; no limit changed)

### Diagnosis results (each Jacobian formed once; `crates/rcfd-fv/tests/w37a_jacobian.rs`, `scripts/w37a_jacobian_analysis.py`)

| Jacobian | rho | leading eigenvalue, eigenvector |
|---|---|---|
| J1 K40 x*, 50 / 55 / 57.5 deg | 0.9505 / 0.9472 / 0.9919 | 50, 55: +0.95, slow mixed u/v mode, m = 1; 57.5: -0.9919, pressure, m = 1 |
| J1 K40 x*, 60 / 65 / 70 deg | **1.0612 / 1.2375 / 1.4425** | negative real (period two), 99 % pressure, m = 1 (next m = 1, then m = 2), no checkerboard, not seam-concentrated |
| J2 K40 x0, 60 / 70 | 1.0586 / 1.4411 | the same pressure mode (the initial state of this nearly linear problem has the same linearisation) |
| J3 K40 x*, alpha_p 0.7: 60 / 65 / 70 | 0.9437 / 0.9528 / **1.1258** | 70: the same negative pressure mode |
| J4 K40 x*, alpha_p 0.5: 60 / 70 | 0.9566 / 0.9563 | a positive pressure mode (m = 13) |
| J5 K20 / K10 at 60 | **1.0471** / 0.9104 | K20: the negative pressure mode, m = 1 of its 20-cell row (= m = 2 of K40) |
| J6 KS (nu 50) x* / x0 at 60 | 1.0587 / 1.0586 | the same mode, 100 % pressure |
| J7 KC (`central`) at 60 | 1.0625 | the same mode |
| J8 B x*, 50 / 55 / 60 | 0.9103 / 0.9105 / 0.9105 | complex pair 0.910 +- 0.012i |
| J9 B x0 / x1 / x5 / x10 / x20 (55 deg; max speed 0 / 1.86 / 4.40 / 14.0 / 93.3) | **1.5836** / 0.9098 / **2.4169** / **20.84** / **11.22** | x0: negative, mean-velocity/pressure mode (m = 0); x5-x20: positive real velocity (v) modes |
| J10 K40 x*, 0 / 30 | 0.9684 / 0.9612 | slow positive mode |

Transient growth at x* (deflated, scaled): 50 deg max ||J^k|| = 59.9 (k = 1), 55: 64.8 (k = 1), decaying from k = 10 on; at 60 and above it grows
without bound (rho > 1). Neutral modes: two (mean u_x and u_y) at 1 +- 3e-9 at every K40 x*; none excluded at the non-fixed states.

Method checks: **M1** K40 60: scaled 1.65e-5 (raw 4.26e-4, see amendment 1), rho 1.061189 for both steps; B 55: scaled 4.8e-7, raw 7.0e-7, rho
0.910535 / 0.910551: met in the scaled variables. **M2** every state: stateless. **M3** 60 / 65 / 70 (alpha_p 1.0) and 70 (0.7): measured growth
1.0586 / 1.2287 / 1.4343 / 1.1170 against rho 1.0612 / 1.2375 / 1.4425 / 1.1258 (0.25 / 0.71 / 0.57 / 0.79 %): met. **M4** every formed K40 pair agrees
with its measured classification (alpha_p 1.0: 50-57.5 rho < 1 STABLE, 60-70 rho > 1 UNSTABLE; 0.7: 60, 65 STABLE / rho < 1, 70 UNSTABLE / rho > 1).

Readings (registered rules): **H1 stands** (V7 is a linear instability of its converged state, a period-two pressure mode; not a start-up
transient). **H2 stands** (Stokes limit 1.0587, central 1.0625: no convection needed). **H3 stands** (m = 1; K10 0.910 <= 1 < K20 1.047: the 1 x 1 box
cannot hold the mode). **H4 stands, both clauses** (channel B: x* stable 0.91 at 50-60; the linearisation along the start-up is unstable at x0, x5,
x10, x20; the ADR 0081 H-X6 reading is confirmed for the channel and refuted for V7). **H5 stands** (no instability without shear).

### Attribution (K40 60 x* alpha_p 1.0, base rho 1.0612; K40 70 x* alpha_p 0.7, base 1.1258; B 55 x0, base 1.5836)

| switch | K40 60 / 1.0 | K40 70 / 0.7 | B 55 x0 | reading (K40) | cost of one map evaluation away from x* |
|---|---|---|---|---|---|
| P(1) | 0.94376 | 0.94996 | 1.3181 | drives | 2 pressure solves |
| P(2) | 0.94375 | 0.94749 | 1.3455 | drives | 3 |
| P(4) | 0.94375 | 0.94785 | 1.6332 | drives | 5 |
| P(implicit) | 0.94375 | 0.94786 | 1.9409 | drives | GMRES (tens of solves) |
| **R** | **0.94331** | **0.94282** | **0.9514** | **drives** | 1 (+ one flux evaluation) |
| S (own x*) | 0.95207 | 1.06764 | - (no seam) | drives at 60 / contributes at 70 | 1 |

With P or R the period-two pressure mode is gone at K40; what remains (0.943-0.950) is the slow velocity mode of the alpha_p 0.7 / 0 deg maps. On the channel
start-up state x0 the P family leaves a positive mode concentrated at the boundary rows/columns (seam fraction 0.70-0.94), R does not.

**Mechanism (stated with the measurements above):** the outer SIMPLEC iteration on a uniformly sheared mesh has a period-two pressure mode whose
eigenvalue passes -1 between 57.5 and 60 degrees at alpha_p 1.0 (between 65 and 70 at 0.7); it lives in the Stokes part of the iteration (no convection,
no wall, no open boundary needed) and is the mismatch between the cell velocity correction (`rAtU` times the full Green-Gauss gradient of the pressure
change, tangential part included) and the face flux correction (the over-relaxed orthogonal part only): either making the face correction carry the
non-orthogonal part of the CHANGE (P) or making the cell correction carry only what the face correction carries (R) removes the mode, and neither changes
the fixed point. The missing SIMPLEC compensation on periodic seams (S) also moves it; it is a separate defect of periodic meshes (the converged seam
flux carries rAtU where internal faces carry rAU) and is reported for a later wave, not fixed here.

### The fix, frozen (choice rule: R and every P qualify; the smallest rho at K40 70 alpha_p 0.7 is R's, 0.94282; R is also the cheapest)

* Solver: `SimpleSolver::reconstructed_correction: bool` (default `false`: the existing path, bit-identical by construction). With `true` the cell
  velocity after the pressure solve is `HbyA - rAtU grad(p_old) + (1/V_c) sum_f s_cf (phi_f - phi_f(p_old)) (x_f - x_c)`, where `phi_f(p_old)` is
  the corrected flux evaluated with the old pressure (the same function, so the difference is exactly the part of the flux correction that depends on
  `p_new - p_old`; zero on fixed-velocity faces).
* Case key `solver.velocity_correction`: `"gradient"` (default, the existing behaviour) or `"face_flux"` (R). Any other algorithm than `simple` /
  `simplec` with the incompressible steady solver (engine `Steady`) and `face_flux` is a located error at `solver.velocity_correction`.
* The diagnostic switches P(m), P(implicit) and S are removed from the solver after this amendment's record commit (their numbers stay here).
* Acceptance A1-A6 as registered, with the fix = R. A6 adds: an invalid combination (`face_flux` with `pimple`, with `coupled`) is a located error.
  FAN-01 (reported): the P10 set-up with `solver.velocity_correction = "face_flux"`.

Note for the reading of A2: the channel start-up instability (H4) also has velocity modes far from x* (J9 x5-x20, positive eigenvalues up to 20.8,
after the flow has overshot); R was attributed at x0 only (registered), so A2 is not predicted by the diagnosis.

## Addendum: measured values, misses and deviations

Release builds, scratch tree on 6f97dcb (`<scratch dir>`); each registered run once. Limits are the registered ones, unchanged.

Outcome in one paragraph. The SIMPLEC divergence on uniformly sheared meshes is a period-two pressure mode of the outer iteration. The cell velocity
is corrected with `rAtU` times the full Green-Gauss gradient of the pressure change; the face flux is corrected with the over-relaxed orthogonal part
only. On strongly non-orthogonal faces the two disagree enough for the pressure update to overshoot (eigenvalue -1.06 at 60 degrees, -1.44 at 70,
alpha_p 1.0). The mode needs no convection, wall or open boundary. On the doubly periodic box (V7) it is a linear instability of the converged state,
so the ADR 0081 "start-up only" reading does not hold there. On channel B the converged state is stable (0.91) and the linearisation along the start-up
is not (1.58 at the initial state, 2.4 to 21 once the flow has overshot), so there it is a start-up instability as ADR 0081 found. Every
fixed-point-keeping change that makes the two corrections agree removes the mode. The cheapest one reconstructs the cell correction from the face-flux
correction; it is the opt-in key `solver.velocity_correction = "face_flux"`. It met every acceptance limit (V7, the channel and a held-out box stable to
79 degrees at alpha_p 0.7; no threshold fell at 1.0 or 0.5; the converged solution unchanged to 2e-9) and is kept. On FAN-01 the continuity over
iterations 200-300 stays below its value at iteration 10 (P10 met, L / c10 0.33 against 8.36), but the run does not converge by the registered reading
(the residuals stall between 1e-3 and 7e-3).

### Diagnosis

Amendment 2 holds the numbers (J1-J10, M1-M4, H1-H5, attribution). Readings: H1, H2, H3, H4 (both clauses) and H5 stand; none is falsified. M1 is met
in the scaled variables only (raw K40 60: 4.26e-4 against 1e-4; amendment 1). At the stable K40 angles the transient growth is at most 65 (k = 1) and
decays from k = 10 on.

### Acceptance of the fix R (`solver.velocity_correction = "face_flux"`)

| id | measured | limit | result |
|---|---|---|---|
| A1 V7 K40, alpha_p 0.7 | STABLE at every angle 45-79 (max speed 1.03-1.04; continuity over 900-1000 below 2e-13) | theta_c >= 75 | met (> 79) |
| A2 channel B, alpha_p 0.7 | STABLE 45-79, res 1e-6 in 141-420 iterations (max speed 1.17-1.40) | theta_c >= 75 | met (> 79) |
| A3 hold-out V7 30 x 8 on 3 x 1, alpha_p 0.7 | STABLE 45-79 | theta_c >= 75 | met (> 79) |
| A4 K40 1.0 / 0.5; B 1.0 / 0.5 | on: > 79 / > 79 / > 79 / > 79; off: 60 / **75** (measured here) / 50 / 70 | on >= off | met |
| A5 K40 45 / 60 / 70 | max_du <= 1e-13 at 387 / 322 / 407; max abs du 1.8e-9 / 1.2e-10 / 6.0e-12; max abs dp 1.7e-12 / 3.8e-12 / 2.0e-11 (max abs p 1.4e-14, limit 1e-8); RMS error of u_x against sin(k y) 2.372937e-2 on and off, relative difference 5.2e-9 / 3.2e-10 / 4.2e-13 | 1e-8; 1e-8 max(1, p); 1e-6 | met |
| A5 B 45 / 60 / 70 | max_du <= 1e-13 at 328 / 327 / 473; max abs du 1.1e-12 / 1.6e-12 / 1.9e-12; max abs dp 2.3e-12 / 1.2e-12 / 4.0e-12 | 1e-8 | met |
| A6 key absent vs `gradient` (`cavity-simple`, 60 iterations) | bit-identical u, p, phi | bit-identical | met |
| A6 `face_flux` with `pimple`, with `coupled`; an unknown value | located errors at `solver.velocity_correction` | located | met |
| A6 `baseline_hashes.rs`, full suite | see the final gate | unchanged | see below |

Decision (registered): A1-A5 met, A6 met by the tests above and the gate: the key is kept opt-in and documented (`docs/case-schema.md`). The fix-off
threshold of K40 at alpha_p 0.5 (75 degrees) was measured for the first time here.

### FAN-01 (reported, not a gate)

The ADR 0083 A0 set-up with `solver.velocity_correction = "face_flux"`, one 1500-iteration run (`w37a_fan01_with_face_flux_velocity_correction` in
`crates/rcfd-api/tests/fan01_p10_w36d.rs`; about 8 minutes):

* c10 6.92e-4, L (200-300) 2.29e-4, **L / c10 0.330: P10 met** (without the key 8.36, ADR 0083).
* `res_u` 8.3e-2 (iteration 10), 2.8e-3 (100), 1.2e-3 (300), 6.7e-3 (1000), 3.5e-3 (1500); `res_p` 4.5e-2, 7.4e-4, 6.8e-4, 1.0e-3, 7.9e-4.
* The argmax cells are not poor (f_poor 0). Largest relative speed 42.6 m/s (without the key 63-68).
* **"Converges": not met.** `res_u` and `res_p` are never both below 1e-4 within 1500 iterations; from iteration 100 on they stall between 1e-3 and 7e-3.

No FAN-01 Jacobian was formed: whether the A0 oscillation (`res_u` 0.35-0.43) is the same mode was not diagnosed; the key removes the P10 growth
and leaves a non-convergence about a hundred times smaller in `res_u`. Candidates for it are the 83-degree faces, the rotating-frame terms and the laminar
flow at Re 1.4e5; none was diagnosed.

### Deviations from the registered plan (candid list)

* M1's limit did not say in which variables. It is met in the scaled ones and missed in the raw ones (amendment 1, written after J1 and before the rest).
* P(implicit) was added as a candidate in amendment 1, after J1 had shown the period-two pressure mode; it did not win the choice.
* J10 ran in the J1 invocation (a prefix match of the selection); it was registered and formed once.
* The channel B attribution was formed at x0 only, the first start-up state with rho > 1. It was not formed at x*, whose rho is below 1; the
  registration asked for attribution only where the base rho > 1.
* The diagnostic switches P(m), P(implicit) and S were in the solver for the attribution runs (record commit 93d66ab) and were removed afterwards.
* The FAN-01 report used one 1500-iteration run for both the 300- and the 1500-iteration readings (the first 300 iterations are the same run).
* Finding S is reported, not fixed: the SIMPLEC compensation is missing on periodic seam faces, so a converged SIMPLEC seam flux carries rAtU where
  internal faces carry rAU. Fixing it changes converged results on periodic meshes and needs its own registration.

### Unverified references

As registered (Ferziger and Peric on the non-orthogonal pressure-correction terms; the Kolmogorov stability bound via ADR 0081). The sin(theta) bound
of amendment 1 is a first-principles derivation (symbols of the orthogonal and cross-derivative parts), not taken from a reference.

### Final gate

On a fresh `git archive` of 5ae198a (main with this wave's commits on top of a98e531, which contains the other wave-37 agents' commits up to it):
`cargo fmt --all -- --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo test --workspace --release --no-fail-fast`:
**1879 passed, 0 failed, 525 ignored** (wave 36: 1873 / 0 / 519; this wave adds 5 running tests, A1-A3 and the two A6 tests, and 6 ignored ones: the
harness smokes, the registered Jacobians, A4, A5 and the FAN-01 report). `baseline_hashes.rs` passed unchanged. No LNK1201. This paragraph is a
docs-only commit, not re-gated.
