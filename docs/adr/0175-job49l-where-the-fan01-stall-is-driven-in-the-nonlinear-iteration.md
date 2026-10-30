# ADR 0175: Job 49L, where the FAN-01 steady stall is driven in the nonlinear iteration: the blade-root leading edge or the hub mid-chord

Status: **registered 2026-10-30**, before any code of this job and before any solver run. The hook, the masks, the
validity checks, the limits, the predictions, the reading rules and the not-run list are fixed here. Phase 2 is
registered here too; its term set is chosen by a rule fixed now from phase 1. Amendments follow this text, each written
before what it covers; results go in an addendum. Not yet accepted by the maintainer.

Job 49L, on branch `job49k-fan01-wall-metric` at d5d6301 (worktree branch `job49l-fan01-stall-locus`). Number 0175 and
job id 49L were reserved for this job; 0176 is reserved for another job running in parallel.

House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
- limits are fixed here before the registered runs; a missed limit or a falsified prediction becomes an ignored
  `*_known_miss` test with the limit intact; a defective set-up gets an amendment before the corrected run;
- defaults stay bit-identical: the one solver change is a doc-hidden test-only field, default `None`, read only when
  `Some` (the pattern of `cyclic_partner_in_h1_test_only`, ADR 0117); no case-schema key;
  `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded;
- no new crate, no unsafe code, no new third-party crate, no commit trailers, nothing merged or pushed.

Clean-room: first principles and our own ADRs only.

## The request

ADR 0174 question 2: the next stall job is on the scheme side, but the record points at two places. Locate the drive of
the FAN-01 Barth-Jespersen stall in the actual nonlinear iteration, at the blade-root leading-edge cells or at the hub
mid-chord cells, so that a later job can choose a scheme fix. This job registers, runs and writes up; it adds no fix.

## What I read (rule d; nothing run)

- [ADR 0110](0110-wave43c-fan01-rotor-passage-non-convergence-diagnosis.md): on B3 (bladed F3, 4 920 cells, the ADR 0086
  set-up) the residuals stall from iteration ~100 in a cycle of about three iterations (median `res_u` / `res_p` over
  500-1000 1.09e-3 / 4.87e-4; torque 1.8493 N m at 1000); 75-78 % of `sum |du|^2` in the largest 1 %, at the blade-root
  leading edge c665, c17, c341, then c701, c53. S1 (limiter frozen at 1000): the leading-edge cells grow to 17 m/s. S10
  (`upwind` from iteration 1000): STANDS, `rho_med` 4.3e-3, `rho_du` 7.4e-3, both residuals below 1e-5 at iteration 1114.
  S12 (limiter frozen at its window minimum): the leading-edge cells calm down and the oscillation moves to hub seam
  cells at z 0.16-0.17 (FALSIFIED, `rho_med` 1.02). Falsified causes: pressure-velocity coupling, SST, seam gradient,
  linear-solver tolerance, outlet backflow, explicit Coriolis source.
- [ADR 0111](0111-wave44a-fan01-frozen-limiter-linearisation-and-relaxed-deferred-correction.md): the frozen-limiter
  Jacobian at the iteration-1000 state has its leading eigenvalue -1.33 with the mode at hub mid-chord cells c1234,
  c365, c401, c1250, c1233 (0.1 % of the energy in the five leading-edge cells); holding the convection gradient of c17
  or of the five cells (H17, H5) or swapping c17's gradient formula (GG17) changed rho by less than 1e-5; holding every
  convection gradient (Hall) lowered it to 1.29 only. M3 failed, so none of this is attributed. Branch 5 named "the
  Laplacian's non-orthogonal correction" as the remaining untested candidate.
- [ADR 0174](0174-job49k-the-49h-wall-cell-metric-on-the-fan01-meshes.md): both cell sets are mild cells on the wall-cell
  metric (20th-30th percentile); V2 confirmed that the cell numbering of ADRs 0110 and 0111 holds at d5d6301.
- Code (`crates/rcfd-fv/src/simple.rs` `iterate`, `convection.rs` `convection`, `diffusion.rs` `laplacian`). **S10 changed
  three terms of the momentum equation, not one**, which ADR 0110 did not state:
  1. the internal-face high-order correction `phi_f (u_HO - u_UD)` (zero for `Upwind`);
  2. the central deferred correction on periodic seam faces (`periodic_correction()` is 1 for `linear_upwind`, 0 for
     `upwind`); it does not use the gradient;
  3. the explicit non-orthogonal (secondary-gradient) correction of the momentum Laplacian, internal and Dirichlet
     boundary faces: with `upwind` and `non_orthogonal_correctors = 0` (the FAN-01 case) the kernel computes no
     gradient at all and passes `None` to `laplacian`.
  ADR 0111's Hall held the convection gradients but left terms 2 and 3 live. ADR 0110's reading of S10 ("the
  high-order part of `linear_upwind` drives the cycle") is therefore not yet separated from term 3. Terms 1 and 2
  together are called **C** (the convection deferred correction) and term 3 **L** below.
- `crates/rcfd-api/tests/fan01_stall_w43c.rs`: `case_toml(F3, true, "", false)`, `arm(...)`, `verdict`, `location`,
  `cell_attributes`, reused unchanged (copied into this job's test file where they are private).

## The hook (written after this registration)

`SimpleSolver::high_order_mask_test_only: Option<HighOrderMask>` (doc-hidden), with
`HighOrderMask { conv: Vec<bool>, lap: Vec<bool> }`, one entry per cell (the regions R_C and R_L). Default `None`: no
branch of the kernel changes. With `Some`:

- **Face rule.** An internal face is masked for a term iff its owner or its neighbour is in that term's region. A
  boundary face is masked iff its owner is in the region, or, for a periodic seam face, the owner of its partner face is.
- **C**: on a face masked for C the convection deferred correction is zero: the internal-face correction `q_f = 0` and,
  on a seam face, no central correction (the upwind treatment between the cell and its lagged partner stays, as for
  `upwind`).
- **L**: on a face masked for L the explicit non-orthogonal correction of the momentum Laplacian is zero (internal faces
  and Dirichlet boundary faces). The matrix is unchanged.
- The limiter, the gradients and every other term are untouched. Only the momentum equation is affected (turbulence has
  its own scheme).

Implemented as private `_impl` variants of `convection` and `laplacian` with an optional face mask (the public
functions call them with `None`), so the default path keeps its arithmetic.

## Definitions

- **Case**: `case_toml(F3, true, "", false)` of `fan01_stall_w43c.rs` (B3), unchanged.
- **Arm**: the ADR 0110 switch protocol: 1000 reference iterations, then the mask is set on the solver, then 2000 more;
  windows the last 500 before the switch and the last 500 of the arm. `rho_med` = the larger of the ratios (arm median /
  reference median) of `res_u` and `res_p`; `rho_du` = `|du|_2` at the end / `|du|_2` at the switch.
- **Arm verdict** (ADR 0110 part S, unchanged): **CALMS** (ADR 0110 "STANDS") iff `rho_med <= 0.01` and
  `rho_du <= 0.01`; **NO EFFECT** (ADR 0110 "FALSIFIED") iff `rho_med >= 1/3`; **PARTIAL** otherwise. Reported with it:
  the first iteration with `res_u` and `res_p` both below 1e-5 (`reached`), `is_converged()` at the end, torque and
  Delta p_ts at the end.
- **Seeds**: S_LE = {c17, c665, c701, c341, c53} (ADR 0110); S_HUB = {c1234, c365, c401, c1250, c1233} (ADR 0111);
  S_D = the five cells whose centres are nearest to (r 0.22 m, z 0.15 m) in the (r, z) plane (a distant control: mid-span
  of the outer half of the passage, mid-chord, away from both sites).
- **Rings**: X_0 = S_X; X_k = X_(k-1) plus every cell that shares a face with a cell of X_(k-1), where a periodic seam face
  links its owner to the owner of its partner face. LE1 = LE_1, LE2 = LE_2, HUB1, HUB2, D2 likewise; U1 = LE1 ∪ HUB1,
  U2 = LE2 ∪ HUB2. ALL = every cell.
- **Location record at the end of an arm**: the five largest-`|du|` cells of the last iteration (`location` of ADR 0110),
  each classified as **LE** (in LE2), **HUB** (in HUB2) or **other**; the site of the arm's remaining cycle is the class
  with at least 3 of the 5 cells, else **mixed**. Also printed: the share of `sum |du|^2` in LE2, in HUB2 and in the top
  1 %.

## Validity checks (a failure is a defective set-up: amendment before anything is read)

- **V1** B3 has 4 920 cells, and the ADR 0174 V2 positions hold to 6e-4 m in r and z: c665 (0.1256, 0.1003), c17
  (0.1243, 0.0998), c341 (0.1249, 0.1000), c1234 (0.1408, 0.149), c365 and c401 (0.1249, 0.172).
- **V2** (hook exactness, B3, after 30 default iterations, one more iteration compared bit for bit in u, p and phi):
  (a) the empty mask (both regions empty) equals the default iteration; (b) the full mask (both regions ALL) equals the
  iteration with the ADR 0110 `upwind` switch. Plus the non-ignored default test: E3, 30 iterations with an empty mask,
  bit-identical to no mask.
- **V3** the reference reproduces ADR 0110 / 0111 to their printed digits: median `res_u` / `res_p` over 500-1000
  1.089e-3 / 4.87e-4, torque 1.8493 N m at iteration 1000.
- **V4** the rings: LE2 ∩ HUB2 is empty; |D2| is within a factor 2 of |LE2| and D2 is disjoint from LE2 and HUB2. All
  ring sizes printed before any arm runs.
- **V5** arm C-CL (below) reproduces S10: `reached` 1114, `rho_med` 4.3e-3, `rho_du` 7.4e-3 to the printed digits (it
  follows from V2 (b) if the iteration is deterministic; checked, not assumed).

## Phase 1: which term carries the S10 stabilisation (region ALL)

| Id | Mask | Prediction |
|---|---|---|
| **C0** | empty (both regions empty; the stalled reference continued, and its location record at 3000) | NO EFFECT |
| **C-CL** | R_C = R_L = ALL (= S10, control V5) | CALMS |
| **C-C** | R_C = ALL, R_L empty | CALMS |
| **C-L** | R_L = ALL, R_C empty | NO EFFECT |

**Limits.** P0: C0 is NO EFFECT (the empty mask does not disturb the stall). P1: C-C CALMS. P2: C-L is NO EFFECT. Each
is a prediction of mine; a miss is reported as a known miss and does not change phase 2's set-up except through the
rule below.

**Term set T of phase 2 (rule fixed now):** T = C if C-C CALMS; else T = L if C-L CALMS; else T = C+L (both regions set
to the arm's mask) if C-CL CALMS; else (C-CL does not calm, so V5 failed) phase 2 is not run without an amendment. If
both C-C and C-L calm, T = C and that is reported as a finding.

## Phase 2: the locus (mask on the terms T only, the other term live everywhere)

| Id | Region | Prediction |
|---|---|---|
| **A-LE1** | LE1 | NO EFFECT, the cycle relocates to HUB |
| **A-LE2** | LE2 | NO EFFECT or PARTIAL, the cycle relocates to HUB |
| **A-HUB1** | HUB1 | NO EFFECT, the cycle stays at LE |
| **A-HUB2** | HUB2 | NO EFFECT, the cycle stays at LE |
| **A-U1** | U1 | PARTIAL |
| **A-U2** | U2 | CALMS |
| **A-D2** | D2 | NO EFFECT, the cycle stays at LE |

My predictions above amount to "two drivers": either site alone can sustain a cycle (S12 is the reason). Limits: **P3**
A-U2 CALMS; **P4** no single-site arm (A-LE1, A-LE2, A-HUB1, A-HUB2) CALMS; **P5** A-D2 does not CALM; **P6** the cycle of
A-LE2 sits at HUB by the location record. Each is checked against its arm's numbers, and a miss is a known miss.

## Reading rules (fixed now; applied in this order)

A site X calms iff A-X1 or A-X2 CALMS. The reading is made only if V1-V5 hold.

1. **A-D2 CALMS**: a first-order patch away from both sites also stops the cycle; local masks cannot localise this stall,
   and no locus is claimed (rules 2-6 void). Report the location records only.
2. **LE calms, HUB does not**: the stall is driven at the blade-root leading edge (the term set T there is necessary for
   the cycle). A scheme fix should target the LE root cells; the ADR 0111 hub mode is not the nonlinear driver.
3. **HUB calms, LE does not**: the stall is driven at the hub mid-chord cells, where the ADR 0111 mode sits; the large
   amplitude at the leading edge is a response. A scheme fix should target the hub mid-chord (seam) cells.
4. **Both calm**: one coupled cycle that needs both sites (removing T at either breaks it); the fix may target either,
   the smaller ring first.
5. **Neither calms, A-U1 or A-U2 CALMS**: two drivers, each able to sustain a cycle alone; a fix must cover both sites.
   If the LE arm's record moves the cycle to HUB (or the reverse), that is the relocation of S12, now in the live
   iteration.
6. **Nothing calms** (no single-site arm, no union): the drive is not inside ring 2 of either site; not decided here.
   The location records say where the cycles sit; a larger ring or another region would be a new registration.

"Calms" is the registered threshold. A PARTIAL arm is reported with its numbers and does not enter the rules. Nothing
here is a statement about torque, Delta p_ts, accuracy, or agreement with the measured fan. No result changes a default,
a limit of another ADR, or a known miss.

## Not run (and why)

- E3, E2, E1 (empty passage) and B2: the question is the bladed B3 stall, where both sites and all the record exist; a
  B2 locus would need the cell sets mapped to B2 first (not on record). Listed for a later job.
- No fix, no case key, no linearisation, no limiter change, no ring larger than 2 (a new registration if rule 6 applies).
- The ADR 0110 S10 run itself is not repeated; C-CL is its bit-identical twin by V2 (b) and is checked by V5.

## Deliverables

- `crates/rcfd-fv/src/simple.rs`, `convection.rs`, `diffusion.rs`: the hook (doc-hidden, default `None`).
- `crates/rcfd-api/tests/fan01_stall_locus_w49l.rs`: V1-V5, the eleven arms (ignored, about 2.5 min each in release,
  each run alone in its own process, at most two at a time), the non-ignored default test; each arm writes its numbers
  as JSON with `W49L_OUT=<dir>`.
- `docs/investigations/job49l-fan01-stall-locus/`: one JSON per arm and `results.csv`, committed as soon as they exist.
- The existing bit-identity tests run unchanged: `w44a_hooks_at_defaults_are_bit_identical`,
  `frozen_limiter_hook_is_bit_identical_at_the_freeze`, `w45a_p5_defaults_are_bit_identical_to_main`
  (`fan01_stall_w43c.rs`) and `baseline_hashes.rs`.
- This ADR's addendum; a line in `docs/milestone-fan.md`'s robustness row.
