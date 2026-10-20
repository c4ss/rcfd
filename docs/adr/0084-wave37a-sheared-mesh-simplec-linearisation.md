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
