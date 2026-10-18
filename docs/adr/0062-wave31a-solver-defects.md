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
