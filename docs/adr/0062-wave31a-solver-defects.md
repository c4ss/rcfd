# ADR 0062: Wave 31A, solver defects found by the wave 30 validation (Van Albada overflow, cooler-cavity Ra 1e6 stall, cylinder Re 100 bisection)

Status: accepted. Everything below "Registered" was written before the code change or run it governs. Limits are not changed, widened, replaced or deleted
after a run; a miss stays an ignored `*_known_miss` test with the limit intact, a test set-up defect gets a NEW corrected test and the old one stays.
Deviations are disclosed in addenda at the end. Baseline hashes (`crates/rcfd-api/tests/baseline_hashes.rs`) and all existing tests stay bit-identical and are
never re-recorded; if a proven fix would change one, the work stops and is reported. Clean-room: published literature only. No relaxation factor is tuned to
make a case pass.

Parents: ADR 0060 (V5 cylinder, bisection list K1/K3), ADR 0061 (addendum 2 Van Albada, addendum 5 Ra 1e6 known miss).

## Registered

### 1. Van Albada limiter overflow (fix, user-approved in principle)
Defect (ADR 0061 addendum 2): `TvdLimiter::VanAlbada.psi` evaluates `(r^2 + r)/(r^2 + 1)`, NaN for r > ~1.34e154 (r^2 = inf, inf/inf), reached through the 1e-300
`safe()` guard of the gradient ratio; HLLC + Van Albada + RK3 then aborts at step 1 with "non-positive density or pressure".
Patch (proven in a scratch copy, ADR 0061): for r > 1e150 use the algebraically identical `(r + 1)/(r + 1/r)`, which cannot overflow in numerator or denominator;
for r = +inf (where that form is inf/inf) return the limit 1.0. For r <= 1e150 the expression is the unchanged one, so every result with finite
r <= 1e150 is bit-identical (all of the baseline hashes and existing tests; only inputs that previously gave NaN change).
Acceptance, fixed before the fix:
* F1 the ignored Sod record `b_hllc_vanalbada_rk3_known_miss` (registered limits B1 to B5 of ADR 0061 unchanged) run with `--ignored` either passes or is
  reported with the measured numbers (L1 of rho, u, p at n = 800, observed order); the file keeps the old ignored tests (note added), and new passing tests
  with the same limits are added (`b_hllc_vanalbada_rk3`).
* F2 limiter unit test: psi_VanAlbada(r) is finite for r in {1e150, 1e154, 1e300, f64::MAX, +inf}; for NaN the result is as before the fix (`NaN > 0` is false, so the
  function returns 0.0; the test asserts 0.0); psi is continuous at 1e150 (|psi(1e150) - psi(next float above 1e150)| <= 1e-12 and |psi(1e150) - psi(1.0000001e150)| <= 1e-12);
  TVD bounds 0 <= psi <= 2 and psi(1) = 1 on a grid of r from 1e-300 to 1e300 and psi = 0 for r <= 0; bit-identical to the unpatched formula on a grid of r <= 1e150.
* F3 `baseline_hashes` and the rest of `cargo test --workspace --release` unchanged.

### 2. Heated cavity Ra 1e6 (`a_ra_1e6_known_miss`) diagnosis
Observed (ADR 0061): 32 x 32 (Symmetric ratio 8, van Leer TVD for momentum and energy, SIMPLEC, alpha_u 0.8, energy relax 0.9, Pr 0.71) reaches the 40000-iteration cap
at residual 1.3e-2 (limit cycle); v_max over the sequence 220.642, 220.651, 220.559 (non-monotone, 0.04 % spread).
Questions: is the discrete steady problem at that mesh without a steady state, or does the SIMPLEC iteration stall on a state that exists; which element of the
discretisation (under-relaxation, convection scheme and limiter, coupling variant, steady versus pseudo-transient, initial condition, mesh grading) is responsible.
Method: a new ignored diagnostic test file `crates/rcfd-fv/tests/diag_cavity_ra1e6.rs` (prints only, no tuning, no existing file edited) runs one-factor-at-a-time
changes on the baseline case, each with the cap 40000 and the residual history recorded (final residual, minimum residual, and over the last 2000 iterations the
relative peak-to-peak amplitude of Nu_avg and v_max):
* D0 baseline reproduction (expected: not converged, residual of order 1e-2, v_max 220.642).
* D1 under-relaxation: alpha_u in {0.5, 0.3}, energy relax in {0.7, 0.5} (diagnostic only; to find out whether the stall is relaxation-limited, not to choose a pass setting).
* D2 convection scheme (momentum and energy together, then each alone): upwind; linear-upwind; TVD with minmod, Koren, Van Albada (smooth, after item 1), van Leer (baseline).
* D3 coupling variant: classic SIMPLE (alpha_u 0.7, alpha_p 0.3).
* D4 steady versus transient: `BuoyantPimple` (PISO, same schemes and mesh) marched from rest to t = 0.5 (the diffusive time unit; several convective times at Ra 1e6) with a Courant number
  of 0.5 on the scale v_max, checking whether the field becomes steady (max cell change of T and |u| per unit time below 1e-8 over the last 0.05 time units) or keeps oscillating.
* D5 initial condition for the steady iteration: conduction profile T = 1 - x with u = 0; the converged transient state of D4 (if steady); the stalled final state itself (restart), to test whether the
  iteration is trapped.
* D6 mesh grading and size: n = 32 with Symmetric ratio {2, 4, 6, 8, 10}; and n in {30, 31, 33, 36, 40} at ratio 8, to see whether the stall is specific to the 32 x 32 / ratio 8 mesh.
Decision rules, fixed before the runs. "No steady state of the discrete system" if the transient march of D4 with the baseline scheme does not become steady at two time steps (oscillation of
v_max at the probe above 1e-6 relative at the end). "Iteration stall on an existing steady state" if D4 reaches steady and the steady iteration started from that state stays converged (residual below 1e-9)
while the baseline start does not. A factor is named the root cause only if changing that factor alone turns the stall into convergence below 1e-9 within the cap; if several do, all are reported. The v_max
non-monotonicity is attributed by repeating the three-mesh sequence under the converging variant (if it becomes monotone, it belongs to the unconverged 32 x 32 member).
Fix policy: a fix is allowed only as an OPT-IN key that leaves every existing result bit-identical. If the cause is a property of the scheme (limiter switching) rather than a defect, no code is changed and the
ignored `a_ra_1e6_known_miss` with its limits stays the record. Acceptance of an opt-in fix (registered now): with the key enabled, the 32 x 32 case reaches residual < 1e-9 within 40000 iterations and the
ADR 0061 limits A0 to A6 for Ra 1e6 (unchanged) are met by a NEW passing test (the old known-miss test stays ignored); with the key off, `cargo test --workspace` is unchanged.

### 3. Cylinder Re 100: remaining registered bisection and independent dt / h study
ADR 0060 K-section: if K1 misses, bisection (i) fine mesh with dt halved (done: St 0.16175, C_D 1.3234), (ii) same dt with the next finer mesh, (iii) r_out = 30 D (done: St 0.15781, C_D 1.2961),
(iv) probe-position independence with a second probe at (2, 0, 0.1). Steps (ii) and (iv) are run now, with the set-up exactly that of `validation_cylinder_re100.rs` (PISO, linear upwind, alpha_u = alpha_p = 0.9,
n_outer 2, r_out 20, Expansion(150), t = 160, statistics from t > 60, 8 whole periods). Each run is executed at most twice.
New file `crates/rcfd-api/tests/validation_cylinder_re100_w31.rs` (ignored tests; the old file is untouched) with its own runner that records both probes. Runs:
* (ii) 181 x 141 (ratio sqrt 2 on the fine mesh), dt 0.05; and the same mesh with dt 0.025.
* (iv) 128 x 100, dt 0.05, probes (3,0,0.05) and (2,0,0.1) from the same run; criterion: St from the two probes agrees to 0.5 % (St is a global frequency).
* Independent time-step and mesh study (St and C_D) on the matrix meshes {64x50, 90x70, 128x100} x dt {0.1, 0.05, 0.025}, plus 128 x 100 at dt 0.0125.
Separation of errors: columns of fixed dt give the mesh error, rows of fixed mesh the time-step error. Observed orders in dt (rows) and in h (columns, refinement ratio sqrt 2 in n) are computed from
three-value sets (Roache GCI, safety factor 1.25); the combined model St(h, dt) = St_0 + a h^p + b dt^q is fitted by least squares on the nine matrix points; St_0 and the two separate error terms at the finest
mesh and the smallest dt are reported. Rule for the explanation, fixed before the runs: "dt-dominated" is stated if |St(fine, dt/2) - St(fine, dt)| exceeds the mesh change
|St(next finer mesh, dt) - St(fine, dt)| by more than a factor of 2 for dt = 0.05; otherwise the error is called mesh-dominated or mixed.
Limits unchanged: K1 St within 3 % of 0.165, K2 C_D within 4 % of 1.342, K3 GCI_fine of St and C_D <= 3 %, K4 at least 9 zero crossings. The old `k1_k3_strouhal_and_gci_known_miss` stays. Because the registered sequence
refined mesh and dt together (so its GCI mixes the two errors, a set-up defect of the sequence), a corrected sequence is registered now: the three matrix meshes at the common dt = 0.025 (spatial error only), with the
same K1 (finest mesh), K2 (finest) and K3 (GCI_fine of the spatial triple, refinement ratio sqrt 2) limits. If it passes it becomes a new ignored heavy test that passes (cost above the 5 minute guideline); if it misses it is a `*_known_miss`.

Order: this ADR is committed first (docs only); then the limiter fix with its tests and the Sod runs; then the diagnostic test files, then runs; then addenda with measured numbers.

## Addendum 1 (registered after D0 to D3 and before D7 and the corrected Ra 1e6 test; no number below was changed by it)

Measured so far (diagnostic file, prints only): D0 reproduces the miss (40000 iterations, residual 1.31e-2, v_max 220.6422, Nu 8.8581; last-2000 peak-to-peak 7e-4 in Nu). D1: alpha_u = 0.5 alone,
alpha_u = 0.3 alone, energy relax 0.7 alone and 0.5 alone all converge to residual < 1e-9 in 336, 581, 748, 1697 iterations. D3: classic SIMPLE (0.7, 0.3) converges in 250 iterations. D2: every convection scheme
(upwind, linear-upwind, minmod, Koren, Van Albada, MUSCL, and each of momentum or energy alone) stalls at residual 1e-2 with the same relaxation factors. So a steady state of the discrete system exists, the stall
does not depend on the limiter or on the scheme order, and it disappears when the lagged buoyancy-temperature loop is damped more (lower alpha_u or lower energy relax) or when SIMPLE is used.
The converged answer depends on alpha_u only at 5e-5 in Nu_avg and 3e-5 in v_max (8.87276, 8.87249, 8.87317; 220.4816, 220.4785, 220.4870): the known relaxation dependence of the Rhie-Chow face flux.
Open: v_max over the mesh sequence. With the converged 32 x 32 value 220.48 the sequence is 220.48, 220.651, 220.559, still non-monotone, so the unconverged member does not explain it. Registered now:
* D7 v_max extraction and sequence: with alpha_u = 0.5 (energy relax 0.9, everything else as the baseline) all of n = 32, 48, 64, 96, 128 at ratio 8 are converged (residual < 1e-9) and v_max is extracted three
  ways from the same fields (parabola through three cells as the validation test, the raw maximum cell value, and a cubic through the four cells around the maximum), with Nu_avg and the location. Rule fixed before the run:
  the non-monotonicity "belongs to the unconverged member" only if the converged n = 32, 64, 128 triple (same alpha_u) is monotone; "belongs to the extraction" if it is non-monotone for the parabola but monotone for the
  cubic or raw maximum; otherwise it is "a property of the discrete sequence outside its asymptotic range" (reported with the observed pattern over the five meshes).
* New corrected test `a_ra_1e6_alpha05` (ignored heavy, in `validation_natural_convection.rs`, old `a_ra_1e6_known_miss` kept): the set-up defect is the relaxation of the registered run (the iteration at alpha_u 0.8 does
  not converge on a mesh that has a steady state); the corrected run uses alpha_u = 0.5 for all three meshes and every other setting and EVERY limit A0 to A6 unchanged. alpha_u = 0.5 was the first value tried in D1 and 0.3 gives the same answer
  to 3e-5; it is not selected from a set by the outcome. Whatever the corrected test measures stays registered (a miss becomes a `*_known_miss`). No solver code is changed and no opt-in key is added (the existing
  `alpha_u` and `relax` keys already carry the remedy; the case-file default values are not changed).

## Addendum 2: measured results, item 2 (Ra 1e6 cavity, all numbers from `crates/rcfd-fv/tests/diag_cavity_ra1e6.rs` and `validation_natural_convection.rs`)

Deviations disclosed: (a) the first run of the diagnostic extracted v_max wrongly (the two cell rows next to y = 0.5 were not averaged, giving 8e14); fixed before any reported number, the D0/D1 run was repeated. (b) The registered
decision rule for D4/D5 had two outcomes (steady state reached and the steady iteration stays converged from it, or no steady state); the measured third outcome is described below and no rule was
re-worded. (c) D7 and the corrected test were registered in addendum 1 after D0 to D3 had been seen.

Baseline D0 (reproduced): 40000 iterations, residual 1.31e-2, v_max 220.6422, Nu 8.8581, peak-to-peak over the last 2000 iterations 7e-4 (Nu) and 5e-3 (v_max).
* D1 under-relaxation (one factor): alpha_u 0.5 converges (336 iterations, Nu 8.87276, v_max 220.4816); alpha_u 0.3 (581, 8.87249, 220.4785); energy relax 0.7 (748, 8.87317, 220.4870); energy relax 0.5 (1697, same); alpha_u 0.5 with relax 0.5 (1767, 8.87276).
* D3 classic SIMPLE (0.7, 0.3): 250 iterations, Nu 8.87280, v_max 220.4795; with (0.5, 0.2): 336 iterations, 8.87262.
* D2 convection scheme (momentum and energy both, and each alone, with the baseline relaxation): upwind, linear-upwind, minmod, Koren, Van Albada, MUSCL all stall at residual 0.9e-2 to 2.5e-2 with peak-to-peak 1e-3 to 5e-3
  in v_max (18 runs); no scheme or limiter converges.
* D4 transient (PISO, van Leer, same mesh, from rest, dt from Courant 0.5 and 2 on v_max 220): the flow becomes exactly steady: max|dT/dt| 6e-9 at t = 0.40 and 5e-10 at t = 0.5, max|du/dt|/v_max 1.4e-8 and 2e-9 (Courant 0.5; Courant 2: 6e-9, 1e-8, 6e-10, 1e-9),
  v_max 220.4968 (Co 0.5) and 220.4929 (Co 2), Nu 8.87298 and 8.87299. This agrees with the converged steady iterations of D1 (8.87249 to 8.87317, 220.478 to 220.487) to 3e-5 (Nu) and 7e-5 (v_max): a steady state of the discrete system exists, it is the one D1 converges to.
* D5 initial condition and restart: starting from the conduction profile, from rest, and from the stalled state again (second 40000 iterations) all give the same limit cycle (residual 1.31e-2, Nu 8.85813, v_max 220.6422). Starting the steady iteration with the baseline
  relaxation from the converged transient state (the true discrete steady state) does NOT stay there: it leaves it and ends in the same kind of limit cycle (residual 1.48e-2, Nu 8.86185). The steady state is a repelling fixed point of the
  baseline iteration map.
* D6 mesh: n = 32, ratio 2 and 4 converge (221 and 215 iterations; Nu 9.270 and 9.001); ratio 6, 8, 10 stall (8.8e-3, 1.3e-2, 2.2e-2); ratio 8 with n = 30, 31, 33, 36 stall (3.1e-2, 2.3e-2, 1.4e-2, 3.7e-3, the residual falls with n), n = 40 converges (330 iterations),
  as do 48 to 128. So the instability of the baseline iteration is not specific to one mesh; its strength grows with the wall clustering and decreases with the cell count, and it vanishes at 40 and more cells for ratio 8.
* D7 converged v_max sequence at alpha_u 0.5 (n = 32, 48, 64, 96, 128; Nu 8.87276, 8.84764, 8.83827, 8.83121, 8.82865): v_max by the parabola of the validation test 220.4816, 220.1378, 220.6504, 220.6298, 220.5589; raw cell maximum 220.4811, 218.8066, 220.3645, 220.0842, 220.4832;
  cubic through four cells 220.5909, 220.4631, 220.5593, 220.5769, 220.5722 (maximum at x = 0.0373 to 0.0377).

Root cause (item 2): the SIMPLEC iteration with alpha_u 0.8 and energy relaxation 0.9 and the lagged buoyancy-temperature coupling is linearly unstable on the 32 x 32 ratio 8 mesh (and, with weaker amplitude, on neighbouring meshes up to 36 cells), although the discrete
steady problem has a unique converged state (found by lower relaxation, classic SIMPLE, and the transient march). It is not a limiter or scheme effect (upwind stalls the same), not an initial-condition effect, and not the absence of a steady state. The "limit cycle at 1.3e-2" is
the nonlinear saturation of that iteration instability. The remedy is the relaxation of the iteration path, which does not change the converged answer beyond 5e-5. No solver defect is proven (no wrong equation, no NaN, no
non-physical result), so no code is changed and no opt-in key is added (the existing `alpha_u` and `relax` keys carry the remedy).
The v_max non-monotonicity (registered A5 check form: |Q_f - Q_m| < |Q_m - Q_c|): with the converged coarse member the registered A5 check on v_max passes (|220.5589 - 220.6504| = 0.092 < |220.6504 - 220.4816| = 0.169), so the registered miss was caused by the unconverged
32 x 32 member (220.642 instead of 220.482). The sequence is NOT monotone in the strict sense even when converged (220.482, 220.650, 220.559), and the five-mesh D7 sequence is not monotone with any of the three extractions; the parabola extraction scatters by up to 0.2 percent
(n = 48: 220.14) and the cubic by 0.06 percent, both larger than the asymptotic differences, so the non-monotonicity of v_max is the sampling of a peak in a graded boundary layer by cell-centre values (extraction error), not a statement about the discretisation order.

Corrected test `a_ra_1e6_alpha05` (ignored heavy, 322 s, all limits A0 to A6 unchanged, alpha_u 0.5): PASSES. n = 32, 64, 128 converge in 336, 1057, 3592 iterations (residual below 1e-9, hot/cold imbalance below 5e-11); finest Nu_avg 8.8286 (+0.326 %, limit 1.5 %), u_max 64.842 (+0.33 %, limit 2.0 %),
v_max 220.559 (+0.55 %, limit 2.0 %), Nu_max 17.568 (-1.99 %, limit 4.0 %), locations within 0.02, Nu_avg observed order 1.84, GCI_fine 0.053 % (limit 1 %), Richardson Nu 8.8249 (+0.28 %, limit 1.5 %). The old `a_ra_1e6_known_miss` stays ignored and still reports the unconverged run.

## Addendum 3 (registered before the probe below is run): step (ii) at dt = 0.05 on 181 x 141 does not complete

Facts so far (cylinder file of this ADR, item 3): the matrix cells and the corrected sequence completed (Addendum 4). The registered step (ii) run (181 x 141, dt 0.05, set-up of the wave 30B file) ran for 2 h 20 min without finishing (the first run had no progress output
and was stopped by me; a first 60 steps cost 0.22 s each, 2x the 128 x 100 cost as expected). The second run, with a progress line every 10 time units, printed t = 10, 20, 30 (|u|max 1.99, 1.71, 2.33; 42, 80, 119 s) and then nothing for more than 10 minutes of busy CPU (stopped by me): the step
slows down by orders of magnitude between t = 30 and 40 of the start-up transient. Both runs of that cell are used up (limit: two). Registered probe (prints only, one run, aborts by itself when a single step takes longer than 30 s of wall time or a value is not finite):
`probe_181x141_dt0050_slow_step` in `validation_cylinder_re100_w31.rs` prints from t = 28 on every 0.5 time units the largest speed, the largest Courant number of the new flux, the pressure-solver iterations of the step (`StepInfo::p_iters`), the continuity residual and the wall time of the step, to find out whether the run
blows up (growing |u|, Courant), or the pressure solver stalls (p_iters at the cap). Decision: if the probe shows blow-up the cell is reported as "the registered dt is unstable on the 181 x 141 mesh" (a stability limit of the PISO step, Courant number); if it shows pressure-solver
stalling it is reported as a solver-robustness finding, with no code change in this wave. In both cases the finer-mesh evidence is taken from the dt = 0.025 run of the same mesh (run once), and the dt-versus-h separation uses the matrix.
