# ADR 0178: Job 49O, the FAN-01 hub mid-chord cycle with the leading-edge driver held off by F-R

Status: **registered 2026-10-30**, before any code of this job and before any solver run. The arms, the seeds, the
validity checks, the limits, the predictions, the reading rules and the not-run list are fixed here. Amendments follow
this text, each written before what it covers; results go in an addendum. Not yet accepted by the maintainer.

Job 49O, on branch `job49o-fan01-hub-locus` from `job49n-fan01-fr-b3` at 145ee57 (worktree `.claude/worktrees/job49o`).
The number 0178 and the job id 49O were given by the coordinator.

House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
- limits are fixed here before the registered runs; a missed limit or a falsified prediction becomes an ignored
  `*_known_miss` test with the limit intact; a defective set-up gets an amendment before the corrected run;
- defaults stay bit-identical. This job is expected to need **no solver change**: it combines the two existing
  test-only fields, `high_order_mask_test_only` (ADR 0175) and `deferred_correction_relaxation_test_only` (ADR 0177),
  both kept by the maintainer. No case-schema key. `baseline_hashes.rs` is not re-recorded;
- no new crate, no unsafe code, no new third-party crate, no commit trailers, nothing merged or pushed.

Clean-room: first principles and our own ADRs only.

## The request

The maintainer approved ADR 0177's question 1: diagnose the hub cycle that takes over on B3 when F-R(0.25) removes the
leading-edge cycle.
- Keep F-R(0.25) on throughout, so that the leading-edge driver stays off.
- Use complement and necessity arms at ring sizes 3 and 4.
- Extend the hub seeds by c149, c113 and the other z 0.17 cells.
- Include controls that reproduce the F-R(0.25) reference and C-C.

## What I read (rule d; nothing run)

- [ADR 0175](0175-job49l-where-the-fan01-stall-is-driven-in-the-nonlinear-iteration.md): the stall is carried by the
  convection deferred correction C (C-C CALMS: `rho_med` 3.957e-3, `rho_du` 6.897e-3, `reached` 1115). The blade-root
  leading edge (LE2) sustains a cycle alone. Neither hub ring 1 nor ring 2 of the ADR 0111 seeds sustains one or is
  necessary. When the leading edge is masked, the cycle reappears at hub cells at z 0.16-0.18.
- [ADR 0177](0177-job49n-under-relaxed-deferred-correction-on-the-bladed-fan01-passage.md): F-R(0.25) from iteration
  1000 gives `rho_med` 1.038 and `rho_du` 1.929. Max `|du|` over LE2 goes from 2.287 to 0.0103 m/s. A 10-15 m/s
  cycle then grows at hub cells at r 0.124-0.126, z 0.170-0.177. Torque 1.9210 N m and Delta p_ts 46.999 Pa at the
  end; reference torque at 1000 1.9177 N m (current reports, ADR 0170). F-R(0.1) is far worse.
- Code: both test-only fields act in the same momentum assembly, the mask first (`convection_masked`), then F-R on
  what is left. Where every face of a cell is masked, F-R's `c(u)` is exactly zero there.

## Definitions

- **Case** B3, `case_toml(F3, true)` as in `fan01_fr_b3_w49n.rs`, unchanged.
- **Arm**: 1000 default iterations. At the switch, `deferred_correction_relaxation_test_only = Some(0.25)` and the
  arm's C-mask are set (`lap` region empty in every arm). Then 2000 more iterations. Windows, `rho_med` and `rho_du`
  are as ADR 0175, against the same default-stall reference window (iterations 501-1000).
- **Hub seeds** S_H (19 cells, fixed now from the records):
  - the ADR 0111 cells {c1234, c365, c401, c1250, c1233};
  - every cell in the top five of ADR 0177's F-R(0.25) LE2 record from iteration 1010 to the end: {c907, c906,
    c581, c905, c582, c149, c437, c150, c438, c402, c113, c114, c77, c257} (c401 is in both lists).
- **Rings** H3, H4: S_H grown 3 and 4 times by face neighbours, seam partners included (the ADR 0175 rule). LE2 is
  the ADR 0175 ring.
- **Distant seeds** S_D': the 19 cells nearest to (r 0.20 m, z 0.30 m) in the (r, z) plane, downstream of the blade
  at mid-span. They are away from the hub, the leading edge, and the r 0.20-0.23, z 0.11-0.14 cells where ADR 0177's
  F-R(0.1) cycle spread. D3 is ring 3 of S_D'.
- **Location class** of a cell: LE (in LE2), HUB (in H4), D (in D3), other.

| Id | C removed in | Kind |
|---|---|---|
| **F0** | nothing (F-R(0.25) alone) | control: ADR 0177 F-R(0.25) |
| **FC** | ALL | control: C-C (F-R acts on nothing) |
| **N-H3** | H3 | necessity |
| **N-H4** | H4 | necessity |
| **K-H3** | ALL minus H3 | complement (C, relaxed, live only inside H3) |
| **K-H4** | ALL minus H4 | complement |
| **N-D3** | D3 | distant necessity control |
| **K-D3** | ALL minus D3 | distant complement control |

**Verdicts.**
- *Necessity arms* use the ADR 0175 rule: CALMS iff `rho_med <= 0.01` and `rho_du <= 0.01`; NO EFFECT iff
  `rho_med >= 1/3`; PARTIAL otherwise. Reported with it: the end location (the five largest-`|du|` cells and their
  classes) and the shares of `sum |du|^2` in LE2 and H4.
- *Complement arms* use the ADR 0175 amendment 2 rule:
  - CALMS as above;
  - SUSTAINS iff not CALMS, `rho_du >= 0.1`, and at least 3 of the 5 end top cells are in the arm's own site (H4 for
    K-H3 and K-H4, D3 for K-D3);
  - WEAK otherwise.

## Validity checks (a failure is a defective set-up: amendment before anything is read)

- **V1** B3 has 4 920 cells. In every arm the reference medians are 1.089e-3 / 4.87e-4 and the torque at iteration
  1000 is 1.9177 N m, to the printed digits.
- **V2** F0 reproduces ADR 0177's F-R(0.25) record: `rho_med` 1.038, `rho_du` 1.929, max `|du|` over LE2 at the end
  1.032e-2, to the printed digits.
- **V3** FC reproduces ADR 0175 C-C: `rho_med` 3.957e-3, `rho_du` 6.897e-3, `reached` 1115.
- **V4** printed before any arm: |S_H| = 19 and the sizes of H3, H4, D3, and H3 ∩ LE2, H4 ∩ LE2. Required:
  - D3 is disjoint from H4 and from LE2;
  - |D3| is within a factor 2 of |H3|.
  An overlap of H3 or H4 with LE2 is reported, not a defect: F-R holds the leading edge off in every arm.
- **V5** the existing bit-identity tests pass unchanged: the three in `fan01_stall_w43c.rs`,
  `w49l_empty_mask_is_bit_identical`, `w49n_field_none_is_bit_identical` and `baseline_hashes.rs`.

## Limits (my predictions)

| Id | Limit | Prediction |
|---|---|---|
| **P1** | N-H3 does not CALM | holds (ring 3 too small, as rings 1-2 were) |
| **P2** | N-H4 CALMS | holds |
| **P3** | K-H3 SUSTAINS | holds |
| **P4** | K-H4 SUSTAINS | holds |
| **P5** | N-D3 does not CALM | holds |
| **P6** | K-D3 CALMS | holds |

## Reading rules (fixed now; made only if V1-V4 hold)

"Hub necessary" iff N-H3 or N-H4 CALMS. "Hub sufficient" iff K-H3 or K-H4 SUSTAINS.

0. **N-D3 CALMS or K-D3 SUSTAINS**: the controls are not specific (any patch of this size stops or sustains the cycle).
   No hub reading is made, and the fix question stays where ADR 0177 left it.
1. **Hub necessary and sufficient**: with the leading edge held off, the hub mid-chord region is the second autonomous
   driver, and removing C there (plus F-R) stops the stall. For a fix: F-R combined with a local treatment of the
   hub region is a working configuration at F3. It cannot become a fix while the cells are hand-picked. The next job
   must find what distinguishes these cells (geometry, flux, limiter state) so that an automatic cell rule can be
   registered and tested on B2.
2. **Hub necessary, not sufficient**: removing C in the hub region stops the stall, but the region cannot drive a cycle
   alone; the hub cycle needs the wider field. For a fix: the local treatment works at F3, but its mechanism is not
   local, so a cell rule may not transfer. Next job as rule 1, with B2 as the deciding test.
3. **Hub sufficient, not necessary**: the hub region is a driver, but removing it lets another site take over (as ADR
   0175's U2 exposed a mid-span set). The chain of local drivers is open-ended. For a fix: local treatments are not
   viable. The next job is a global scheme change that is not an under-relaxation (F-R(0.1) was worse), to be
   proposed and registered.
4. **Neither**: the hub cycle is not localised within ring 4. For a fix: as rule 3, a global non-relaxation scheme
   change.

A PARTIAL or WEAK arm is reported with its numbers and enters no rule. Nothing here is a statement about torque, Delta
p_ts, accuracy or agreement with the measured fan. No result changes a default, a limit of another ADR, or a known
miss.

## Not run (and why)

- Rings 1-2 of the new seeds: ADR 0175 already showed rings 1-2 of the ADR 0111 seeds neither necessary nor sufficient.
- Without F-R: ADR 0175 covers it.
- F-R at other beta: ADR 0177 covers it.
- E3, E2, E1 and B2: the question is the B3 hub cycle.
- No fix candidate, no case key, no linearisation.

## Deliverables

- `crates/rcfd-api/tests/fan01_hub_locus_w49o.rs`: V1-V4, the eight arms (ignored, about 2 min each in release, one per
  process, at most two at a time). Each arm writes JSON with `W49O_OUT=<dir>`.
- `docs/investigations/job49o-fan01-hub-locus/`: the JSON, `results.csv` and the logs, committed as soon as they exist.
- This ADR's addendum; one sentence appended at the end of the robustness row of `docs/milestone-fan.md`.

## Amendment 1 (2026-10-30, after the V4 check, before any arm)

What I knew when writing it: only the V4 printout. No arm has run. |S_H| is 19, |H3| 322, |H4| 512, |D3| 351, |LE2|
46. H3 and H4 are disjoint from LE2. **V4 failed**: D3 shares 4 cells with H4, because the hub rings reach 4 cells
into the distant ring. This is a defective set-up of mine: I chose the distant point without knowing how far ring 4
reaches.

Correction (fixed now): the distant region is **D3' = D3 minus H4** (347 cells, disjoint from H4 and LE2; |D3'| / |H3|
= 1.08). N-D3, K-D3, the class "D" and K-D3's own site use D3'. Everything else is unchanged, including V4's
requirements, now applied to D3'.
