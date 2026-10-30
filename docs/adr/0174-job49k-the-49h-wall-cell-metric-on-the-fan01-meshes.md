# ADR 0174: Job 49K, the ADR 0171 wall-cell metric on the FAN-01 meshes: is the FAN-01 stall the trailing-edge mechanism?

Status: **registered 2026-10-30**, before any code of this job and before any computation on a FAN-01 mesh. The cell sets,
the populations, the limits, the predictions, the reading rules and the not-run list are fixed here. Amendments follow this
text, each written before what it covers; results go in an addendum. Not yet accepted by the maintainer.

Job 49K, on local main edb4839 (worktree branch `job49k-fan01-wall-metric`). Number: 0172 is the highest on main; 0173 is
taken on the branch `studio-case-lineage` (uncommitted there), so 0174. Job ids 49A to 49J are taken.

House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
- limits are fixed here before the registered tests run; a missed limit or a falsified prediction becomes an ignored
  `*_known_miss` test with the limit intact; a defective set-up gets an amendment before the corrected run;
- no solver, schema, default or run-output change: this job adds one test file and this ADR, so defaults are bit-identical
  by construction and `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded;
- no new crate, no unsafe code, no new third-party crate, no commit trailers, nothing pushed.

Clean-room: our own code and our own ADRs only.

## The request

The maintainer chose (2026-10-30), from four candidates for the next fan job, "Link 49H to FAN-01": test whether the
mechanism of [ADR 0171](0171-job49h-the-naca0012-trailing-edge-divergence-at-fine-wall-spacing.md) (unlimited
`linear_upwind` extrapolating far outside the neighbour range over the long side of thin, strongly non-orthogonal wall
cells, measured by `aspect x tan(non-orthogonality)`, the SV302 metric of [ADR 0172](0172-job49j-set-up-warnings-sv301-at-the-settled-step-one-warning-path-linear-upwind-wall-risk-sv302-guard-hint-and-correctors-in-the-plots.md))
is what stalls the FAN-01 rotor passage, and whether it can say anything about the growth of the spurious outlet angular
momentum with refinement (ADR 0110 part L). The purpose is to decide whether the next large fan job belongs on the mesh
side (cell shape) or on the scheme side.

Correction to the option as offered: it proposed full-length FAN-01 runs with `barth_jespersen`. Those exist (ADR 0086,
ADR 0110: the limit cycle is a Barth-Jespersen run), so none is repeated here.

## What I read (nothing run)

- ADR 0110 part S: on B3 (bladed F3, 4 920 cells) the Barth-Jespersen limit cycle is concentrated (75-78 % of
  `sum |du|^2` in the largest 1 %) at the blade-root leading edge, c665 (blade, r 0.1256, z 0.1003, 28 deg), c17 (hub and
  blade, r 0.1243, z 0.0998, 26 deg), c341 (blade, r 0.1249, z 0.1000), then c701, c53 (the five cells of ADR 0110
  amendment 5 and ADR 0111); "none is near the 83-degree faces". R-U: unlimited `linear_upwind` on E3 (empty F3, 5 040
  cells) fails first at c4971 (tip layer at the shroud next to the seam, r 0.2477, z 0.104, 77 deg), then c4935, c4970
  (69-77 deg); guard trip at iteration 7.
- ADR 0111 amendment 1: the leading mode of the frozen-limiter Jacobian at the B3 cycle point lies at hub-region mid-chord
  cells c1234 (seam, r 0.1408, z 0.149, 59 deg), c365 and c401 (interior, r 0.1249, z 0.172, 38 and 56 deg), c1250, c1233
  (seam); the linearisation did not predict the frozen iteration (M3 failed), so this location is reported, not attributed.
- ADR 0110 part L, ADR 0117: on the empty family E3 / E2 / E1 (ADR 0086 counts F3 / F2 / F1, first cell and wall stacks not
  refined) abs(L) is 0.0095 / 0.0596 / 0.1060 N m against an exact 0; mechanisms L1, L2 and F1 are falsified or only
  contribute.
- ADR 0171 finding 3 and ADR 0172 D3: the metric values on the aerofoil meshes (about 21 000 on M1, 6 900 on M1w05, both
  diverged at 6 degrees; 1 600 on M1w15, clean) and the SV302 level 3 000; the metric alone does not predict divergence.
- `crates/rcfd-api/tests/fan01_stall_w43c.rs`: the meshes and cases are built from `mesh_toml` / `case_toml`; ADR 0116 G6
  records that the E3 and B3 content hashes are unchanged since e65b4ea, so the cell numbering of ADRs 0110 and 0111 is
  expected to hold (checked by V2 below, not assumed).

## Definitions

- **Meshes and cases**: exactly those of `fan01_stall_w43c.rs` (`case_toml(counts, blade, "", false)`): B3 / E3 (F3), B2 /
  E2 (F2), B1 / E1 (F1). The unlimited variant replaces `linear_upwind_limiter = "barth_jespersen"` by `"none"` and nothing
  else.
- **Metric** of a cell c: `m_c = aspect_c * tan(min(theta_c, 89.9 deg))`, with `aspect_c` = `rcfd_mesh::quality::
  compute_metrics(mesh).aspect_ratio[c]` and `theta_c` the largest `non_orthogonality` over the faces of c, a NaN read as 0:
  the formula of `rcfd_api::rules::wall_cell_risk`, applied to every cell.
- **Populations**: A = all cells; W = cells owning a face of a patch for which `rules::is_wall_patch` is true (the SV302
  population; hub, shroud and, on B, blade).
- **Rank**: the percentile of `m_c` in a population P, `#{d in P : m_d <= m_c} / |P|`. "Top k %" means rank `> 1 - k/100`.
- **Cell sets** (from the record above): S_RU = {c4971, c4935, c4970} on E3; S_LC = {c17, c665, c701, c341, c53} on B3;
  S_LM = {c1234, c365, c401, c1250, c1233} on B3.

## Validity checks (a failure is a defective set-up: amendment before anything is read)

- **V1**: cell counts B3 4 920, B2 13 627, B1 38 184 (ADR 0086) and E3 5 040 (ADR 0110), exactly. The E2 and E1 counts
  are not on record and are printed.
- **V2**: the recorded positions hold to 6e-4 m in r and z: c665 (0.1256, 0.1003), c17 (0.1243, 0.0998), c341 (0.1249,
  0.1000) on B3; c1234 (0.1408, 0.149), c365 and c401 (0.1249, 0.172) on B3; c4971 (0.2477, 0.104) on E3.
- **V3**: the harness angle of `fan01_stall_w43c.rs::cell_attributes` (largest angle between the owner-neighbour vector
  and the face normal) reproduces the recorded 28 / 26 deg (c665 / c17), 59 deg (c1234) and 77 deg (c4971) to 0.6 deg.
  It is the recorded angle, not the metric's angle; both are printed for every cell of the three sets.

## Limits and predictions

Every value is printed whatever the verdict. The predictions are mine, written now.

| Id | What | Verdict rule | Prediction |
|---|---|---|---|
| **K1** | the 49H mechanism at the unlimited-failure locus (E3) | STANDS if each cell of S_RU is in the top 1 % of A **and** of W on E3, and each has `m >= 3000`; FALSIFIED otherwise | STANDS |
| **K2** | the 49H mechanism at the Barth-Jespersen limit cycle (B3) | STANDS if at least 3 of the 5 cells of S_LC are in the top 5 % of A on B3; FALSIFIED otherwise | FALSIFIED |
| **K3** | the 49H mechanism at the leading Jacobian mode (B3) | as K2 for S_LM | FALSIFIED |
| **K4** | does the metric track part L (E3 -> E2 -> E1)? | with `N` = number of cells of A with `m >= 3000` and `M` = max of m over A: TRACKS if both N and M increase strictly from E3 to E2 to E1; DOES NOT TRACK if either decreases strictly at either step; otherwise UNDECIDED | DOES NOT TRACK (in-plane refinement with fixed wall stacks shortens the long edges, so the aspect ratio falls) |
| **D1** | SV302 on FAN-01 | `rules::dynamic_setup_diagnostics` of the unlimited E3 and B3 simulations each contain one SV302 warning; the Barth-Jespersen ones (the ADR 0086 set-up) contain none | met |
| **D2** | where SV302 points | the cell SV302 names on unlimited E3 is in S_RU or within 0.01 m of c4971 | met |

Reported without a verdict: for each of the six meshes, |A|, |W|, N, M and its cell (r, z, patches), the 99th and 50th
percentile of m over A and W; the rank in A and W, m, aspect and both angles of every cell of S_RU, S_LC, S_LM; on B3 the
ten largest-m cells.

## Reading rules (fixed now)

- K1 STANDS and K2, K3 FALSIFIED: the FAN-01 passage has two separate defects. The unlimited divergence is the 49H
  mechanism (a mesh-side fix such as a C-grid without the sheared tip-layer cells, or the limiter, removes it); the
  Barth-Jespersen limit cycle is not, and a mesh change aimed at high-metric cells is not expected to remove it. The next
  job for the stall is then on the scheme side at the leading-edge root (the record says the high-order part there is
  locally unstable).
- K2 or K3 STANDS: the limit cycle sits on high-metric cells; the next job is mesh-side (the ADR 0087 index-offset tip
  topology, or a C-grid change at the root), with the metric as its acceptance measure.
- K1 FALSIFIED: the unlimited divergence on FAN-01 is not explained by the metric; ADR 0171's mechanism is not claimed for
  FAN-01, and SV302 is recorded as a false negative or a lucky positive there (D2 says which).
- K4 TRACKS: the metric is a candidate for part L (correlation only; a mechanism needs a run). DOES NOT TRACK: the metric is
  not the cause of part L, which stays open. UNDECIDED: nothing is claimed.
- Nothing in this job is a statement about torque, Delta p_ts or agreement with the measured fan. No result here changes a
  default, a limit of another ADR, or a known miss.

## Not run (and why)

- No solver iteration on any mesh: every quantity is a property of the mesh and the built simulation (`Simulation::new`
  only). The unlimited runs on E2, E1, B2, B1 (where would they fail?) are not run; they would be the first step of a
  follow-up if K1 stands.
- No mesh change, no new generator option, no limiter change.
- The aerofoil meshes of ADR 0171 are not recomputed; their values are quoted.

## Deliverables

- `crates/rcfd-api/tests/fan01_wall_metric_w49k.rs`: V1-V3, K1-K4, D1, D2 as tests (the mesh construction copied from
  `fan01_stall_w43c.rs` unchanged; the F1 members ignored with a time note if they take over a minute in debug), the report
  printed with `--nocapture`.
- `docs/investigations/job49k-fan01-wall-metric/results.json`: the printed values, committed with the results.
- This ADR's addendum; a line in `docs/milestone-fan.md`'s robustness row.

## Addendum: results (2026-10-30)

Data: `docs/investigations/job49k-fan01-wall-metric/results.json`, written by the `report` test (`W49K_OUT`); one run, release,
on 12603be plus the test file. Every value below is from that file.

### Validity

V1 met (B3 4 920, B2 13 627, B1 38 184, E3 5 040; E2 13 882 and E1 38 688 printed). V2 and V3 met: the cell numbering of ADRs
0110 and 0111 holds, and for every cell of the three sets the harness angle equals the metric's angle.

### Limits

| Id | Measured | Verdict |
|---|---|---|
| K1 | c4971: rank 1.000 in A and W (it is the largest-m cell of E3), m 750; c4970: rank 0.999, m 585; c4935: rank 0.955 (W 0.948), m 130 | **FALSIFIED** (known miss: m < 3000 for all three, c4935 outside the top 1 %) |
| K2 | 0 of 5 in the top 5 %: ranks 0.206-0.277, m 3.7-5.4, aspect 7.0-8.4, 26-36 deg | FALSIFIED, as predicted |
| K3 | 0 of 5 in the top 5 %: ranks 0.207-0.320, m 3.7-6.7, aspect 3.6-4.7, 38-59 deg | FALSIFIED, as predicted |
| K4 | N (m >= 3000) 0 / 0 / 0; M 750 / 708 / 1497 on E3 / E2 / E1 | DOES NOT TRACK, as predicted (M falls E3 -> E2) |
| D1 | SV302 count, unlimited / Barth-Jespersen: 0 / 0 on E3 and on B3 | **MISSED** (known miss) |
| D2 | `wall_cell_risk` on E3 names c4971 | met |

Reported: the largest m on every mesh is a shroud tip-layer cell at the leading-edge plane (r 0.2477-0.2491, z 0.093-0.104, 77
deg on F3): B3 / E3 750, B2 / E2 708, B1 / E1 1 497; the 99th percentile over all cells is 196-286 and the median 5.6-12.5. No
FAN-01 cell reaches 3 000, nor the 1 600 of ADR 0171's clean M1w15.

### Reading (by the registered rules)

1. **The Barth-Jespersen stall is not the ADR 0171 mechanism** (K2, K3). The limit-cycle cells at the blade-root leading edge
   and the leading Jacobian mode's hub cells are among the most benign cells of B3 (about the 20th to 30th percentile, aspect
   below 9). K2 and K3 alone say that a mesh change aimed at high-metric cells is not expected to remove the stall. The
   registered rule that names the next job assumed K1 STANDS, so no rule here decides the next job (deviation 1).
2. **The unlimited divergence: K1 is falsified as registered, with a split result.** The location agrees exactly: unlimited
   `linear_upwind` fails first in the largest-m cell of E3, and SV302's own search names that cell (D2). The level does not:
   750, against 6 900 to 21 000 where the aerofoil diverged and 1 600 where it did not. So ADR 0171's mechanism is not claimed
   for FAN-01. What this does show is that a metric level taken from the aerofoil does not carry over to a swirling, strongly
   off-axis flow, consistent with ADR 0171's own caveat that the metric alone does not predict divergence. Reported without a
   verdict: an extrapolation over the long side of the worst cell is a candidate on both meshes; testing it needs a run
   (Barth-Jespersen limited only at the top-m cells), which was not registered here.
3. **SV302 is a false negative on FAN-01** (D1). Its level of 3 000 is a judgement from three aerofoil meshes (ADR 0172 D3).
   No change is made here (warnings are another job's subject); see question 1.
4. **Part L**: K4 is DOES NOT TRACK by its rule, but the verdict is not robust. N is 0 on all three meshes, so it rests
   on M alone, which falls 6 % from E3 to E2 (750 to 708) while abs(L) rises sixfold, and then doubles from E2 to E1 (to
   1 497). Part L stays open, and nothing is claimed either way. The prediction's reason (the aspect ratio falls with
   in-plane refinement) holds for the median, not for the worst cell.

### Deviations

1. Reading 1: the reading rules did not cover the combination K1 FALSIFIED with K2 and K3 FALSIFIED for the choice of
   the next job. The first draft of this addendum applied the K1-STANDS rule anyway; that draft was committed in 3c2dac5
   and corrected in the next commit, before the job was reported.
2. The F1-family tests (`v1_cell_counts_f2_f1`, `k4_*`, `report`) were ignored without measuring their debug time, as
   the deliverables required. The gate runs in release (`docs/test-process.md`), where the whole binary, the ignored tests
   included, took 23 s.

Nothing here is a statement about torque, Delta p_ts or agreement with the measured fan. No default, limit or known miss
of another ADR changes.

### Questions for the maintainer

1. **SV302 level.** Lower it, or make it depend on the flow angle at the cell? Recommendation: neither yet. One more data
   point (FAN-01 at 750) does not make a rule. Record it in `docs/case-schema.md` as a known false negative and revisit
   when a third geometry diverges.
2. **Next stall job.** Recommendation: scheme side, with the locus still open (K2 and K3 say a mesh change aimed at
   high-metric cells is not expected to help). The record points two ways. ADR 0110's nonlinear runs grow at the five
   blade-root leading-edge cells. ADR 0111's linearisation puts the leading mode at the hub mid-chord cells (c1234, c365,
   c401), and ADR 0110 S12 saw the oscillation move to hub seam cells at z 0.16-0.17 once the leading-edge cells were
   calmed. ADR 0111 part J already held the convection gradient of c17 and of the five root cells fixed (H17, H5) and swapped
   c17's gradient formula (GG17): each changed rho by less than 1e-5, though that linearisation was not testable (M3 failed).
   A next job should first locate the drive in the nonlinear iteration, at both sites, before choosing a scheme change. It
   needs its own registration.
3. **The unlimited path.** Is a follow-up run worth it (limiter on the top-m cells only, to test reading 2's candidate)?
   Recommendation: no. The product path uses Barth-Jespersen, and its stall is reading 1's problem, not this one.
