# ADR 0180: Job 49Q, a face-bounded second-order convection scheme for the FAN-01 bladed stall, tested on B3 and B2

Status: **registered 2026-10-31**, before any code of this job and before any solver run. The arms, the fixed parameters,
the acceptance limits, the refinement check, the choice rule, the time budgets, the predictions, the reading rules, the
stop rule and the not-run list are fixed here. **No arm of this job has been run on any mesh.** Amendments follow this
text, each written before what it covers; results go in an addendum. Not yet accepted by the maintainer.

Job 49Q, on branch `job49q-fan01-second-order` from `job49p-fan01-auto-fallback` at 2678c49 (worktree
`.claude/worktrees/job49q`). The job id 49Q was given by the coordinator; 0180 was free when this was written
(`git log --all -- docs/adr/018*` empty).

House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
- limits are fixed here before the registered runs; a missed limit or a falsified prediction becomes an ignored
  `*_known_miss` test with the limit intact; a defective set-up gets an amendment before the corrected run;
- **no solver change is planned.** Every arm uses a convection scheme the solver already has and the case schema already
  accepts (`tvd_minmod`, `tvd_van_albada`, `tvd_van_leer`; `crates/rcfd-api/src/sim.rs`), switched on the running solver
  through `Simulation::simple_solver_mut` as ADR 0110 S10 switched to `upwind`. Defaults are bit-identical by
  construction, there is no new case key, and `baseline_hashes.rs` is not re-recorded;
- no new crate, no unsafe code, no new third-party crate, no commit trailers, nothing merged or pushed.

Clean-room: first principles, our own ADRs, and published limiter definitions (cited below, recalled, UNVERIFIED as
ADR 0110 marks its references; nothing depends on the citations).

## The request

The maintainer dropped first-order patching after ADR 0179 (B2 showed a 2.88 % torque effect against 0.43 % on B3) and
asked for a convection scheme change that keeps second order everywhere and lets the bladed passage converge on B3 and
B2. Accuracy is the aim, not only a settled run. At most three arms, preferably with little or no new code.

### What "second order everywhere" means here

The baseline, `linear_upwind` with the Barth-Jespersen limiter, is a limited scheme: it falls to first order at local
extrema and wherever the limiter clips. Every TVD limiter does the same (at r <= 0, and in the clipped range). No limited
scheme can be second order in every cell at every state. The claim registered here is therefore:
- the arm is in the **same formal class as the baseline**: second order where the solution is smooth and monotone,
  limited at extrema;
- **no cell is forced to first order by a rule**, a mask or a selector (unlike ADRs 0178 and 0179).

If the maintainer means something stricter (an unlimited second-order scheme), no arm here can meet it; unlimited
`linear_upwind` trips the steady guard within 7-27 iterations on these meshes (ADR 0110 set-up smokes). This is a
question to the maintainer (below), not a limit.

## What I read (rule d; nothing run)

- [ADR 0110](0110-wave43c-fan01-rotor-passage-non-convergence-diagnosis.md):
  - S1: freezing the Barth-Jespersen factors on B3 makes the leading-edge oscillation **grow** (`rho_med` 1.36); S12
    (frozen at the windowed minimum) moves it to the hub (1.02); S9 / S9' (frozen plus pseudo time) give exact
    period-two cycles. S10 (`upwind`) converges.
  - S1v: Venkatakrishnan(5) on B3, `rho_med` 0.94.
  - Reading: the high-order, explicit part of the limited `linear_upwind` makes the outer iteration locally unstable;
    the limiter's switching only bounds that instability into a cycle.
- [ADR 0111](0111-wave44a-fan01-frozen-limiter-linearisation-and-relaxed-deferred-correction.md): the frozen-limiter map
  at the B3 cycle point has a leading eigenvalue of about -1.33 at hub mid-chord cells (its prediction check M3 failed,
  so this is not attributed).
- ADRs [0175](0175-job49l-where-the-fan01-stall-is-driven-in-the-nonlinear-iteration.md),
  [0177](0177-job49n-under-relaxed-deferred-correction-on-the-bladed-fan01-passage.md),
  [0178](0178-job49o-the-fan01-hub-cycle-with-the-leading-edge-driver-held-off.md),
  [0179](0179-job49p-an-automatic-first-order-fallback-for-the-fan01-stall-cells.md): the convection deferred
  correction carries the stall; the leading edge and the hub mid-chord are two autonomous drivers; F-R(0.25) removes the
  first, removing the correction in the hand-picked hub ring H3 removes the second (torque 1.9144 N m, Delta p_ts 45.75
  Pa); an automatic first-order selector failed, and its B2 torque effect grew.
- **Prior TVD evidence on older FAN-01 meshes, against my predictions**:
  - ADR 0077 D9 and ADR 0078 N1: `tvd_van_leer` diverged on the H-slit sector mesh (continuity 1e69 by iteration 300);
    `tvd_minmod` was stable there.
  - ADR 0082 amendment 2: the empty H-slit passage with `tvd_minmod` (X2d-36) did **not** converge by tolerance in
    8 000 iterations (`res_u` 5.3e-4 at the cap).
  - Those meshes are not the `camber_seam_h` C-grids of B3 and B2, and those runs predate SST on the casing and the
    SIMPLEC compensations, so the evidence is not decisive. It is weighed in the predictions.
- Code read (no run):
  - `crates/rcfd-fv/src/convection.rs` `face_value`: `Tvd(lim)` uses the face form
    `phi_f = phi_U + psi(r) lambda (phi_D - phi_U)`, `r = 2 grad(phi_U) . d / (phi_D - phi_U) - 1`, `lambda` the
    linear-interpolation weight of the downwind cell, with the **unlimited** least-squares gradient. Its periodic-face
    correction is 1.0 (the full central value), as for `linear_upwind`; only `Gamma`, `Compressive` and
    `BoundedCentral` keep the upwind value on periodic faces, so those three are excluded (first order across the seam).
  - `crates/rcfd-fv/src/simple.rs`: `linear_upwind_limiter` and `frozen_limiter` act only for `LinearUpwind`; with a
    `Tvd` scheme they are ignored. `conv_gradient_hook` sees the limited gradient of each component.
  - `crates/rcfd-api/src/sim.rs` 1760: a case with a `Tvd` scheme must not set `linear_upwind_limiter` (located error).

## Why these arms (first principles)

Write any upwind-biased face value as a blend of the two cells of the face,
`phi_f = phi_U + kappa_f (phi_D - phi_U)`. With frozen `kappa_f` and a divergence-free flux `F`, the convection
operator's energy is

`sum_c phi_c sum_f F_f phi_f = sum_f |F_f| (1/2 - kappa_f) (phi_U - phi_D)^2 + boundary terms`

(use `phi_U (phi_U - phi_D) = (phi_U^2 - phi_D^2)/2 + (phi_U - phi_D)^2/2` and the zero divergence). So a face
dissipates for `kappa_f < 1/2` (upwind: `kappa = 0`) and is anti-dissipative for `kappa_f > 1/2`.

- **`linear_upwind`** extrapolates from the upwind cell with its own gradient, which also uses the upwind cell's other
  neighbours: `kappa_f = psi g_U . (x_f - x_U) / (phi_D - phi_U)`. The Barth-Jespersen limiter bounds `phi_f` by the
  extremes of the upwind cell's neighbours, **not by `phi_D`**. Where `phi_D` is close to `phi_U` but the gradient is
  large (a stagnation corner, a wall corner, a hub cell with two wall faces), `kappa_f` can be far above 1 or negative.
  The frozen operator can then be locally anti-dissipative. That is consistent with ADR 0110 (freezing makes it grow;
  only the switching bounds it) and with ADR 0111's eigenvalue beyond -1. A smooth limiter of the same cell
  extrapolation (Venkatakrishnan) or a frozen one does not bound `kappa_f` either. **So freezing and Venkatakrishnan
  get no arm** (and both were already run: S1, S12, S9, S9', S1v).
- **The face-form TVD schemes** bound `kappa_f = psi(r) lambda` by `psi_max lambda`:
  - minmod: `psi <= 1`, so `kappa_f <= lambda`, never more downwind than linear interpolation (about 1/2 on smooth
    meshes): non-anti-dissipative up to the mesh stretching;
  - van Albada: `psi <= 1.21` (at r = 1 + sqrt 2), and differentiable for r > 0: smooth and almost bounded;
  - van Leer: `psi -> 2`, so `kappa_f` up to `2 lambda`, about 1: smooth for r > 0 but **not** bounded. It is the
    control for the hypothesis (smoothness alone versus the face bound), and the least dissipative of the three.

All three have `psi(1) = 1` (second order in smooth monotone regions) and lie in Sweby's TVD region (Sweby, SIAM J.
Numer. Anal. 21, 1984; minmod: Roe, Annu. Rev. Fluid Mech. 18, 1986; van Leer, J. Comput. Phys. 14, 1974; van Albada,
van Leer and Roberts, Astron. Astrophys. 108, 1982; the face form on unstructured meshes: Darwish and Moukalled,
Int. J. Heat Mass Transfer 46, 2003; all recalled, UNVERIFIED). The energy identity is first principles and covers the
frozen convection operator only: not SIMPLEC coupling, the Coriolis source, the turbulence model or the nonlinearity.

## Arms (fixed now)

| Id | Scheme from iteration 1001 | Case value | Role |
|---|---|---|---|
| **T-MM** | `ConvScheme::Tvd(TvdLimiter::Minmod)` | `tvd_minmod` | face-bounded (`kappa <= lambda`), non-smooth at r = 0 and 1 |
| **T-VA** | `ConvScheme::Tvd(TvdLimiter::VanAlbada)` | `tvd_van_albada` | smooth for r > 0, almost bounded (`kappa <= 1.21 lambda`) |
| **T-VL** | `ConvScheme::Tvd(TvdLimiter::VanLeer)` | `tvd_van_leer` | smooth for r > 0, not bounded (`kappa <= 2 lambda`); hypothesis control |

**Fixed parameters.**
- Cases: B3 = `case_toml(F3, true)`, B2 = `case_toml(F2, true)` of `fan01_auto_fallback_w49p.rs` (the ADR 0086 set-up,
  SST, SIMPLEC, `linear_upwind` + Barth-Jespersen, rotating frame), unchanged.
- Protocol of an arm on mesh m: 1000 default iterations (the stalled reference), then
  `simple_solver_mut().settings.convection = <scheme>` (nothing else changes; no F-R, no mask, no frozen limiter),
  then 2000 more iterations. Windows, `rho_med` and `rho_du` as ADRs 0175-0179.
- Reference: iterations 501-1000 of the same run; T_ref,m and P_ref,m the means of torque and Delta p_ts over every
  iteration of it (as ADR 0179).
- No parameter of any arm is calibrated. There is nothing to tune, so B3 is not used for calibration; B2 is still held
  out in the sense that **all three B3 records are committed before any B2 run**, and no B2 result changes an arm, a
  limit or the choice rule.

## Validity checks (a failure is a defective set-up: amendment before anything is read)

- **V1** B3 has 4 920 cells; its reference medians are 1.089e-3 / 4.87e-4 and the torque at 1000 is 1.9177 N m, to the
  printed digits.
- **V2** B2 has 13 627 cells; its reference reproduces ADR 0179: medians 1.004e-3 / 6.23e-4, torque mean 2.0308 N m and
  Delta p_ts mean 63.860 Pa, to the printed digits.
- **V3** the existing bit-identity tests pass unchanged (`fan01_stall_w43c.rs` non-ignored ones,
  `w49l_empty_mask_is_bit_identical`, `w49n_field_none_is_bit_identical`, `baseline_hashes.rs`). No solver file changes,
  so this is a check of the branch, not of new code.

## Acceptance limits (per arm)

| Id | Limit |
|---|---|
| **C1** | B3 and B2: `res_u` and `res_p` both below 1e-5 at some iteration after the switch |
| **C2** | B3 and B2: `is_converged()` true at the end of the run |
| **C3** | B3 and B2: `rho_du <= 0.01` |
| **RN** (not grow) | with `D_f,m = abs(f_end,m - f_ref,m) / abs(f_ref,m)` for f = torque and f = Delta p_ts: `D_T,B2 <= max(D_T,B3, 0.002)` **and** `D_P,B2 <= max(D_P,B3, 0.005)` |
| **RS** (shrink; needed only for the label "second-order-consistent") | `D_T,B2 < D_T,B3` and `D_P,B2 < D_P,B3`, each with `D_B3` above its floor (0.002 torque, 0.005 Delta p_ts); if a `D_B3` is at or below its floor, RS is NOT EVALUABLE for that functional |

The floors are under half the half-width of the B3 stalled band of ADR 0177 (torque [1.9072, 1.9279], Delta p_ts
[46.263, 47.130], each widened by 0.5 %): differences below them are inside the cycle's own scatter. 0.002 is ADR 0179's
R1 floor.

`D` compares two discretisations of the same class on the same mesh (the arm's converged state against the stalled
Barth-Jespersen cycle mean). If both are consistent, `D` must fall with refinement. With r32 = 1.40 (ADR 0086) the
expected ratio `D_B2 / D_B3` is about 0.51 for a second-order difference and 0.71 for a first-order one; **both are
below 1, and two meshes cannot establish an order.** The ratio and the implied order are reported without a limit.

**An arm is accepted** iff C1, C2, C3 hold on B3 and on B2 and RN holds. It is **accepted and second-order-consistent**
iff RS also holds for both functionals.

**Choice rule (fixed now)**: if more than one arm is accepted, the recommendation is the least dissipative accepted arm,
in the a-priori order T-VL, T-VA, T-MM (largest `psi` first). If two accepted arms tie on that order, which cannot
happen here, the smaller `D_T,B2` decides.

Reported without a limit, per run:
- `reached` (first iteration at tolerance), `rho_med`, the end residuals, the turbulence change at the end (to say why
  C2 fails if C1 holds);
- torque and Delta p_ts at the end, their D against the reference, and against ADR 0178 N-H3 (B3: 1.9144 N m, 45.753 Pa);
- the gap to the measured values (torque 2.142 N m, Delta p_ts 126.5 Pa, ADR 0075): not a limit of this job;
- the five largest-`|du|` cells at the end with r, z;
- wall-clock seconds.

**Mechanism record K (report only, B3 runs, captured in iteration 1000 before the switch)**: through
`conv_gradient_hook`, which reads the limited gradient and leaves it unchanged (the hook is cleared right after that one
iteration), compute `kappa_f` of the Barth-Jespersen `linear_upwind` face value for every internal face and velocity
component with `abs(phi_D - phi_U) > 1e-3` m/s. Report the share of face-components with `kappa > 1/2`, `kappa > 1` and
`kappa < 0`, over all faces and over the faces of the 20 cells with the largest `|du|` at the switch.

## Predictions (mine)

| Id | Prediction |
|---|---|
| **Q1** | T-MM meets C1, C2, C3 on B3 |
| **Q2** | T-MM meets C1, C2, C3 on B2 and RN |
| **Q3** | T-VA meets C1, C2, C3 on B3 and B2 |
| **Q4** | T-VL misses C3 on B3 (or trips the guard), as van Leer diverged on the H-slit mesh |
| **Q5** | for every accepted arm, `D_T,B3 <= 0.03` (the torque within 3 % of the stalled cycle mean) |
| **QK** | in record K the share of `kappa > 1` over the top-20 cells' faces exceeds the share over all faces |

My confidence in Q1-Q3 is moderate, not high: minmod stalled on the empty H-slit passage (ADR 0082), and a non-smooth
limiter can still cycle by switching even when its frozen operator dissipates. C2 may fail through the SST change
criterion even where C1 holds (ADR 0110 S10 did this).

## Reading rules (fixed now; made only if V1-V3 hold)

1. **At least one arm accepted**: a face-bounded second-order scheme converges the bladed FAN-01 passage on B3 and B2
   without masks, and its functional difference from the baseline does not grow with refinement. The recommendation is
   the arm given by the choice rule; it is already a case value, so no code is needed to use it. Next job (not this
   one): the run from the start on B1 within the 60-minute guard, the empty passages E3, E2, E1, and a solution
   verification of torque and Delta p_ts with the chosen scheme. If the arm is also second-order-consistent (RS), say so;
   if not, say that convergence is shown but consistency of the functional difference is not.
2. **C1-C3 on B3, not on B2**: the scheme does not transfer to the finer mesh; report where the B2 cycle sits. Not
   accepted.
3. **Residual-converged, C2 missed** (C1 and C3 met on a mesh, `is_converged()` false): reported as residual-converged,
   with the turbulence change that blocks C2. Not accepted by the limits (C2 is a limit). The maintainer decides
   whether the SST change criterion should bind for this decision.
4. **C1-C3 on both meshes, RN missed**: the scheme converges, but its difference from the baseline grows from B3 to B2.
   Not accepted; the size of the growth is reported.
5. **Mechanism** (no verdict, read with the others): T-MM / T-VA converging while T-VL does not is consistent with the
   face bound mattering (not a proof). T-VL converging as well says the face form alone (or smoothness) suffices, and by
   the choice rule it is then the recommendation if accepted.
6. **All three arms miss** (none accepted): **stop and report**. No further arm, scheme, parameter or relaxation is run
   in this job without the maintainer.

Nothing here changes a default, a limit of another ADR, or a known miss.

## Time budgets and run order

- B3 arm: 10 min wall clock (expected about 2-2.5); B2 arm: 30 min (expected 5-8). A run that hits its budget stops,
  writes its record with `budget_hit = true`, and counts as NOT EVALUABLE: every convergence limit of that run is
  missed. It is not rerun without an amendment.
- A run stopped by the steady divergence guard or a non-finite residual counts as missed (C1-C3).
- Order: T-MM-B3, T-VA-B3, T-VL-B3 (at most two at a time); commit the three B3 records; then T-MM-B2, T-VA-B2,
  T-VL-B2, every arm on B2 whatever its B3 result (B2 alone or beside at most one other run).
- **Report-only run from the start** (only if reading rule 1 applies): B3 with `solver.convection = <recommended case
  value>` and no `linear_upwind_limiter` line, 3000 iterations from iteration 1, budget 10 min; `reached`,
  `is_converged()`, torque and Delta p_ts against the switch arm's end values. No limit.

## Not run (and why)

- Freezing the limiter, Venkatakrishnan or any other cell-based limiter of the `linear_upwind` extrapolation: run before
  (ADR 0110) and not face-bounded (above).
- `Gamma`, `Compressive`, `BoundedCentral`: first order on the periodic faces. Superbee, MUSCL, Koren: `psi` up to 2,
  like van Leer, so they add nothing to the control. QUICK, central: unbounded.
- F-R, masks, selectors: not schemes; ADRs 0177-0179.
- B1, E3, E2, E1, the shipped cases: next job under reading rule 1; no default changes.

## Deliverables

- `crates/rcfd-api/tests/fan01_second_order_w49q.rs`: the arms (ignored; one per process), V1, V2, record K, the
  refinement check and the from-start run. Each run writes JSON with `W49Q_OUT=<dir>`.
- `docs/investigations/job49q-fan01-second-order/`: the JSON, `results.csv` and the logs, committed as soon as they
  exist.
- This ADR's addendum; one sentence appended at the end of the robustness row of `docs/milestone-fan.md`.

## Addendum: results (2026-10-31)

Data: `docs/investigations/job49q-fan01-second-order/` (one JSON per run, `*-refinement.json`, `results.csv`, `logs/`).
Release, on d17b2ca (no solver change). Each run was made once: T-MM-B3 and T-VA-B3 side by side, then T-VL-B3 alone; the
three B3 records were committed (206fc97, 98426a5) before any B2 run; then T-MM-B2 and T-VA-B2 side by side, T-VL-B2
alone. No run reached its budget (B3 about 2 min, B2 about 5 min); no run tripped the divergence guard.

### Validity

- **V1 met.** B3 has 4 920 cells; medians 1.089e-3 / 4.87e-4; torque 1.9177 N m at 1000 (every B3 run).
- **V2 met.** B2 has 13 627 cells; medians 1.004e-3 / 6.23e-4; reference means 2.0308 N m and 63.860 Pa (every B2 run).
- **V3 met.** `w44a_hooks_at_defaults_are_bit_identical`, `frozen_limiter_hook_is_bit_identical_at_the_freeze`,
  `w45a_p5_defaults_are_bit_identical_to_main`, `w49l_empty_mask_is_bit_identical`, `w49n_field_none_is_bit_identical`
  and the three `baseline_hashes.rs` tests pass unchanged.

### Runs

Every run switches at iteration 1000 and runs to 3000. `|du|_2` at the switch: 5.74 m/s (B3), 8.25 m/s (B2).

| Run | `reached` | `is_converged()` | `rho_med` | `rho_du` | `res_u` end | Torque end (D_T) | Delta p_ts end (D_P) | End cycle (five largest `|du|`) |
|---|---|---|---|---|---|---|---|---|
| T-MM-B3 | - | false | 2.20 | 0.849 | 2.2e-3 | 1.9037 (0.0073) | 39.63 (0.151) | 1.0-1.4 m/s, hub, r 0.124-0.126, z 0.13-0.17 |
| T-VA-B3 | - | false | 0.519 | 0.306 | 4.5e-4 | 1.9110 (0.0035) | 41.59 (0.109) | 0.3-0.8 m/s, shroud tip layer r 0.248-0.249, z 0.119 |
| T-VL-B3 | - | false | 4.88 | 3.39 | 3.3e-3 | 1.8920 (0.0134) | 40.45 (0.134) | 3-5 m/s, hub r 0.126, z 0.10-0.12 (leading-edge root) |
| T-MM-B2 | - | false | 1.87 | 1.41 | 1.9e-3 | 2.0590 (0.0139) | 60.03 (0.060) | 2.6-3.9 m/s, hub r 0.124-0.126, z 0.171-0.172 |
| T-VA-B2 | - | false | 1.87 | 1.48 | 1.9e-3 | 2.0545 (0.0117) | 60.82 (0.048) | 2.8-3.8 m/s, the same hub cells |
| T-VL-B2 | - | false | 1.69 | 1.39 | 1.6e-3 | 2.0404 (0.0047) | 60.64 (0.050) | 2.9-4.1 m/s, the same hub cells |

None of these runs converged, so every end value is one sample of a cycle, not a solution.

Gap to the measured values (2.142 N m, 126.5 Pa; a report, not a limit; cycle samples): torque -11.1 / -10.8 / -11.7 % on
B3 and -3.9 / -4.1 / -4.7 % on B2 (T-MM / T-VA / T-VL); Delta p_ts -69 / -67 / -68 % on B3 and -53 / -52 / -52 % on B2.

Record K (the Barth-Jespersen `linear_upwind` face blend at iteration 1000 on B3; 41 340 face-components with
`|phi_D - phi_U| > 1e-3` m/s, 258 of them at the faces of the 20 largest-`|du|` cells):

| Share | All faces | Top-20 cells' faces |
|---|---|---|
| `kappa > 1/2` | 35.9 % | 27.5 % |
| `kappa > 1` | 10.0 % | 13.2 % |
| `kappa < 0` | 14.9 % | 22.5 % |

### Limits

| Arm | C1 B3 | C2 B3 | C3 B3 | C1 B2 | C2 B2 | C3 B2 | RN | RS |
|---|---|---|---|---|---|---|---|---|
| T-MM | MISSED | MISSED | MISSED | MISSED | MISSED | MISSED | **MISSED** (D_T 0.0073 -> 0.0139) | MISSED |
| T-VA | MISSED | MISSED | MISSED | MISSED | MISSED | MISSED | **MISSED** (D_T 0.0035 -> 0.0117) | MISSED |
| T-VL | MISSED | MISSED | MISSED | MISSED | MISSED | MISSED | met (D_T 0.0134 -> 0.0047, D_P 0.134 -> 0.050) | met |

RN and RS are computed as registered, from the end samples. With no run converged they compare cycle samples, so T-VL's
"met" says nothing about consistency and I read nothing into it.

**No arm is accepted.** Predictions: Q1, Q2 and Q3 are missed; Q4 holds (T-VL misses C3 on B3); Q5 is not evaluable (no
accepted arm); QK holds as worded (13.2 % against 10.0 %), but see reading 3. Known misses: `t_mm_b3_known_miss`,
`t_va_b3_known_miss`, `t_vl_b3_known_miss`, `t_mm_b2_known_miss`, `t_va_b2_known_miss`, `t_vl_b2_known_miss`. The
report-only run from the start (`from_start_b3`) was not run: reading rule 1 does not apply.

### Reading (by the registered rules)

**Rule 6 applies: all three arms miss. The job stops here; no further arm was run.**

1. **Bounding the face blend is not enough.** Minmod has `kappa <= lambda` on every face, so its frozen convection
   operator does not anti-dissipate, and it still cycles on both meshes (worse than the baseline: `rho_med` 2.2 / 1.9).
   So the energy argument for the frozen convection operator, which is the reason for the arms, does not capture what
   drives the stall. Whatever it is lies outside that operator alone: the SIMPLEC coupling, the rotating-frame source,
   SST, or the switching of the limiter itself (minmod has kinks at r = 0 and 1).
2. **On B2 every scheme cycles at the same hub cells.** The B2 cycles of minmod, van Albada and van Leer sit in the
   same five hub cells (r 0.124-0.126, z 0.171-0.172). That is the axial height of the hub cycle that took over on B3 under
   F-R (ADRs 0177 and 0178) and of ADR 0111's leading linear mode. A site that survives a change of limiter family,
   reconstruction (face TVD against cell extrapolation) and smoothness is unlikely to be a limiter property. I report it
   as an observation; this job does not attribute it.
3. **Record K.** The Barth-Jespersen face value lies beyond the central blend (`kappa > 1/2`) on about a third of all
   faces, and beyond the downwind value (`kappa > 1`) on a tenth of them. That is everywhere, not just at the stall
   cells: at the top-20 cells `kappa > 1/2` is in fact rarer (27.5 %), and only `kappa > 1` and `kappa < 0` are
   somewhat more common. So the anti-dissipative faces are not localised where the cycle lives. This agrees with
   reading 1.
4. **Van Albada on B3 came closest** (`rho_med` 0.52, `res_u` 4.5e-4), with the residual cycle moved to the shroud tip
   layer. On B2 it is back at the hub, at the stall level. Q3 is missed on both meshes.
5. The functionals of the TVD cycles are 11-15 % lower in Delta p_ts on B3 than the Barth-Jespersen cycle mean, and
   within 1.4 % in torque. On B2, which is closer to the measured torque, the differences are 4.8-6.0 % (Delta p_ts) and
   0.5-1.4 % (torque). These are values from cycles, not solutions.

Nothing here is a statement about accuracy or agreement with the measured fan. No default, limit or known miss of another
ADR changes.

### Deviations

1. **The JSON location.** `W49Q_OUT` was a relative path in the B3 runs, and cargo runs tests from the crate directory.
   So the three B3 JSON files were written under `crates/rcfd-api/docs/...`. The B3 logs were committed first (206fc97);
   the JSON files were then moved, unchanged, and committed in 98426a5, before any B2 run. The B2 runs used an absolute
   path.
2. **V3 ran beside T-VL-B2.** The V3 tests ran in a separate process while T-VL-B2 was running. The results are
   deterministic, and the T-VL-B2 run did not reach its budget.
3. **Interpreting RN and RS.** The registration did not say how to read RN and RS when no run converges. They are
   reported as computed, and not interpreted (above).

### Questions for the maintainer

1. **What "second order everywhere" means.** Is a limited scheme of the baseline's class acceptable (second order where
   the solution is smooth, limited at extrema, no cell forced to first order by a rule)? Recommendation: yes. Every
   unlimited second-order scheme tried on these meshes trips the guard within 30 iterations (ADR 0110), so the stricter
   reading has no runnable candidate.
2. **Next job: the z 0.17 hub site itself, not another limiter?** Recommendation: yes. On B2 all three limiter families cycle
   at the same hub cells, and on B3 the Barth-Jespersen run with F-R cycled at the same axial height (ADR 0177). Register a diagnosis of that site's mesh and
   coupling: the cells' geometry against the C-grid construction (seam, hub wall layer, the junction at mid-chord), and
   a localised Jacobian of the TVD map there (the ADR 0084 harness). The aim is to find whether a mesh change at that
   site, which would keep the scheme second order everywhere, removes the cycle.
3. **Meanwhile, keep the cycle-mean functionals with a band for the milestone?** ADR 0086 reports torque and Delta
   p_ts as means over a cycle with a band. Recommendation: yes, keep them as the reported values until a converged
   second-order state exists, and mark them as cycle values. No scheme tried so far changes the B2 torque by more than
   1.4 % from the Barth-Jespersen cycle mean.
