# ADR 0111: Wave 44A, the linear stability of the FAN-01 frozen-limiter map, its attribution, an under-relaxed deferred correction, and the stop rule

Status: registered before any registered run. Limits and decision rules are fixed here and are not changed after a run; a miss becomes an
ignored `*_known_miss` test with the limit intact; a defective set-up gets an amendment written before the corrected run. Defaults stay
bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded. Clean-room: first principles only. Written at HEAD 2f1a3ce
(scratch clone `<scratch dir>`, target `<target dir>`). Parents: ADR 0084 (the Jacobian method), ADR 0086
(the FAN-01 family), ADR 0110 (the stall diagnosis), `docs/milestone-fan.md`. No agreement with measured fan data is a question of this ADR
or is claimed.

## Starting point (ADR 0110)

* The ADR 0086 runs stall in limit cycles. On the empty passage E3 the Barth-Jespersen factors psi flip every iteration; freezing psi
  converges E3 to 1e-12, but the frozen fixed point is not a Barth-Jespersen fixed point, and freezing diverges on E2 and E1.
* On the bladed B3, freezing psi at iteration 1000 makes the oscillation at the blade-root leading-edge cells grow (`max_du` 2.2 to
  16.8 m/s; cells c17, c665, c701, c341, c53; c17 touches the hub and the blade). First-order `upwind` converges B3 (S10). Falsified as
  causes: the pressure-velocity coupling, SST, the seam gradient, the linear-solver tolerance, outlet backflow, the explicit Coriolis source.
* ADR 0110 inferred, without measuring it, that the frozen-limiter map has an eigenvalue at or beyond -1 at those cells, and proposed to
  attribute it to the explicit `linear_upwind` (deferred) correction gain or to the least-squares gradient of the cell with two wall faces.
* `is_converged()` stayed false in S10 although `res_u` and `res_p` were below 1e-5 from iteration 1114; the conjunction in `sim.rs` then
  points at the SST criterion (`res_nut`, the largest relative change of the eddy viscosity per iteration, documented in
  `docs/case-schema.md` as part of the convergence test).

## What I already knew when writing this (rules c, d; disclosed)

Code written before this registration (a deviation from "ADR before code", stated here):
* `SimpleSolver::conv_gradient_hook` (default `None`): a closure that may change the (limited) convection gradient of each velocity
  component before the `linear_upwind` correction is assembled; `SimpleSolver::lagged_state` / `set_lagged_state`: read and restore the
  wall-pressure quantities the kernel lags from the previous iteration (`wall_grad`, `body_ratio`).
* `SimpleSolver::deferred_correction_relaxation` (default 1 = off) and `deferred_correction_state`: the fix candidate F-R below.
* The linearisation harness and the F-R switch arms in `crates/rcfd-api/tests/fan01_stall_w43c.rs` (section "ADR 0111"), and the analysis
  script `scripts/w44a_arnoldi_analysis.py` (written next, before any registered run).

Smoke runs (release, one each; no eigenvalue was computed):
* **Linearisation smoke** (`w44a_lin_smoke`): the B3 reference reproduces ADR 0110 (median `res_u` / `res_p` over 500-1000 1.089e-3 /
  4.87e-4; torque 1.849301 N m, Delta p_ts 46.643 Pa at 1000; the top cells c665, c17, c341 at the blade-root leading edge; at iteration
  1000 c17 has psi = 0 / 1 / 1). **M0**: the harness map evaluated at the iteration-1000 state reproduces the real iteration 1001 of
  `Simulation::step` in all 35 371 state entries bit for bit. **M2**: the map with tight linear solves evaluated twice is bit-identical;
  one evaluation costs 0.051 s. At iteration 1000 the default run has `res_nut` 0.158 and a largest per-iteration velocity change of 0.046 of
  the largest speed.
* **F-R smoke** (`w44a_fix_smoke`, beta 0.5 switched on at iteration 20, 20 more iterations): B3 and E3 finite, no guard trip, no other
  reading.

## Part J: the linearisation (method)

**The map F.** State x = (u_x, u_y, u_z, p of every cell, phi of every non-empty face): 35 371 entries on B3. F(x) = one
`SimpleSolver::iterate` from x with: psi frozen at the reference state (the ADR 0110 hook); the eddy viscosity `nu_face` that the real
iteration 1001 used; the lagged wall-pressure quantities of the reference state; the rotating-frame force evaluated from the velocity of x
with the case's `Sources` exactly as `Simulation::step` does (M0 checks this); the ADR 0110 S6 linear solves (relative 1e-10, absolute
1e-16 / 1e-18). The turbulence model is therefore frozen; ADR 0110 S4 falsified SST as the cause of the stall, and the frozen-psi growth was
observed with SST live (a difference this method does not see; stated).

**Reference state** x_ref = the B3 default state after 1000 iterations (the S1 switch state of ADR 0110). It lies on the default cycle and is
not a fixed point of F; J(x_ref) describes one iteration from there (check M3 tests whether it predicts the frozen iteration).

**The Jacobian** J = dF/dx is never formed. Matrix-free Arnoldi on the scaled operator D^-1 J D (ADR 0084 scales: velocity U = max(1,
max|u|), pressure max(max|p|, U^2), flux U times the largest face area; with the F-R state block its largest magnitude), J v by central
differences with step h times the entry scale, h = 1e-6, start vector sin(0.7071 i) normalised, m = 200 steps, modified Gram-Schmidt with one
reorthogonalisation. Ritz values and vectors from the Hessenberg matrix (`numpy.linalg.eig`, `scripts/w44a_arnoldi_analysis.py`).

**rho** of a Jacobian = the largest modulus among its converged Ritz values; **rho_neg** = the same over the Ritz values with negative real
part. A Ritz value is **converged** iff its residual estimate `h_{m+1,m} |y_m|` (y normalised) is at most 1e-2 |theta|.

**Checks of the method (a failed check makes the readings of that Jacobian NOT TESTABLE):**
* **M0** (harness = kernel): done in the smoke (above); repeated in every registered run. **M0'**: with the hold hooks H17, H5, Hall (below)
  F(x_ref) is bit-identical to the unhooked F(x_ref) (the held gradients are the actual ones at x_ref). GG17 changes F(x_ref) by design.
* **M1** (step size): J0 also with h = 1e-7: |rho(1e-6) - rho(1e-7)| <= 1e-3 in the scaled variables. Reported beside it: the smallest
  |phi_f| / (U A_max) on the internal faces of the n5 cells (below) against the largest relative step h (a central difference across a
  flux-sign kink is step dependent, ADR 0084 amendment 1).
* **M-A** (Arnoldi convergence): the leading Ritz value converged (above), and nested agreement: rho from the first 150 steps and from all 200
  differ by at most 1e-3.
* **Resolution**: with u0 = max(|rho(1e-6) - rho(1e-7)|, |rho_150 - rho_200|, the leading residual estimate), rho_0 > 1 counts only if
  rho_0 - 1 > 3 u0; otherwise J0 is NOT RESOLVED.
* **M3** (prediction of the frozen iteration): two trajectories of F (base settings, 20 iterations) from x_ref and from x_ref + eps D v (eps
  1e-6, v the real part of the leading Ritz vector of J0, unit scaled 2-norm). g = the geometric mean of ||d_k|| / ||d_(k-1)|| over k = 6..20,
  d_k the scaled difference of the two iterates. M3 holds iff |g - rho_0| <= 0.02, and, when the leading Ritz value is real and negative,
  the dot product of d_k and d_(k-1) is negative for at least 12 of k = 6..20.

**Localisation.** n5 = the five cells c17, c665, c701, c341, c53 and their internal-face neighbours. Reported for the leading Ritz vector:
the share of the scaled velocity-block energy in n5 and in the five cells, the five cells with the largest velocity energy, the block shares.

## Part J: predictions and falsifiers (registered)

| Id | Prediction | Falsified iff |
|---|---|---|
| P1 | J0 has rho_0 > 1, resolved, and its leading Ritz value has a negative real part (an oscillating mode; ADR 0110 saw period ratios 0.24-0.72) | rho_0 <= 1; or the leading value is real positive |
| P2 | the leading mode is localised: at least 50 % of its scaled velocity energy in n5 | below 20 % |
| P3 | attribution branch 3 below (the gain of the five leading-edge cells, not of c17 alone, not the gradient formula) | any other branch |

## Part J: attribution (registered)

Variants (each a J by the same method, m = 200, h = 1e-6, at x_ref):
* **H17**: the convection gradient of c17 (all three components) held at its x_ref value (no dependence on the iterate).
* **H5**: the convection gradients of the five cells held.
* **Hall**: every convection gradient held (the deferred correction becomes a fixed source; its dependence on the flux stays).
* **GG17**: the convection gradient of c17 replaced by psi_ref times the Green-Gauss gradient of the iterate (a different gradient formula,
  still explicit).

Reading per variant (ADR 0084): **drives** iff rho_v <= 1; also counted as drives when rho_0's mode is oscillating and every converged Ritz
value of the variant above 1 is real, positive and at most 1.005 (a near-neutral slow mode that the non-fixed reference can push just above
1). **Contributes** iff rho_v < rho_0 - 0.005 without driving; otherwise **does not drive**.

Branches, in this order of precedence:
1. GG17 drives: **the least-squares gradient formula of the cell with two wall faces**.
2. H17 drives, GG17 does not: **the deferred-correction gain at c17** (any explicit gradient there feeds the mode).
3. H5 drives, H17 does not: **the deferred-correction gain of the five leading-edge cells**.
4. only Hall drives: **a distributed deferred-correction gain**.
5. none drives: both candidates falsified; the instability does not run through the convection correction (the other explicit use of the same
   gradient, the Laplacian's non-orthogonal correction, is then named as the remaining candidate, not tested).

The attribution is made only if J0 passes M0, M1, M-A, resolution and M3. Otherwise it is **NOT TESTABLE by linearisation at x_ref**.

## Part F: the fix F-R (registered)

**F-R, under-relaxed deferred correction.** In each momentum component the correction source of every cell (the right-hand side of the
`linear_upwind` convection beyond the upwind one: the face corrections and the periodic-face central correction; the matrices are identical)
is `c = c_prev + beta (c(u) - c_prev)`; the first relaxed iteration takes c(u). At a fixed point c = c(u), so the fixed point is unchanged; the
Barth-Jespersen factors are recomputed every iteration as by default (no freezing). Single-cell model (first principles, ADR 0085 style):
with momentum relaxation alpha and an explicit gain g (correction change per unit change of the cell value, divided by the implicit
diagonal), the iteration is stable for g > -(2.2 - 1.1 beta) / (0.9 beta) at alpha = 0.9 (-1.22 at beta = 1, -3.67 at 0.5, -8.6 at 0.25,
-23 at 0.1); a negative (oscillating) explicit gain is what relaxation can stabilise.

**Choice of beta.** J(beta) = the Jacobian of the frozen-limiter map with F-R (state augmented by the relaxed source, reference value
c(x_ref)), for beta in {0.5, 0.25, 0.1} (`w44a_lin_relaxed`).
* Branches 1-4: beta* = the largest beta with rho(J(beta)) <= 1 (same special case as "drives"). If none: F-R is not eligible and no fix is
  added. In branch 1 F-R is still the candidate: it under-relaxes the only term through which the convection gradient of c17 acts.
* Branch 5: no fix.
* J0 NOT RESOLVED, P1 falsified, or the attribution NOT TESTABLE: F-R is run as an **unattributed candidate** with beta* = 0.25 through the
  same acceptance and is reported as not attributed.

Before the acceptance runs: a smoke of beta* on E3, E2, E1 and B3 (20 + 20 iterations, finite, no guard trip).

**Acceptance (switch arms as ADR 0110: 1000 default iterations, then F-R with beta*, 2000 more; windows the last 500 before the switch and
the last 500 of the arm; `rho_med`, `rho_du` as ADR 0110; probe = one more iteration with beta = 1 from the final state).**
* **Z1, the empty passage first, E3 then E2 then E1** (exact absolute swirl 0). Each member:
  (a) `rho_med <= 0.01` and `rho_du <= 0.01` (the residual medians fall 100-fold and the cycle stops);
  (b) `res_u` and `res_p` both below 1e-5 at some iteration of the arm;
  (c) the probe gives `res_u` and `res_p` <= 1e-5 (the converged state is a fixed point of the default Barth-Jespersen map, which the S1
  frozen state was not);
  (d) exact-swirl check, one-sided against the exact value 0: |L| <= L_c + max(0.005 L_c, 1e-4 N m) and the swirl energy <= E_c + max(0.005
  E_c, 0.01 Pa), with L_c, E_c the largest |L| and swirl energy over the 100-iteration samples of the default reference window (the stalled
  cycle's worst value; ADR 0086 cap-rule tolerances). The fix may not converge to a state with more spurious swirl than the stall shows.
  Reported, no limit: L and the swirl energy per member and their trend with refinement (ADR 0110 part L is undiagnosed).
* **Z2, B3, only after Z1 passes on all three members**: (a), (b), (c) as Z1. The torque and Delta p_ts are reported against ADR 0086
  without a limit (ADR 0110 amendment 5: the stalled values are values of a cycle).
* **Z3 defaults** (only if Z1 and Z2 pass): the case key `solver.deferred_correction_relaxation` (float in (0, 1], default 1; a located
  error outside (0, 1] and for any engine other than the steady SIMPLE / SIMPLEC one) absent and set to 1 bit-identical (u, p, phi on B3
  after 30 iterations, non-ignored); `baseline_hashes.rs` passes unchanged.
* Reported with every arm: the iteration at which the tolerance was first met, `is_converged()` at the end, and the data of rule G.

**Kept or removed (rule b, registered now).** `conv_gradient_hook` and `lagged_state` / `set_lagged_state` are diagnostics used by the
registered (ignored) tests and are kept, default `None` / unused; a non-ignored test checks that a no-op hook with relaxation 1 is
bit-identical to the default on E3 after 30 iterations. `deferred_correction_relaxation` and its state are kept only if Z1 and Z2 pass;
otherwise they are removed with the tests that need them (their numbers stay here).

## Part G: is the SST change criterion a defect of the stop rule? (registered)

Known when writing (disclosed): in S10 the residuals were below 1e-5 from iteration 1114 while a 0.015 m/s period-two cycle stayed at the
outlet-hub corner and `is_converged()` was false; the default B3 at iteration 1000 has `res_nut` 0.158.

Measured at the end of a run: c_u = max over cells |u^n - u^(n-1)| / max over cells |u^n| (the velocity's own per-iteration change in the
same max-norm, relative form as `res_nut`), and `res_nut`. A run is **evaluable** iff `res_u` and `res_p` are below the tolerances in each
of its last 500 iterations.
* **DEFECT** iff in an evaluable run c_u < 1e-5 and `res_nut` >= 1e-5 (the eddy-viscosity criterion blocks a velocity field that is
  stationary to the tolerance).
* **NOT A DEFECT** iff in every evaluable run c_u >= 1e-5 whenever `is_converged()` is false (the criterion correctly reports a state that
  is not stationary pointwise, which the normalised residuals average away); the 60-minute problem is then the cycle, not the stop rule.
* Consistency, reported: an evaluable run with c_u < 1e-5 and `res_nut` < 1e-5 must have `is_converged()` true.
Runs: **G1** = the ADR 0110 S10 arm again (B3, `upwind` from iteration 1000), plus every Z arm that is evaluable. If DEFECT, the opt-in fix is
registered in an amendment before its code.

## Run order and rules

J0, J0 with h = 1e-7, H17, H5, Hall, GG17 (`w44a_lin_base_and_attribution`), analysis; M3 (`w44a_lin_m3`); J(beta) (`w44a_lin_relaxed`),
analysis; the beta rule; the beta* smoke; Z1 E3, E2, E1; Z2 B3; G1; Z3. Each registered run once, at most two at a time, none over 60
minutes. Tests in `crates/rcfd-api/tests/fan01_stall_w43c.rs`, ignored when over 60 s.

## Not claimed

Agreement with the FAN-01 measurement; the accuracy of any converged functional (only the one-sided exact-swirl check); anything about
meshes finer than F1; the linear stability of the map with SST live.

### Unverified references

None used. The Arnoldi method and the single-cell relaxation model are first principles.

## Amendment 1 (written after J0, the attribution variants, M3 and J(beta) had run, before the beta* smoke and any acceptance run)

What I knew when writing it: the part J results below (each registered run made once). No limit or rule is changed; this amendment applies
the registered rules and fixes beta*.

* **Method checks.** M0: the harness map reproduces the real iteration 1001 in all 35 371 entries bit for bit (in each of the three runs that
  built the reference). M0': H17, H5 and Hall leave F(x_ref) bit-identical; GG17 changes it, as stated. M2: bit-identical. M1:
  rho(1e-6) 1.330555, rho(1e-7) 1.330553, |diff| 2e-6 (limit 1e-3), met; the smallest |phi_f| / (U A_max) on the n5 faces is 1.16e-6 against
  the largest relative step 1e-6 (no flux sign can flip). M-A: the leading Ritz value converged (residual estimate below 1e-50), rho from
  150 steps equals rho from 200 to 5e-15: met. Resolution: rho_0 - 1 = 0.33 against 3 u0 = 6e-6: resolved.
* **J0**: leading eigenvalue **-1.3306** (real), then +1.163, +1.112 +- 0.104i, -1.097, +1.047; 118 of 200 Ritz values converged. **P1
  holds.** The leading mode is NOT at the leading edge: 0.1 % of its scaled velocity energy is in the five cells and 0.2 % in n5, so **P2
  is falsified**. Its most energetic cells are c1234 (seam, r 0.1408, z 0.149, 59 deg), c365 and c401 (interior, r 0.1249, z 0.172, 38 and
  56 deg), c1250, c1233 (seam): hub-region cells in mid-chord, the region where the ADR 0110 S12 oscillation moved to. Velocity carries 98 %
  of the mode's scaled energy.
* **Attribution variants**: H17 1.330555, H5 1.330554, GG17 1.330555 (each changes rho by less than 1e-5: does not drive); Hall 1.291514
  (contributes: -0.039; the leading mode and its location unchanged, and Hall raises the positive eigenvalues to 1.284 and 1.229). By the
  precedence this is branch 5 (none drives).
* **M3 fails**: the two frozen trajectories separate at a geometric-mean rate of **1.017** over k = 6..20 (limit |g - rho_0| <= 0.02 against
  1.3306), and the difference changes sign in 5 of 15 steps (at least 12 required). The per-step ratio swings between 0.71 and 1.34. The
  linearisation at the cycle point x_ref does not predict the frozen iteration: the base trajectory moves away from x_ref (x_ref is not a
  fixed point, and J0 has four further eigenvalues above 1 with positive real part), so the Jacobian along it changes from step to step.
* **Reading (registered rule):** the attribution is **NOT TESTABLE by linearisation at x_ref**. What the measurement does show, reported
  without a verdict: at x_ref neither the convection-correction gain of the leading-edge cells nor the least-squares gradient of c17 carries
  the leading instability of the frozen map (removing either changes rho by less than 1e-5), and the leading mode lies at hub-region
  mid-chord cells, not at the leading edge where the frozen nonlinear run grows.
* **J(beta)** (reported; rho of the relaxed frozen map): beta 0.5: -1.3350; 0.25: -1.3384; 0.1: -1.3406 (all converged, nested agreement
  better than 1e-13). The under-relaxed deferred correction does not reduce the linear growth at x_ref, consistent with Hall: the leading mode
  does not run through the convection correction.
* **beta\*** by the registered rule for the NOT TESTABLE case: F-R is run as an **unattributed candidate with beta\* = 0.25**. J(beta)
  predicts no stabilisation of the frozen map; F-R runs with the live limiter, whose switching J(beta) does not model, so the acceptance runs
  decide. Nothing else changes: the smoke of beta\* on E3, E2, E1, B3, then Z1 (E3, E2, E1), Z2 (B3) only after Z1, G1, Z3.

## Addendum: measured values, verdicts, deviations

Release builds of the scratch clone at 2f1a3ce plus this wave's files; each registered run made once. The commits on main in the meantime
(ADR 0112, 0113) touch `simple.rs` only in a separate opt-in hunk (`coupled_offdiag_in_h1`, default off) and nothing else on the path of
these runs; this wave's hunks were applied onto main's current files.

### Part J (linearisation at x_ref; numbers in amendment 1)

| Check / prediction | Measured | Limit | Result |
|---|---|---|---|
| M0 harness = kernel | 35 371 of 35 371 entries bit-identical (each run) | bit-identical | met |
| M0' H17 / H5 / Hall | bit-identical | bit-identical | met |
| M2 statelessness | bit-identical | bit-identical | met |
| M1 step size | abs(rho(1e-6) - rho(1e-7)) = 2e-6 | 1e-3 | met |
| M-A Arnoldi | leading residual < 1e-50; abs(rho_150 - rho_200) 5e-15 | 1e-2 abs(theta); 1e-3 | met |
| Resolution | rho_0 - 1 = 0.33; 3 u0 = 6e-6 | rho_0 - 1 > 3 u0 | resolved |
| M3 prediction of the frozen iteration | g = 1.017; 5 of 15 sign changes | abs(g - 1.3306) <= 0.02; >= 12 of 15 | **MISS** |
| P1 oscillating unstable mode | -1.3306 (real) | rho > 1, Re < 0 | holds |
| P2 localisation in n5 | 0.2 % (five cells 0.1 %) | >= 50 % (falsified below 20 %) | **falsified** |
| P3 branch 3 | branch 5 by the numbers, but NOT TESTABLE (M3) | branch 3 | not met |

**Attributed: nothing.** M3 failed, so by the registered rule the attribution is NOT TESTABLE by linearisation at x_ref. Observations only
(no verdict): at x_ref, holding the convection gradient of c17, of the five leading-edge cells, or replacing c17's gradient by Green-Gauss
changes rho by less than 1e-5; holding every convection gradient lowers it to 1.2915; the under-relaxed correction (J(beta)) does not lower it
(1.335-1.341); the leading mode sits at hub-region mid-chord cells (c1234, c365, c401; z 0.149-0.172, 38-59 deg), not at the leading edge.
The linear map at a cycle point did not predict the frozen nonlinear iteration, and (below) it did not predict the live-limiter F-R runs
either. Why is not explained here.

### Part F (F-R, unattributed candidate, beta* = 0.25; switch at 1000, 2000 more)

| Arm | (a) rho_med / rho_du (<= 0.01 each) | (b) tolerance reached | (c) probe beta = 1 (<= 1e-5) | (d) exact swirl, one-sided | Result |
|---|---|---|---|---|---|
| Z1 E3 | 9.8e-9 / 2.8e-9 | iteration 1003 | 1.9e-13 / 5.4e-14 | abs(L) 0.009508 <= 0.009518 + 1e-4; E_s 6.26586 <= 6.26632 + 0.01 Pa | **met** |
| Z1 E2 | **0.103 / 0.036** | iteration 1004 | 6.5e-7 / 1.3e-6 | abs(L) 0.059611 <= 0.059612 + 1e-4; E_s 9.23040 <= 9.23040 + 0.01 | **MISS (a)** |
| Z1 E1 | 2.8e-8 / 3.1e-9 | iteration 1004 | 1.6e-12 / 2.7e-13 | abs(L) 0.105975 <= 0.105977 + 1e-4; E_s 5.48372 <= 5.48377 + 0.01 | **met** |
| Z2 B3 | not run (Z1 not met; registered order) | | | | - |
| Z3 | not applicable (no key added) | | | | - |

* E3 and E1 converge to `res_u` 2e-13 / 1e-12 and the probe shows the converged states are fixed points of the default Barth-Jespersen map
  (unlike the ADR 0110 S1 frozen state, whose probe returned to the stall level). `is_converged()` is true at the end of both arms. The
  outlet L and swirl energy are the stalled values to 1e-5 relative: F-R stops the cycle and leaves the spurious swirl unchanged (the growth
  of L with refinement, 0.0095 / 0.0596 / 0.1060 N m, stays undiagnosed, as in ADR 0110).
* E2 falls 82-fold in `res_u` and 9.7-fold in `res_p` (medians 3.8e-7 / 7.3e-7, below the tolerance from iteration 1004 on) but does not
  settle: a non-decaying drift of 1e-3 to 1e-2 m/s per iteration (period ratio about 2) remains at the shroud tip-layer cells (r 0.2495-0.2499,
  z 0.087-0.095, c13838, c13839 and neighbours). Their psi is constant between iterations, no flux sign flips, the wall faces carry flux of
  about 1e-16: it is not a limiter cycle.
* **Decision (rule b, as registered):** Z1 is not met, so F-R is removed: `deferred_correction_relaxation`, its state, `w44a_lin_relaxed`,
  the F-R smokes and the Z tests are deleted; their numbers are only here. Preamble rule 1 asks for a failed registered test to stay as an
  ignored `*_known_miss`; the registration of this ADR (part F, "kept or removed") took precedence because the tests cannot compile without
  the removed field. Stated as a deviation.
* The first iteration at which `is_converged()` turned true in E3 and E1 was not recorded (only the end value).

### Part G (the stop rule)

| Run | evaluable | c_u | `res_nut` | `is_converged()` | reading |
|---|---|---|---|---|---|
| G1 (B3, `upwind` from 1000; ADR 0110 S10 again) | yes | 3.5e-4 | 3.0e-2 | false | not stationary pointwise |
| Z1 E2 | yes | 1.9e-4 | 1.1e-4 | false | not stationary pointwise |
| Z1 E3 | yes | 3.3e-12 | 2.5e-11 | true | consistent |
| Z1 E1 | yes | 2.1e-11 | 1.5e-10 | true | consistent |

**NOT A DEFECT.** In every evaluable run where `is_converged()` was false the velocity itself still changed by 2e-4 to 4e-4 of the largest
speed per iteration; the SST criterion is the one test that sees a local cycle that the normalised residuals average away. Where the state
was stationary, the flag was true. No stop-rule change was made. The 60-minute problem of ADR 0086 symptom 2 is the cycle, not the stop rule.
G1 reproduces ADR 0110 S10 (tolerance at 1114, first-order torque 1.3353 N m, Delta p_ts -4.02 Pa).

### What is kept

* `SimpleSolver::conv_gradient_hook` (type `ConvGradientHook`) and `lagged_state` / `set_lagged_state`: diagnostics, default `None` /
  unused, registered to be kept; the non-ignored `w44a_hooks_at_defaults_are_bit_identical` (E3, 30 iterations, a no-op hook, u, p, phi bit
  for bit) covers the hook path. The registered relaxation part of that test was dropped with the field.
* The ignored tests `w44a_lin_smoke`, `w44a_lin_base_and_attribution`, `w44a_lin_m3`, `w44a_g1_upwind`; `scripts/w44a_arnoldi_analysis.py`.
* No case key, no schema change; defaults bit-identical by construction; `baseline_hashes.rs` not touched.

### Deviations (candid)

* The hooks, the harness and F-R were written before the registration commit (disclosed in "What I knew").
* Z1 E1 ran in parallel with Z1 E2, not after it (registered "E3 then E2 then E1"); no verdict depends on the order.
* The failed Z tests were deleted with the field instead of kept as ignored known misses (above).
* `arnoldi` carries `#[allow(clippy::approx_constant)]` to keep the registered start vector sin(0.7071 i).

### What remains

* The bladed stall: no mechanism attributed; F-R was not tried on B3. A linearisation at a fixed point (not at a cycle point) would need a
  converged frozen or relaxed state first.
* The E2 shroud tip-layer drift under F-R.
* F-R itself: it converged E3 and E1 to true Barth-Jespersen fixed points with unchanged functionals; a new registration (for example a
  longer arm, or E2 diagnosed first) would be needed to accept it.
* The growth of the spurious outlet angular momentum with refinement (ADR 0110 part L).
