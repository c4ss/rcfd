# ADR 0179: Job 49P, an automatic first-order fallback for the cells that sustain the FAN-01 stall (with F-R), tested on B3 and B2

Status: **registered 2026-10-31**, before any code of this job and before any solver run. The selector, its parameters, the
threshold rule, the acceptance limits, the refinement condition, the time budgets, the predictions, the reading rules
and the not-run list are fixed here. **None of the candidate properties has been evaluated on any FAN-01 cell or run
for this job.** Amendments follow this text, each written before what it covers; results go in an addendum. Not yet
accepted by the maintainer.

Job 49P, on branch `job49p-fan01-auto-fallback` from `job49o-fan01-hub-locus` at 14c9639 (worktree
`.claude/worktrees/job49p`). The number 0179 and the job id 49P were given by the coordinator.

House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
- limits are fixed here before the registered runs; a missed limit or a falsified prediction becomes an ignored
  `*_known_miss` test with the limit intact; a defective set-up gets an amendment before the corrected run;
- **no solver change is planned.** The selector runs in the test harness and drives the two kept test-only fields,
  `high_order_mask_test_only` (ADR 0175) and `deferred_correction_relaxation_test_only` (ADR 0177), between iterations.
  So defaults are bit-identical by construction, there is no case key, and `baseline_hashes.rs` is not re-recorded;
- no new crate, no unsafe code, no new third-party crate, no commit trailers, nothing merged or pushed.

Clean-room: first principles and our own ADRs only.

## The request

The maintainer approved ADR 0178's question 1. Find a measurable property that picks out the cells that sustain the
stall, without hand-picking. Then run F-R(0.25) plus first order wherever it fires, and test that on B3 and B2.

The maintainer also accepted question 2's condition. First order in a small share of cells is acceptable only if B2
shows its effect on torque shrinking with refinement.

Requirements of the request:
- no cell ids and no FAN-01-tuned coordinates;
- the candidates and the threshold rule registered before they are scored on the ADR 0178 hub cells;
- B2 held out if B3 is used for calibration;
- convergence criteria, a share cap, and a comparison with the ADR 0178 hand-picked run;
- a fixed torque-difference measure and limit;
- time budgets.

## What I read (rule d; nothing run)

- [ADR 0178](0178-job49o-the-fan01-hub-cycle-with-the-leading-edge-driver-held-off.md): with F-R(0.25) on, removing C
  (the convection deferred correction) in the hand-picked hub ring H3 (322 cells, 6.5 %) makes B3 converge. Residuals
  reach 1e-11, `reached` is 1298, `is_converged()` is true, torque is 1.9144 N m and Delta p_ts 45.753 Pa. H3 was grown
  from cells chosen by hand from the run records.
- [ADR 0177](0177-job49n-under-relaxed-deferred-correction-on-the-bladed-fan01-passage.md): F-R(0.25) alone removes the
  leading-edge cycle within 10 iterations, and a hub cycle then grows. B3 torque band at 1000 is [1.9072, 1.9279]
  (current reports, ADR 0170).
- [ADR 0174](0174-job49k-the-49h-wall-cell-metric-on-the-fan01-meshes.md) K2, K3: the stall cells rank at the 20th-30th
  percentile of the mesh metric `aspect x tan(non-orthogonality)`. That is prior evidence that static mesh-quality
  properties do not pick these cells.
- [ADR 0110](0110-wave43c-fan01-rotor-passage-non-convergence-diagnosis.md) and ADR 0175: in every stalled run the
  defect is a localised cycle of the iterate. In the reference 75-78 % of `sum |du|^2` lies in the largest 1 % of the
  cells, and in ADR 0178's F0 99 % lies in H4.
- [ADR 0086](0086-wave37c-fan01-solution-verification.md): B2 has 13 627 cells, about 0.107 s per iteration, and also
  stalls (`res_u` ~1e-3 at the 8 000 cap).

## Candidate properties (registered; none scored yet)

1. Mesh quality: the ADR 0172 / 0174 wall-cell metric, the largest face non-orthogonality, the aspect ratio.
   **Not used.** ADR 0174 already showed the stall cells are mild on them, and a static mesh property cannot follow a
   cycle that moves when the scheme changes (ADR 0175, 0177).
2. Limiter switching: the number of iterations in a window in which a cell's Barth-Jespersen factor changes by
   more than 1e-3 (ADR 0110 recorded switching in 1 300 cells per iteration, far more than the stall cells). **Not
   used as the selector** for that reason; reported only.
3. Iterate oscillation (the selector): `a_c` = the largest `|u_c^n - u_c^(n-1)|` (speed change of cell c) over a
   round of iterations. A cell that keeps changing after the rest of the field has settled is, by definition, where the
   outer iteration fails to converge. It uses only the current solution. It needs no geometry, no ids and no
   coordinates, and it would apply unchanged to any case.

## The selector (fixed now)

The **adaptive first-order fallback** works as follows.

**From the switch.** At iteration N0 = 1000, F-R(0.25) is switched on and the mask is empty (M = ∅).

**Rounds.** After every round of R = 50 iterations:
1. compute `a_c` over the round for every cell, and `A = max_c a_c`;
2. add to M every cell with `a_c >= max(theta A, 0.01 A_0)`;
3. grow the added cells by g = 1 face ring (seam partners included, the ADR 0175 adjacency);
4. set C removed in M (`high_order_mask_test_only.conv = M`, `lap` empty).

M only grows.

**Parameters.**
- `A_0` is `max_c |u_c^1000 - u_c^999|`, the stalled cycle's largest amplitude at the switch. The floor 0.01 A_0 stops
  the growth once the oscillation has fallen below 1 % of the stall's.
- theta = 0.1 (a cell is selected if it carries at least a tenth of the largest current oscillation).
- R = 50 covers at least 15 periods of the 2-3 iteration cycles.
- g = 1.

**Cap.** If adding a round's cells would make |M| exceed 10 % of the cells, no cells are added in that round or
later (the cap is hit, and this is recorded).

**Parameter choice.** theta, R, g, the floor and the cap are fixed a priori, by the reasons above, not calibrated.
Calibration happens only under the conditional rule below, and only on B3.

**Conditional calibration (only if the a-priori arm S-B3(0.1) misses any of C1, C2, C3, S on B3):**
- run S-B3(0.3) and S-B3(0.03), with everything else unchanged;
- theta* = the theta in {0.3, 0.1, 0.03} that meets C1, C2, C3 and S on B3 with the smallest final share;
- if none meets them, theta* = 0.1 and the selector is reported as failed on B3.

B2 runs once, with theta = 0.1 if no calibration happened, or with theta* otherwise. B2 is a held-out test in both
cases: no B2 run is made before theta is fixed, and B2 never feeds back into theta.

## Protocol

- **Cases**: B3 = `case_toml(F3, true)` and B2 = `case_toml(F2, true)`, counts F2 = (8, 17, 8, 11, 4, 15, 7) as in
  `fan01_stall_w43c.rs`. Both are the ADR 0086 set-up unchanged.
- **Arm S-m(theta)**: 1000 default iterations (the stalled reference), then the selector as above, then 2000 more
  iterations.
- **Reference window**: iterations 501-1000 of the same run. Its torque mean T_ref,m and Delta p_ts mean P_ref,m use
  every iteration. Medians as ADR 0175.
- **Time budgets** (wall clock, whole run):
  - 10 min for a B3 arm (expected about 2.5);
  - 30 min for a B2 arm (expected about 6-8 at 0.107-0.15 s per iteration, more when two runs share the machine).
  A run that hits its budget stops, writes its record with `budget_hit = true`, and counts as NOT EVALUABLE: every
  convergence limit of that run is missed, a known miss. It is not rerun without an amendment that says why. B2 runs
  alone or beside at most one B3 run.

## Validity checks (a failure is a defective set-up: amendment before anything is read)

- **V1** B3 has 4 920 cells and its reference reproduces medians 1.089e-3 / 4.87e-4 and the torque at 1000 of 1.9177
  N m, to the printed digits.
- **V2** B2 has 13 627 cells. Its reference window is stalled: median `res_u` over 501-1000 >= 1e-4. Its values are
  printed; B2 has no current-report record.
- **V3** the existing bit-identity tests pass unchanged: the three in `fan01_stall_w43c.rs`,
  `w49l_empty_mask_is_bit_identical`, `w49n_field_none_is_bit_identical` and `baseline_hashes.rs`.

## Acceptance limits (each on B3 and on B2 unless stated)

| Id | Limit |
|---|---|
| **C1** | `res_u` and `res_p` both below 1e-5 at some iteration after the switch |
| **C2** | `is_converged()` true at the end of the run |
| **C3** | `rho_du <= 0.01` (`|du|_2` at the end over `|du|_2` at the switch) |
| **S** | the final first-order share |M| / n_cells <= 10 % and the cap was not hit |
| **H** (B3 only) | against the ADR 0178 hand-picked run N-H3: torque within 1 % of 1.9144 N m, and Delta p_ts within 2 % of 45.753 Pa |
| **R1** (refinement, the maintainer's condition) | with Delta_m = abs(T_end,m - T_ref,m) / abs(T_ref,m) (the selector run's final torque against the same mesh's stalled second-order cycle mean), Delta_B2 < Delta_B3, or Delta_B2 <= 0.002 (0.2 %, under half the half-width of the B3 stalled torque band of ADR 0177, so within the cycle's own scatter) |

The selector is **accepted** iff C1, C2, C3 and S hold on B3 and on B2, and H and R1 hold. Reported without a limit:
- Delta p_ts against its reference mean, both meshes;
- the round-by-round mask size and A;
- the iteration at tolerance;
- the share of `sum |du|^2` in the final mask;
- after the runs, the overlap of the B3 mask with ADR 0178's H3 and H4 (scored only after the runs, as required);
- for each final mask, the cells' median rank on candidate 1 (the ADR 0174 metric): a report, not a selector.

## Predictions (mine)

| Id | Prediction |
|---|---|
| **Q1** | S-B3(0.1) meets C1, C2, C3, S and H |
| **Q2** | S-B2 meets C1, C2, C3 and S |
| **Q3** | R1 holds |
| **Q4** | the B3 final mask is under 6.5 % of the cells (smaller than H3) |

## Reading rules (fixed now; made only if V1-V3 hold)

1. **Accepted**: an automatic, case-agnostic fallback (F-R plus first order where the iterate keeps oscillating)
   converges FAN-01 on B3 and B2. Its torque effect shrinks with refinement, so the maintainer's condition holds. Next
   job: register it as an opt-in case key with its own defaults-bit-identical checks. Run B1 within the 60-minute guard
   and the empty passages E3, E2, E1 (exact swirl), and check that it never fires on a converging case (a no-op on the
   shipped cases).
2. **Converges on both, R1 missed**: the fallback works numerically, but its torque effect does not shrink from B3 to
   B2. By the maintainer's condition it is not acceptable as a fix. Next job: a scheme change that keeps second order
   in those cells (for example a smoother limiter there), to be proposed.
3. **Converges on B3, not on B2**: the selector (or theta) does not transfer to the finer mesh. Report where the B2
   cycle sits and whether the cap or the floor stopped the mask; the next step depends on which. Not accepted.
4. **Does not converge on B3 at any theta**: the oscillation amplitude is not a usable selector at these parameters
   (for example the mask chases a moving cycle, as ADR 0175 saw). Next job: a different candidate, to be registered.
5. **H missed while C1-C3 and S hold on B3**: the automatic mask converges to a different mixed-order state than the
   hand-picked one. Reported. Acceptance fails by rule (H is a limit), and the size of the difference informs the
   maintainer.

Nothing here is a statement about accuracy or agreement with the measured fan. No result changes a default, a limit of
another ADR, or a known miss.

## Not run (and why)

- B1: about 3x the B2 cost, so it goes to the next job under rule 1.
- E3, E2, E1: the empty passages, next job.
- The shipped cases: no change to them; the fallback is not on any default path.
- No case key, no solver change, no other selector, no theta outside {0.3, 0.1, 0.03}, no change of R, g, the floor or
  the cap.

## Deliverables

- `crates/rcfd-api/tests/fan01_auto_fallback_w49p.rs`: V1, V2, the selector arms (ignored; B3 about 2.5 min, B2 about
  8 min in release; one per process). Each arm writes JSON with `W49P_OUT=<dir>`.
- `docs/investigations/job49p-fan01-auto-fallback/`: the JSON, `results.csv` and the logs, committed as soon as they
  exist.
- This ADR's addendum; one sentence appended at the end of the robustness row of `docs/milestone-fan.md`.

## Addendum: results (2026-10-31)

Data: `docs/investigations/job49p-fan01-auto-fallback/` (one JSON per arm with the round-by-round record, `results.csv`,
`logs/`). Release, on b99a352 (no solver change). Each arm was run once. The two calibration arms ran side by side; B2
ran alone. No run reached its budget (B3 about 2 min, B2 4.7 min).

### Validity

- **V1 met.** B3 has 4 920 cells; medians 1.089e-3 / 4.87e-4; torque 1.9177 N m at 1000.
- **V2 met.** B2 has 13 627 cells and is stalled: median `res_u` / `res_p` over 501-1000 1.004e-3 / 6.23e-4. Its
  reference-window means are torque 2.0308 N m and Delta p_ts 63.860 Pa. These are current reports; ADR 0086's B2
  values (1.9674 N m / 64.23 Pa) predate ADR 0170.
- **V3 met.** The bit-identity tests and `baseline_hashes.rs` pass unchanged.

### Arms

| Arm | theta | Mask at end (share) | Cap hit | `reached` | `is_converged()` | `rho_du` | Torque end / ref mean (Delta) | Delta p_ts end / ref mean |
|---|---|---|---|---|---|---|---|---|
| S-B3(0.1) | 0.1 | 417 (8.5 %) | yes, round 4 (iteration 1200) | - | false | 0.204 | 1.9258 / 1.9176 (0.43 %) | 45.43 / 46.70 |
| S-B3(0.3) | 0.3 | 442 (9.0 %) | yes, round 12 (1600) | 2453 | false | 1.3e-4 | 1.9105 / 1.9176 (0.37 %) | 44.07 / 46.70 |
| S-B3(0.03) | 0.03 | 0 | yes, round 1 (1050) | - | false | 1.929 | 1.9210 / 1.9176 (0.18 %) | 47.00 / 46.70 |
| S-B2 (held out) | 0.1 | 1 065 (7.8 %) | yes, round 4 (1200) | - | false | 2.453 | 1.9723 / 2.0308 (2.88 %) | 63.47 / 63.86 |

Arm notes:
- **S-B3(0.1).** The first round, during F-R's start-up transient, selected 262 cells; the next two rounds selected
  40 and 115 more. A further 0.34 m/s cycle then remains: an exact cycle, with `res_u` frozen at 8.488e-4.
- **S-B3(0.3).** The mask grew in 11 rounds to 442 cells. The run then converges slowly, reaching `res_u` 9.8e-7 at
  3000, but the cap flag was set at round 12, and `is_converged()` is still false at 3000.
- **S-B3(0.03).** The first round's selection already exceeded the cap, so the mask stayed empty: this is F-R alone,
  as in ADR 0177.
- **S-B2.** The mask makes B2 worse than its stall: `res_u` rises from 1.0e-3 to 7.8e-3, in an exact cycle.

Scored after the runs, as registered:
- the S-B3(0.1) mask holds 195 of H3's 322 cells (222 in H4), and S-B3(0.3) holds 179 (227);
- the masks' median rank on the ADR 0174 metric is 0.44-0.50, i.e. ordinary cells.

### Limits

| Id | Measured | Verdict |
|---|---|---|
| C1 B3 (S-B3(0.1)) | not reached | **MISSED** |
| C2 B3 | false | **MISSED** |
| C3 B3 | `rho_du` 0.204 | **MISSED** |
| S B3 | 8.5 %, cap hit | **MISSED** |
| H | torque 1.9258 (0.6 % from 1.9144); Delta p_ts 45.43 (0.7 % from 45.753) | met |
| Calibration | no theta in {0.3, 0.1, 0.03} meets C1, C2, C3 and S (0.3 meets C1 and C3 but misses C2 and S), so theta* = 0.1 and the selector is **failed on B3** by the registered rule | - |
| C1 B2 | not reached | **MISSED** |
| C2 B2 | false | **MISSED** |
| C3 B2 | `rho_du` 2.45 | **MISSED** |
| S B2 | 7.8 %, cap hit | **MISSED** |
| R1 | Delta_B3 0.0043, Delta_B2 0.0288 (> Delta_B3 and > 0.002) | **MISSED** |

Predictions: Q1, Q2 and Q3 are missed. Q4 is missed too: 8.5 % against the predicted under 6.5 %.

Known misses: `s_b3_010_known_miss`, `s_b3_030_known_miss`, `s_b3_003_known_miss`, `s_b2_known_miss`.

### Reading (by the registered rules)

**Not accepted.** By the registration, no theta meets the B3 criteria, so **rule 4 applies**: the oscillation amplitude
is not a usable selector at these parameters.

1. **The selector chases the cycle into the cap.** Each round masks where the iterate oscillates. The cycle then moves
   to the next cells (the relocation seen in ADRs 0175 and 0177), and the 10 % cap is reached before the cycle dies.
   - With theta 0.3 the growth is slower and B3 does converge numerically, but only after the cap was flagged.
   - With theta 0.03 the first round alone exceeds the cap.
   - Part of the cap is spent on the F-R start-up transient: 262 of the 417 cells came from round 1, while the
     leading-edge cycle was still decaying.
2. **B2, held out, gets worse.** On B2 the masked state cycles at eight times the stall's residual, and the torque moves
   2.9 % from the stalled mean, against 0.43 % on B3. R1 is missed. Strictly that compares a cycle with a cycle, since
   neither run converged. It is still the wrong direction for the maintainer's condition.
3. **What did work.** On B3 the automatic mask overlaps the hand-picked H3 by about 60 % and gives H's functionals
   within 1 %. So the oscillation does find the hub region; the problem is the growth rule, not the property.

Nothing here is a statement about accuracy or agreement with the measured fan. No default, limit or known miss of another
ADR changes.

### Deviations

1. **Rule 4's wording versus S-B3(0.3).** Rule 4 is worded "does not converge on B3 at any theta". S-B3(0.3) met C1
   and C3 (residual convergence) but missed C2 and S. I applied the registered calibration outcome (selector failed on
   B3), which leads to rule 4. The reading rules did not cover a partial convergence; stated here.
2. The cap rule rejects a whole round's addition and stops all later growth, as registered. S-B3(0.3) and S-B2 hit it
   in a round that wanted more cells; nothing else differed from the registration.

### Questions for the maintainer

1. **One more selector job, or move to a scheme change?** Recommendation: one cheap registered variant first.
   - It differs from this job in three ways: it starts selecting only after the F-R transient (for example 200
     iterations after the switch); it selects cells by persistence (oscillating above the threshold in several
     consecutive rounds) rather than by a single round; and it uses no ring growth.
   - Test it on B3 with B2 held out, keeping the same C1-C3, S, H and R1.
   - If it also misses, stop the selector line and register a scheme change that keeps second order in the cycling
     cells (for example a smooth limiter there), since local first order then fails the refinement condition anyway.
2. **The 10 % cap.** Is it the right bound? Recommendation: keep it. Both theta 0.1 runs stopped below it (8.5 %,
   7.8 %) and still did not converge, so a larger cap would not have saved them.
