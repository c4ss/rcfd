# ADR 0043: Wave 25A, temporal and nonlinear accuracy misses (F-210 BDF2, F-212 solidification, F-215 G5)

Status: accepted. Everything under "Decisions" and "Registered checks" is written before any change of solver code in the
repository. Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and stays an
ignored `*_known_miss` test with the registered limit intact; a set-up defect gets a NEW corrected test and the old one stays. A
newly passing known-miss criterion gets a NEW passing test with the same registered limit; the old record stays ignored with a
note. Addenda (measured numbers, deviations) are appended at the end only.

## Scope and ordering disclosure

Three recorded misses are diagnosed, and fixed only where the cause is a genuine defect:

1. F-210 fractional-step BDF2 (ADR 0037 addendum): F2 BDF2 order 1.040 (limit [1.7, 2.3]), F3 ratio 2.479 (>= 3.0), F8 ratio 1.130 (<= 0.5).
2. F-212 enthalpy method (ADR 0035 addendum): Newton limit cycle at N = 480 (P2/P8), P1 successive orders, K2 ny = 20 at 1.0040e-2.
3. F-215 G5 (ADR 0035 addendum): per-side enthalpy drift 2.0e-12 / 1.8e-11 against 1e-12 at dt = 0.01.

Disclosure: exploratory diagnostics were run in a scratch copy of the tree (a `git archive` of HEAD, outside the repository)
BEFORE this ADR was written; no solver code of the repository had been changed. Their results shaped the decisions below and are
reported in the addendum as exploratory (unregistered) evidence, with the code not in the repository. What they showed, so that
the registered checks are not mistaken for blind predictions:

* F2 BDF2: replacing the lagged periodic-partner value by its BDF2-consistent extrapolation lowers the F2 errors by about 4 and
  gives order 1.5 at the registered pair; additionally removing the collocated face-pressure smoothing term gives order 1.98 to
  2.00 for five successive step sizes.
* G5: the stored stiff matrix is not column-conservative to the last bit; the predicted drift matches the measured one for side 0.

## Decisions

### D1 F-210: what is and is not changed

Candidates examined: (a) startup step, (b) non-rotational pressure-correction splitting error, (c) the intermediate-velocity
boundary condition, (d) BDF2 weights / extrapolation, (e) lagged periodic coupling, (f) the collocated pressure-smoothing term.

* (a) The first step is implicit Euler (local error O(dt^2), global O(dt^2)): second order, not limiting.
* (b), (c) The Taylor-Green vortex of F2/F3 is doubly periodic: there is no wall and no pressure boundary condition, so neither the
  non-rotational splitting error nor the intermediate boundary condition can act. The wall-bounded decaying mode already gives
  BDF2 order 2.07 (ADR 0037 addendum, `f2c`). The rotational pressure-correction form (Timmermans, Minev and van de Vosse 1996)
  therefore addresses nothing in these three checks and is NOT implemented in this wave; it stays a named gap (its benefit is the
  pressure and the near-wall velocity gradient, which no registered check measures).
* (d) The BDF2 weights `(1 + r, r^2/(1 + r))`, the effective pressure step `dt/c0` and the convective extrapolation are first
  principles correct: Euler passes, wall-bounded BDF2 is second order.
* (e) DEFECT. The periodic boundary condition (`bc.rs`, `Periodic`) takes the partner cell value from the CURRENT field, i.e. the
  old level `u^n`, and treats it explicitly in the implicit system at level `n+1`. For BDF2 this is an O(dt) consistency error of
  the diffusion and convection coupling across the periodic faces. The fractional-step predictor already extrapolates the
  convected velocity (`conv_cells`) but not the partner value of the coupled faces. Fix, only in `fractional.rs` and only when
  the BDF2 weights are active: the partner value of the coupled faces is the extrapolation `(1 + r) u^n - r u^{n-1}` (`r = dt/dt_old`),
  the same extrapolation as the convected velocity. Euler steps, the startup step and all non-periodic faces are untouched, so
  they are bit-identical. The shared periodic coefficient code in `bc.rs`/`field.rs` and the iterative PISO path are NOT edited
  (PISO with outer iterations removes the lag by iterating; recommended follow-up, outside this wave).
* (f) The face flux carries `dt (interp(grad p) . S - snGrad(p) |S|)` (Zang, Street and Koseff). It is O(dt h^2) by construction:
  at fixed h it behaves as a first-order-in-dt term. It is the price of the collocated coupling that ADR 0037 D3 chose (no Choi
  term) and is NOT a defect; it is not changed. The addendum reports the exploratory evidence (order 2.00 with the term switched
  off) and the consequence: the registered F2 pair at N = 32 may not reach 1.7 and F8 is unaffected.

F8 (Chorin form, Kim-Moin): no code change. The registered ratio compares step sensitivities of a STEADY state, which the Chorin
form pollutes by an O(dt) bulk error (ADR 0037 addendum); the check cannot measure a boundary-condition benefit. It stays a miss.

### D2 F-212: safeguarded nonlinear solve

Cause (from the 21A residual history): the Newton iteration on the piecewise-linear monotone `h(T)` in the temperature variable
has the apparent capacity jump 1 -> 251 -> 1; when a cell crosses the whole mush in one step the Newton direction overshoots, and
the backtracking of the current code ends by ACCEPTING the last trial (`s < 1/256`) without decrease, which allows a limit cycle.

Fix (`solidification.rs`, `PhaseChange::advance`): the existing loop is kept exactly as is. If it ends without convergence
(iteration cap or no accepted decrease), the step is restarted from the initial guess with a safeguarded Newton iteration:

* Newton direction `d = -J^{-1} r` from the same linearisation (`J = V C/dt + A`, SPD for the symmetric diffusion operator);
* line search on the potential `Phi(T) = sum V Psi(T)/dt + T.A.T/2 - b.T - V S.T` with `Psi' = h - h^n` (convex, strictly because
  `h' >= 1`), whose gradient is the true residual `r(T)`: the directional derivative `phi'(s) = d . r(T + s d)` is monotone in `s`
  for the symmetric operator and negative at `s = 0`; the step is `s = 1` when `phi'(1) <= 0`, otherwise the root of `phi'` is
  bracketed in `(0, 1]` and found by bisection (at most 60 halvings). `Phi` therefore strictly decreases every iteration and no
  cycle is possible. For non-symmetric corrections (explicit non-orthogonal term) the bisection still finds a sign change.
* convergence test and iteration cap unchanged.

Because the new branch is entered only after the existing branch failed, every step that converged before is bit-identical, and the
baseline hashes are not affected (no baseline case uses the phase change). `StepStats` gets no new field (struct literals exist
elsewhere); the new counter is a public field of `PhaseChange` (`safeguarded_steps`), which has no struct-literal construction
outside `PhaseChange::new`. The Voller-Swaminathan linearised-source update was considered and rejected: its fixed-point map is a
contraction only for `(L/c)/(T_l - T_s) (V/dt) (V/dt + A)^{-1} < 1`, here `L/(c eps)` = 500, so it needs relaxation.

P1 order: NOT claimed fixed. The error is 0.1 to 0.3 percent; the diagnosis is a decomposition of the metric (see checks S5).
K2 ny = 20: diagnosed as discretisation error by comparison with the exact 1-D discrete solution (check S6); no code change.

### D3 F-215 G5

Hypotheses: (i) conditioning / round-off of the stiff solve, (ii) unbalanced flux accumulation order, (iii) inexact interface flux
use. The interface face conductance is exactly zero (asserted), which removes (iii). The measurement of (i) against (ii): the exact
(compensated) column sums of the assembled transient matrix against the capacities. In exact arithmetic the drift of a side is
`-sum_j (colsum_j - cap_j) x_j / ...` for an exactly solved system; a non-zero column defect of the STORED matrix (the diagonal is
`cap + sum k` rounded once, size ulp(diag) ~ eps Fo cap) would give a drift that scales with the Fourier number and is
independent of the linear tolerance. A fix would need an extended-precision (or flux-form, compensated) assembly and residual with
iterative refinement; whether to implement it depends on the result of the diagnosis: if the cause is double-precision
representation of the stiff matrix, it is documented as not a defect and no code changes.

## Registered checks

All limits below are fixed now. "Bit" checks compare `f64` bit patterns and have limit 0 differing bits.

### F-210 (tests in `crates/rcfd-fv/tests/fractional_step.rs`)

* **A1 Euler and non-periodic paths bit-identical.** Fingerprint (rotate-xor-multiply over `u`, `p`, `phi` bits) of the Euler
  Taylor-Green run (n = 32, nu = 0.05, dt = 0.02, to t = 1) equals `8c2d50e6a8e9b9ee`; fingerprint of the BDF2 cavity run (16 x 16,
  nu = 0.01, lid 1, linear upwind, incremental Kim-Moin, dt = 0.05, to t = 1) equals `e8bd7eaeefd7048b`. Both values were taken from
  the code before the fix. Limit 0 bits. Also every existing test of `fractional_step.rs` that passed keeps passing and
  `crates/rcfd-api/tests/baseline_hashes.rs` passes unchanged.
* **A2 F3 spatial ratio with the fix** (same set-up as `f3_known_miss_spatial_order`): e16/e32 >= 3.0 (the registered limit).
  Passing is recorded in a NEW test; the old test stays ignored with a note.
* **A3 F2 temporal order BDF2 with the fix** (same set-up as `f2_known_miss_temporal_order_bdf2`): order in [1.7, 2.3]. Expected
  to be at risk (D1 (f)); if it misses it stays the ignored record with its limit intact and the measured value is reported.
* **A4 improvement evidence (limit fixed here, not derived from a theory):** at the F2 set-up the errors against the dt/32
  reference must be at most half of the pre-fix values: e(0.1) <= 4.6e-3 (pre-fix 9.256e-3) and e(0.05) <= 2.25e-3 (pre-fix 4.502e-3).
  This only shows that the defect was acting; it does not replace A3.
* **A5 F8:** no change, no new claim.

### F-212 (tests in `crates/rcfd-fv/tests/solidification.rs`)

* **S1 N = 480 invariants.** The registered P2 and P8 set-ups (`stefan_case` with N = 60, 120, 240, 480): at every level P4 balance
  <= 1e-10, P5 range and monotonicity <= 1e-10, P6 all steps converged with fewer than 100 Newton iterations, and the position error at
  N = 480 <= 3 percent (the registered acceptance applied at the finest level too). The successive-order criterion >= 0.9 of the
  original P2/P8 text is reported; it is NOT asserted in the new tests and the old ignored tests keep it, because the N = 240
  error (6.3e-5) is an accidental near-cancellation, so the criterion is expected to fail at 240 -> 480 whatever the solver does.
* **S2 no regression, bit-identical where the old loop converged.** `PhaseChange::safeguarded_steps == 0` after every run that
  converged before (P1 all four levels, P2/P8 at 60, 120, 240, P3), and the tree tests `phase_change_case.rs` pass unchanged
  (tree equals library: difference 0.0).
* **S3 the safeguarded branch is itself correct.** With the branch forced (a public flag `force_safeguard` of `PhaseChange`,
  default false), P2 at N = 60, 120, 240: P4 <= 1e-10, P5 <= 1e-10, P6 converged below 100 iterations, and the interface position
  agrees with the default branch to relative 1e-8.
* **S4 the cycle was reproduced.** The default branch at N = 480 enters the safeguarded branch (`safeguarded_steps > 0`) for P2 (this
  documents that the cycle existed; it is not a pass criterion of the physics).
* **S5 P1 order: honest decomposition, no fix claimed.** Reported, unasserted: the successive differences between the numerical
  positions |s_60 - s_120|, |s_120 - s_240|, |s_240 - s_480| and their orders. If these differences contract at order >= 0.9 the
  scheme converges and the registered metric (error against the analytic position) is limited by a grid-independent offset of the
  reference (mush width, initial state sampled from the analytic profile at t0); the registered P1 order test stays an ignored miss.
* **S6 K2 diagnosis.** The exact solution of the 1-D discrete problem (second-order finite volume, wall at half a cell, same sink)
  agrees with the converged SIMPLEC solution to relative 1e-9 at ny = 20, 40, 80; the centre-normalised error of the discrete
  solution at ny = 20 is then the discretisation error. If both hold, K2 is a truncation-error miss and no code changes; the ignored
  record stays.

### F-215 (exploratory evidence in the addendum; no registered code change)

* **G1 diagnosis falsifiable form:** the compensated column-sum defect of the assembled matrix predicts the side-0 drift at
  dt = 0.01, 1e-3, 1e-4 within a factor 1.5 of the measured one (2.0e-12, 2.5e-13, 1.8e-14). If it does not, the "representation of
  the stored matrix" explanation is withdrawn.
* **G2** The registered G5 limit 1e-12 is not changed; `g5_open_connection_transient_conserves_each_side_known_miss` stays ignored
  and `g5_v2` stays as is. No library change unless a proven, baseline-neutral fix exists; the energy sweep is shared by all
  scalar transport and its bits would change under any refinement, so a refinement could only be opt-in.

## Shared files, baselines, process

Edited: `crates/rcfd-fv/src/pimple/fractional.rs`, `crates/rcfd-fv/src/solidification.rs`, the two test files, this ADR,
`docs/reference-data.md` and `docs/roadmap.json` (only where status genuinely changes). Not edited: segregated SIMPLE/SIMPLEC, the PIMPLE
momentum-source plumbing, `bc.rs`, `field.rs`, `scalar.rs`, `energy.rs`, `docs/ROADMAP.md`. Clean-room: first principles and
published literature only. Cited and NOT read in this session (attribution from memory, unverified): Timmermans, Minev and van
de Vosse 1996 (rotational pressure correction); Kim and Moin 1985; Zang, Street and Koseff 1994; Voller and Swaminathan 1991.

## Amendment 1 (written after the runs of S1 to S5 and before any run of S7; nothing above was changed)

Status of the runs so far, so that the order of events is on record: the safeguarded branch (D2) is committed with the tests S1 to
S5 above; those tests ran, S1 to S4 pass, and the S5 report FALSIFIED the hypothesis of check S5 ("the successive position
differences contract at order >= 0.9"): the differences of the P1 positions at N = 60, 120, 240, 480 are 8.8e-4, 1.1e-4, 1.3e-3
(orders 3.0 and -3.6). The extra report line shows why: the liquid content of the INITIAL state (cell-centre sampling of the
analytic profile, a sharp front inside one cell) differs from the exact position at t0 by 6.04e-3 (2 percent of s(t0)) for
N = 60, 120 and 240 and by 1.69e-3 for N = 480, and the final position errors (1.9e-3, 2.8e-3, 2.7e-3, 1.35e-3) follow it. Because the
enthalpy balance is exact (P4) any initial content error is a persistent offset of the position. This is a SET-UP defect of P1, P2
and P8 (the ADR text says only "cell-centre values; disclosed"), not a property of the scheme.

* **S7 corrected set-up (new tests; the old tests are kept unchanged).** Initial cell enthalpy = cell average of the sharp-front
  analytic enthalpy `H(x) = T_exact(x) + (L/c) [x < s(t0)]` (melting orientation; the freezing mirror image has `L/c - H`) taken with
  200 midpoint sub-samples per cell, inverted to the cell temperature through the piecewise-linear law (below the solidus
  `T = h`, in the mush linear, above the liquidus `T = h - L/c`). Then the unchanged registered limits of P1, P2, P8 apply to
  N = 60, 120, 240, 480: P4 <= 1e-10, P5 <= 1e-10, P6 below 100 iterations, position error at N = 240 <= 3 percent, successive
  orders >= 0.9. The outcome is not known in advance; whichever criterion fails is reported as measured and stays a miss (a new
  ignored `*_known_miss` record for the corrected set-up), and the P1/P2/P8 order claims are made only for criteria that pass.
* **S5 replaced as a diagnosis, not as a criterion:** the offset of the initial liquid content is the reported explanation of the
  non-monotone P1 errors; S7 is its test (if S7 P1 orders still fail, the explanation is incomplete and is reported as such).

## Addendum: results and deviations (written after the runs; no registered limit was changed)

Commits: ADR `139bdc2`, fractional fix `68527ac`, ADR amendment `bcc591a`, safeguarded Newton `39459a8`, S7 tests and S6 check follow
in the log. Release builds; tests in `crates/rcfd-fv/tests/fractional_step.rs` and `crates/rcfd-fv/tests/solidification.rs`.

### F-210 (D1)

| Check | Measured | Limit | Result |
| --- | --- | --- | --- |
| A1 fingerprints, Euler Taylor-Green and BDF2 cavity | `8c2d50e6a8e9b9ee`, `e8bd7eaeefd7048b` (identical to the pre-fix values) | 0 bits | met |
| A2 F3 ratio e16/e32 (dt 0.005), new test `f3_v2_...` | e16 1.219e-3, e32 3.084e-4, ratio 3.954 (pre-fix 2.479) | >= 3.0 | met; the old `f3_known_miss_spatial_order` now also passes when run and keeps its note |
| A3 F2 BDF2 order, registered pair | e(0.1) 2.136e-3, e(0.05) 7.485e-4, order 1.513 (pre-fix 1.040) | [1.7, 2.3] | MISSED, `f2_known_miss_temporal_order_bdf2` stays ignored |
| A4 error reduction | 2.136e-3 and 7.485e-4 against pre-fix 9.256e-3 and 4.502e-3 | <= 4.6e-3, <= 2.25e-3 | met (4.3 and 6.0 times lower) |
| A5 F8 (no change) | ratio 1.130 | <= 0.5 | MISSED, unchanged, `f8_known_miss_kim_moin_effect` |

Baseline hashes: the change is confined to the fractional-step BDF2 path with periodic faces, so no baseline case is affected
(`baseline_hashes.rs` is part of the final gate; result in the closing note below).

Root cause, with evidence (exploratory runs in a scratch copy, unregistered, code not in the repository; reference step dt/128 of
the same scheme, 32 x 32, nu = 0.1, t = 1, BDF2, incremental, partner lag removed in both columns):

| dt | error | order | error, pressure-smoothing term also off | order |
| --- | --- | --- | --- | --- |
| 0.2 | 8.42e-3 | | 9.58e-3 | |
| 0.1 | 2.16e-3 | 1.97 | 2.42e-3 | 1.98 |
| 0.05 | 7.91e-4 | 1.45 | 6.07e-4 | 2.00 |
| 0.025 | 3.99e-4 | 0.99 | 1.52e-4 | 2.00 |
| 0.0125 | 2.09e-4 | 0.93 | 3.80e-5 | 2.00 |

* Defect, fixed: the explicit, old-level periodic partner value is an O(dt) coupling error; extrapolating it like the convected
  velocity gives 4 to 6 times lower errors. This is the "periodic coupling" hypothesis of the 21C addendum, now isolated by a change.
* Not a defect, not changed: the collocated pressure-smoothing face term `dt (interp grad p - snGrad p |S|)` is O(dt h^2), so at
  fixed h the error floors at first order in dt (right columns: exact second order without it). With the term, the registered pair
  dt = 0.1, 0.05 at N = 32 gives 1.51, below 1.7. This is the price of ADR 0037 D3 (no Choi term); removing or lagging the term
  would change the steady state and the collocated coupling, and is not done in this wave.
* Startup (Euler first step), BDF2 weights, the effective pressure step and the intermediate boundary condition are cleared: the
  wall-bounded BDF2 order stays 2.074 (`f2c`) and the Euler and non-periodic runs are bit-identical (A1).
* Rotational pressure correction: not implemented. The periodic vortex has no pressure boundary, so the splitting error it removes
  cannot be responsible for F2/F3; no registered check measures its benefit. It remains a named gap.
* F8: not a defect. The ratio is a step sensitivity of a steady state of the first-order Chorin form (F8b: sensitivity orders 0.67
  to 0.80 for both boundary treatments); the Kim-Moin condition acts on a transient boundary layer that this metric does not see.
* The PISO path (outer iterations remove the lag) and `bc.rs` `Periodic` are not edited. Recommended follow-up outside this wave:
  the same extrapolation for single-corrector BDF2 PISO with periodic faces (the 21C addendum measured the same first-order
  behaviour there).

### F-212 (D2)

| Check | Measured | Limit | Result |
| --- | --- | --- | --- |
| S1 P2 and P8, N = 60, 120, 240, 480: position error | 1.859e-2, 3.50e-3, 6.27e-5, 1.622e-3 (P8 identical, mirror image) | N = 240 and N = 480 <= 3e-2 | met |
| S1 P4 balance, all levels | worst 4.2e-13 (N = 480) | <= 1e-10 | met |
| S1 P5 range and monotonicity | within range, violation 0 | <= 1e-10 | met |
| S1 P6 Newton iterations | max 15, all converged | < 100 | met |
| S2 `safeguarded_steps` for P1 (4 levels), P2/P8 N <= 240 | 0 | 0 | met (bit-identical to the pre-change loop) |
| S3 forced safeguarded branch, P2 N = 60, 120, 240 | position difference 2.9e-16, 1.5e-16, 9.1e-15; max 3 iterations | <= 1e-8 | met |
| S4 plain loop fails at N = 480 | `safeguarded_steps` = 1 (P2 and P8) | > 0 | met: the cycle existed, ONE step of the run needs the branch |
| S5 differences contract at order >= 0.9 | positions 0.9314775, 0.9323571, 0.9322474, 0.9309247; differences 8.8e-4, 1.1e-4, 1.3e-3 | report | hypothesis FALSIFIED (amendment 1) |
| S6 K2 SIMPLEC solution against the exact 1-D discrete solution | 9.3e-13, 1.2e-11, 1.7e-11 (ny = 20, 40, 80) | <= 1e-9 | met |
| S7 corrected set-up P1: invariants and N = 240 position | errors 6.3e-4, 1.28e-3, 1.33e-3, 9.8e-4 | invariants, <= 3e-2 | met |
| S7 P1 successive orders | -1.03, -0.06, 0.45 | >= 0.9 | MISSED, `p1_v2_known_miss_...` (ignored) |
| S7 P2/P8 corrected set-up, invariants and position | errors 7.21e-3, 5.12e-3, 1.42e-3, 7.26e-4 | invariants, <= 3e-2 | met |
| S7 P2/P8 successive orders | 0.49, 1.85, 0.97 | >= 0.9 | MISSED on the first pair, `p2_v2_known_miss_...`, `p8_v2_known_miss_...` (ignored) |

* The registered N = 480 invariants P4 to P6 for P2/P8 are now met. The ORIGINAL `p2_/p8_two_phase_*_known_miss` tests stay
  ignored and now fail only on the successive-order criterion (the N = 240 error 6.3e-5 is an accidental near-cancellation).
* P1 order, explanation (exploratory runs in a scratch copy, code not in the repository, corrected initial state): the relative
  position error against the registered sharp-front reference sits on a floor set by the mush half-width and the step size, not
  by the spatial resolution. Errors for N = 60, 120, 240, 480:
  eps = 2e-3, dt = h/2 (registered): 6.3e-4, 1.28e-3, 1.33e-3, 9.8e-4;
  eps = 2e-3, dt = h/8: -1.1e-3, 6.8e-4, 8.4e-4, 7.9e-4;
  eps = 5e-4, dt = h/2: 4.0e-5, 8.0e-4, 5.8e-4, 3.5e-4;
  eps = 5e-4, dt = h/8: -1.7e-3, 1.9e-4, 1.6e-4, 1.7e-4;
  eps = 5e-5, dt = h/8: -1.8e-3, 4.3e-5, -4.1e-5, -1.6e-5.
  The floor drops with eps and with dt, and the registered metric cannot show a monotone order once it is reached. The original
  set-up also had a 2 percent first-order sampling error of the initial liquid content (a persistent offset by the exact enthalpy
  balance); S7 removed that part (initial errors 6.0e-3, 1.4e-3, 2.9e-4, 5e-6) but not the floor. No fix is claimed for the P1
  order criterion; the registered ignored test stays.
* K2 ny = 20: the SIMPLEC solution equals the exact discrete solution to 1e-11, whose error at ny = 20 is 1.004021e-2: a pure
  truncation error. Not a solver defect; no code change; the ignored record stays.

### F-215 (D3)

* The interface face conductance is exactly zero (asserted), which excludes inexact interface flux use. Exploratory diagnostic
  (scratch copy, not in the repository): the column sums of the assembled transient matrix, evaluated with compensated summation,
  against the capacities `V w/dt`. The stored matrix is not column-conservative: its diagonal `cap + sum k` is one rounding of a
  number about 1.6e4 times larger than `cap` (Fourier number 8e3 per cell), i.e. about one ulp(diag). Total absolute defect over the
  40 columns 4.9e-15 (cap 6.25e-4 per column) at dt = 0.01. The systematic drift this predicts for side 0 per step,
  `sum_j defect_j x_j`, is 1.56e-13, 2.22e-13, 1.84e-13 (dt = 0.01, 1e-3, 1e-4, units of `V w T/dt`); converted to the enthalpy
  drift over 50 steps with `dt`, `rho cp` and `h0` (hand computation, about 10 percent uncertain through `h0`) it is 1.8e-12,
  2.5e-13, 2.0e-14 against the measured 2.0e-12, 2.5e-13, 1.8e-14. G1 for side 0: met (within a factor 1.5 at all three).
* Side 1 (k = 10, ten times stiffer) is NOT explained by this defect alone: the per-step defect prediction (9.9e-13 at dt = 0.01)
  has the order of, but differs in sign from, the sampled per-step actual drift, which scatters from step to step, so the
  linear-solver residual (recursive residual of CG/BiCGStab against the true residual of a matrix with condition number about the
  Fourier number) contributes at least as much. Both are floating-point effects proportional to eps times the Fourier number.
* Conclusion: not a defect of the interface (zero conductance exactly), and not an accumulation order that a reordering would
  remove: the drift is the double-precision representation of a stiff conservative matrix and of its iterative solve. A remedy
  would need an extended-precision (compensated) matrix assembly and a flux-form residual with iterative refinement in the shared
  energy sweep; it would change the bits of every transient scalar solve unless opt-in, and is not worth that for a check that was
  registered at an arbitrary dt. No code change; G5 stays the ignored miss with its limit intact; `g5_v2` (dt = 1e-3) stays.

### Deviations from the registered text

1. Order of events: exploratory diagnostics preceded the ADR (declared at the top). Registered limits were never changed.
2. S5 was a hypothesis that the data falsified (amendment 1); S7 (corrected set-up) was registered before its run and did not
   rescue the P1 order criterion either.
3. A4's limits (half of the pre-fix errors) were chosen after seeing the exploratory result that the fix gives about a quarter;
   they are an evidence floor, not a derived accuracy claim.
4. The test `s7_p1_reference_offset_report` is a report only; its second reference (solid at the solidus) gave LARGER errors, so
   that explanation of the P1 offset is rejected.
5. The Voller-Swaminathan update was not implemented (rejected in D2 for lack of contraction); not tried.
6. That the directional derivative is monotone for the explicit non-orthogonal correction is argued, not proven; the registered
   orthogonal meshes do not exercise it.

### Not resolved

F2 BDF2 order (1.51, collocated pressure-smoothing term), F8, P1 successive orders, original P2/P8 order criterion, K2 ny = 20
(truncation), G5 (floating point), rotational pressure correction, the PISO periodic-lag follow-up.
