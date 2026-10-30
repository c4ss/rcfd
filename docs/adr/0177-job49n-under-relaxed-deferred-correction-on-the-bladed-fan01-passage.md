# ADR 0177: Job 49N, the under-relaxed deferred correction (F-R) on the bladed FAN-01 passage B3

Status: **registered 2026-10-30**, before any code of this job and before any solver run. How F-R is brought back, the
validity checks, the acceptance limits, the LE2 location record, the predictions, the reading rules and the not-run list
are fixed here. Amendments follow this text, each written before what it covers; results go in an addendum. Not yet
accepted by the maintainer.

Job 49N, on branch `job49n-fan01-fr-b3` from `job49l-fan01-stall-locus` at 6a41ecf (worktree
`.claude/worktrees/job49n`). The number 0177 and the job id 49N were given by the coordinator; 0176 is job 49M.

House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
- limits are fixed here before the registered runs; a missed limit or a falsified prediction becomes an ignored
  `*_known_miss` test with the limit intact; a defective set-up gets an amendment before the corrected run;
- defaults stay bit-identical: F-R comes back as a doc-hidden, test-only field, default `None` and read only when `Some`
  (the pattern of `cyclic_partner_in_h1_test_only`, ADR 0117, and `high_order_mask_test_only`, ADR 0175); no case-schema
  key; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded;
- no new crate, no unsafe code, no new third-party crate, no commit trailers, nothing merged or pushed.

Clean-room: first principles and our own ADRs only.

## The request

The maintainer approved ADR 0175 amendment 2's question 1. ADR 0175 found two things on B3. The convection deferred
correction carries the stall (C-C). The blade-root leading edge (LE2) sustains a cycle on its own, while the hub does
not, and no one-site fix works. Its recommended next step is a global treatment of that correction: ADR 0111's F-R
(beta 0.25), which was never run on the bladed passage. Register and run it on B3, and leave the hub's wider cycle alone
unless F-R fails.

## What I read (rule d; nothing run)

- [ADR 0111](0111-wave44a-fan01-frozen-limiter-linearisation-and-relaxed-deferred-correction.md) part F.
  - **Definition.** In each momentum component, the correction source of every cell is relaxed: `c = c_prev + beta
    (c(u) - c_prev)`. This is the right-hand side of the `linear_upwind` convection beyond the upwind one, that is, the
    internal-face corrections and the periodic-face central correction; the matrices are identical. The first relaxed
    iteration takes `c(u)`. The fixed point is unchanged, and the Barth-Jespersen factors are recomputed every
    iteration.
  - **Acceptance Z2 for B3** (never run): (a) `rho_med <= 0.01` and `rho_du <= 0.01`; (b) `res_u` and `res_p` both
    below 1e-5 at some iteration of the arm; (c) the probe, one more iteration with beta = 1 from the final state, gives
    `res_u` and `res_p` <= 1e-5. The torque and Delta p_ts were to be reported without a limit.
  - **Results.** Z1 met on E3 and E1 (both converged to true Barth-Jespersen fixed points with unchanged functionals).
    Z1 missed on E2, which fell 82-fold but left a 1e-3 to 1e-2 m/s drift at the shroud tip-layer cells. So F-R was
    removed under rule (b), and its code is not in the history: it was written and deleted before the ADR 0111 commits.
  - **Linearisation.** J(beta) at the B3 cycle point predicted no stabilisation (rho 1.335-1.341), but that
    linearisation did not predict the nonlinear iteration (M3 failed).
- [ADR 0175](0175-job49l-where-the-fan01-stall-is-driven-in-the-nonlinear-iteration.md):
  - the S10 stabilisation is the convection deferred correction C (C-C), not the Laplacian correction;
  - K-LE2: with C live only in LE2, a period-two cycle of 1.87 m/s stays, all in LE2 (c665, c989, c341, c17, c988);
  - the reference at iteration 1000 has median `res_u` / `res_p` 1.089e-3 / 4.87e-4 over 500-1000, and **current**
    torque 1.9177 N m and Delta p_ts 46.643 Pa (C0's switch values; ADR 0170 changed the moment reports after ADR 0110,
    whose 1.8493 N m is therefore not used here);
  - the arm harness and the rings LE2 (46 cells) and HUB2 (83) are in `fan01_stall_locus_w49l.rs`.
- Code: `crates/rcfd-fv/src/simple.rs` `iterate`, `convection.rs` `convection` (ADR 0175's `convection_masked`).

## How F-R comes back

`SimpleSolver::deferred_correction_relaxation_test_only: Option<f64>` (doc-hidden, default `None`). It comes with a
private state of one vector per velocity component, which is reset whenever the field is `None`.

With `Some(beta)`, in each momentum component the kernel does the following:
1. It assembles the convection matrix as now (`m`).
2. It assembles the same convection with `ConvScheme::Upwind` (`m_up`; same matrix, as `convection` builds the
   coefficients from the flux alone).
3. It takes `c(u) = m.b - m_up.b` per cell.
4. It sets `c = c(u)` on the first relaxed iteration and `c = c_prev + beta (c(u) - c_prev)` after that.
5. It replaces `m.b` by `m_up.b + c` and stores `c`.

Everything else is unchanged: the limiter is recomputed every iteration, and so are the boundary inflow lag, the
Laplacian with its non-orthogonal correction, and the pressure equation. With `None` no line of the default path runs,
so defaults are bit-identical by construction. With beta = 1 the iteration equals the default up to the round-off of
`m_up.b + (m.b - m_up.b)`.

## Definitions

- **Case** B3: `case_toml(F3, true)` of `fan01_stall_locus_w49l.rs` (the ADR 0086 set-up), unchanged.
- **Arm F-R(beta)**: the ADR 0110 / 0175 switch protocol: 1000 default iterations, then `Some(beta)` set on the solver,
  then 2000 more. The windows are the last 500 iterations before the switch and the last 500 of the arm. `rho_med` is
  the larger of the ratios (arm median / reference median) of `res_u` and `res_p`; `rho_du` is `|du|_2` at the end over
  `|du|_2` at the switch.
- **Probe**: from the final state, the field is set to `None` (beta = 1, the default Barth-Jespersen map) and one more
  iteration is made; its `res_u` and `res_p` are recorded.
- **Reference band** of a functional f (torque, Delta p_ts): `[min, max]` of f over iterations 501-1000 of the same
  run (every iteration sampled), widened on each side by 0.5 % of `|mean|`. The stalled values are values of a cycle
  (ADR 0110 amendment 4), so a fixed point of the same discretisation is expected inside the cycle's range, not at one
  sample.
- **LE2 record**: at the switch, at iterations 1010, 1100, 1250, 1500, 2000 and at the end, it records:
  - max `|du|` over LE2 and over HUB2;
  - the share of `sum |du|^2` in LE2 and in HUB2;
  - the five largest-`|du|` cells, each classed LE (in LE2), HUB (in HUB2) or other.

## Validity checks (a failure is a defective set-up: amendment before anything is read)

- **V1** B3 has 4 920 cells. The reference medians are 1.089e-3 / 4.87e-4 and the torque at iteration 1000 is 1.9177
  N m, each to the printed digits.
- **V2** (non-ignored, E3) the field left `None` for 30 iterations is bit-identical to a solver that never had it (u, p,
  phi). (ignored, B3) After 30 default iterations, one iteration with beta = 1 differs from the default one by at most
  1e-12 relative in max-norm, in u, p and phi (round-off only).
- **V3** the existing bit-identity tests pass unchanged: `w44a_hooks_at_defaults_are_bit_identical`,
  `frozen_limiter_hook_is_bit_identical_at_the_freeze`, `w45a_p5_defaults_are_bit_identical_to_main`,
  `w49l_empty_mask_is_bit_identical` and `baseline_hashes.rs`.

## Acceptance (F-R(0.25) on B3), limits and predictions

| Id | Limit | Prediction |
|---|---|---|
| **Z2a** | `rho_med <= 0.01` and `rho_du <= 0.01` (ADR 0111) | met |
| **Z2b** | `res_u` and `res_p` both below 1e-5 at some iteration of the arm (ADR 0111) | met |
| **Z2c** | probe `res_u` and `res_p` <= 1e-5: the end state is a fixed point of the default Barth-Jespersen map (ADR 0111) | met |
| **Z2t** | the torque at the end lies inside its reference band | met |
| **Z2p** | Delta p_ts at the end lies inside its reference band | met |
| **Z2le** | max `|du|` over LE2 at the end <= 0.01 x its value at the switch (the leading-edge cycle that K-LE2 showed to be autonomous is removed) | met |

**Accepted on B3** iff Z2a, Z2b, Z2c, Z2t, Z2p and Z2le are all met. Reported without a limit:
- the first iteration at tolerance and `is_converged()` at the end (with `res_nut`; ADR 0111 rule G);
- the functionals at the end against the band;
- the period ratio;
- the LE2 record.

**Conditional arm F-R(0.1)**: run only if F-R(0.25) misses Z2a, with the same limits and the same record. It does not
change F-R(0.25)'s verdict. It answers whether stronger relaxation helps, and it is reported as such.

## Reading rules (fixed now; made only if V1-V3 hold)

1. **Accepted**: F-R removes the bladed stall at F3 without moving the functionals out of the cycle's range. It is
   the candidate fix. Next job:
   - first, ADR 0111's open E2 drift;
   - then B2 (and E2 / B2 at the cap), before an opt-in key is registered.
   None of these is part of this job.
2. **Z2a, Z2b, Z2c met but Z2t or Z2p missed**: the stall converges, but its converged functionals lie outside the
   stalled cycle's range. The ADR 0086 numbers are then cycle values that a converged run does not reproduce. Reported
   as a known miss; the maintainer decides whether the band was the right reference. F-R is not called accepted.
3. **Z2a missed, Z2le met**: F-R removes the autonomous leading-edge cycle, but a cycle stays elsewhere. Per the
   maintainer's decision, the next job is then the hub's wider cycle (ADR 0175 question 2: complement arms at ring 3
   / 4 of the hub), with F-R on.
4. **Z2a and Z2le both missed**: F-R does not stabilise the leading-edge driver either. Under-relaxing the correction
   is then not a fix on the bladed passage at these beta, and F-R(0.1) says whether stronger relaxation changes that.
   The next job is the hub diagnosis above plus a scheme change that is not a relaxation, to be proposed then.
5. **Z2a met, Z2b or Z2c missed**: the cycle stops but the run has not converged to a Barth-Jespersen fixed point
   within 2000 iterations. Reported; a longer arm would be a new registration.

Nothing here is a statement about accuracy or agreement with the measured fan. No result changes a default, a limit of
another ADR, or a known miss.

## Not run (and why)

- E3, E2, E1 (ADR 0111 Z1, known: E3 and E1 met, E2 missed) and B2, B1: the request is B3. They are the next job under
  reading rule 1.
- No case key, no linearisation, no mask arms, no hub ring-3 / 4 arms (reading rule 3 names them for later).
- F-R(0.1) unless F-R(0.25) misses Z2a.

## Deliverables

- `crates/rcfd-fv/src/simple.rs`: the test-only field and its state.
- `crates/rcfd-api/tests/fan01_fr_b3_w49n.rs`: V1, V2 (and V3 by running the existing tests), the F-R(0.25) arm and the
  conditional F-R(0.1) arm (ignored, about 2.5 min each in release, one per process). Each arm writes JSON with
  `W49N_OUT=<dir>`.
- `docs/investigations/job49n-fan01-fr-b3/`: the JSON, a `results.csv` and the logs, committed as soon as they exist.
- This ADR's addendum; one appended sentence at the end of the robustness row of `docs/milestone-fan.md`.
