# ADR 0181: Job 49R, a mesh change at the FAN-01 z 0.17 hub site (the hub trailing edge), second-order convection kept

Status: **registered 2026-10-31**, after the step-1 diagnosis (report only, committed in 776d2e0) and before any generator
change and before any arm mesh was built or run. The arms, the validity checks, the acceptance limits, the refinement
check, the sanity band, the choice rule, the budgets, the predictions, the reading rules and the stop rule are fixed here.
Amendments follow this text, each written before what it covers; results go in an addendum. Not yet accepted by the
maintainer.

Job 49R, on branch `job49r-fan01-hub-mesh` from `job49q-fan01-second-order` at 294e618 (worktree
`.claude/worktrees/job49r`). The job id was given by the coordinator; 0181 was free when this was written
(`git log --all -- docs/adr/0181*` empty).

House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
- limits are fixed here before the registered runs; a missed limit or a falsified prediction becomes an ignored
  `*_known_miss` test with the limit intact; a defective set-up gets an amendment before the corrected run;
- every generator change is behind an opt-in value; the default meshes and the default results stay bit-identical;
  `baseline_hashes.rs` is not re-recorded;
- no new crate, no unsafe code, no new third-party crate, no commit trailers, nothing merged or pushed.

Clean-room: first principles, our own ADRs, and grid-generation text recalled from the literature (UNVERIFIED as ADR 0110
marks its references; nothing depends on the citations).

## The request

The maintainer took ADR 0180's question 2: diagnose the z 0.17 hub site on B3 and B2 against the C-grid construction,
then test whether a mesh change there removes the limit cycle while the convection scheme stays second order everywhere
(the baseline class: `linear_upwind` with Barth-Jespersen, or a TVD scheme; second order in smooth flow, limited at
extrema, no cell forced to first order). The pressure-rise gap is out of scope.

## What I knew when writing this (rule d)

- ADRs 0110, 0111, 0174, 0175, 0177-0180 and `docs/milestone-fan.md`; the generator `crates/rcfd-mesh/src/fan_cgrid.rs`
  (`camber_seam_h` = `CGridOptions::camber_seam_h_construction()`), `crates/rcfd-api/src/fan_case.rs`, and the harnesses
  `fan01_stall_w43c.rs` and `fan01_second_order_w49q.rs`.
- **Runs already made in this job** (step 1, report only, records `D-B3.json`, `D-B2.json` in
  `docs/investigations/job49r-fan01-hub-mesh/`): the default case on the unchanged B3 and B2 meshes, 1000 iterations each,
  twice (the second time with the cell velocity added to the record; identical numbers). They reproduce ADR 0180 V1 and
  V2 to the printed digits: B3 4 920 cells, medians 1.089e-3 / 4.87e-4, torque 1.9177 N m at 1000, cycle means
  (iterations 501-1000) 1.9176 N m and 46.704 Pa, `|du|_2` 5.742 m/s at 1000; B2 13 627 cells, medians 1.004e-3 /
  6.23e-4, means 2.0308 N m and 63.860 Pa, `|du|_2` 8.248 m/s. No changed mesh has been built or run.
- The default B3 and B2 mesh hashes on the unchanged generator: `0xbe64f588a252d0b4` and `0xcae52c4997c9463f`
  (test `w49r_default_meshes_are_bit_identical`).

## Step 1: diagnosis (report only)

Harness: `crates/rcfd-api/tests/fan01_hub_mesh_w49r.rs` (`diagnose_b3`, `diagnose_b2`). The generator mesh is rebuilt
with `build_cgrid_mesh_with` and asserted equal (content hash) to the simulation's mesh, so every cell has its generator
tag (block, i, j, k). Definitions (per cell): non-orthogonality and skewness, the largest over its faces, and aspect, from
`rcfd_mesh::quality::compute_metrics`; volume ratio, the largest over its internal faces; area jump, the largest area
ratio of a pair of opposite faces; growth, the largest ratio of the centre distances across a pair of opposite faces;
`lambda_max`, the largest linear-interpolation weight `max(w, 1 - w)` over its internal faces; Peclet, the largest face
`|F_f| / ((nu + nu_t)_f |S|^2 / (S . d))` with the convecting flux at iteration 1000. **The steady SIMPLEC iteration has
no time step, so no Courant number is defined**; the residence time `V / sum(outflow)` over the median cell's is reported
in its place.

### Where the site is

1. **The z 0.17 hub site is the hub trailing edge, not mid-chord.** The hub section runs from z 0.1000 (leading edge) to
   z 0.17160 (trailing edge) on both meshes. The label "hub mid-chord" used by ADRs 0111 and 0175-0180 should be read as
   "hub trailing edge".
2. **B2**: the six site cells of ADR 0180 (c7, c57, c107, c157, c621, c1235) are all on C-block line `i = n_down - 1 = 7`,
   the first wake column downstream of the trailing edge (suction-side branch of the wake cut), rows `j` 0-3, layers
   `k` 0-2 (r 0.1243-0.1256). The 66 cells of the two first wake columns at `k <= 2` (0.48 % of the mesh) carry **37.0 %**
   of `sum |du|^2` at iteration 1000; the last chord columns at `k <= 2` 3.4 %, the leading-edge columns at `k <= 2` 0.1 %.
3. **B3**: the 19 hub seeds of ADR 0178 (the F-R(0.25) hub cycle) are on lines `i` 5-7 (5 = the first wake column, 6-7
   the last two chord cells on the suction side) plus three cells at `i` 9, 10 and 26, rows `j` 1-7, layers `k` 0-3. The
   B3 default stall is at the leading-edge root (c17, c341, c665: `i = h - 1 = 17`, `j = 0`, `k` 0-2): the
   leading-edge columns at `k <= 2` carry 30.8 % of `sum |du|^2`, the first wake columns at `k <= 2` 11.9 %.
   The hub site therefore is one location in generator indices on both meshes (first wake column and last chord cells
   at the hub layers); on B3 it is the second driver, on B2 the first.

### Cell shape: the site is not a geometric outlier

Means over the site cells and over the same tags moved to mid-span (`k = n_span / 2`) or to mid-chord and the
leading-edge column at the same `(j, k)`:

| B2 (6 cells) | non-orth [deg] | skew | aspect | vol. ratio | area jump | growth | `lambda_max` | Peclet | residence |
|---|---|---|---|---|---|---|---|---|---|
| site | 42.7 | 0.12 | 3.06 | 3.19 | 1.03 | 2.30 | 0.76 | 100 | 2.57 |
| same (i, j) at mid-span | 36.4 | 0.15 | 28.4 | 3.67 | 1.19 | 2.87 | 0.80 | 276 | 0.64 |
| same (j, k) at mid-chord | 24.6 | 0.07 | 15.5 | 1.88 | 1.02 | 1.49 | 0.62 | 512 | 2.11 |
| same (j, k) at LE column | 35.5 | 0.45 | 6.67 | 1.98 | 5.15 | 4.17 | 0.71 | 428 | 0.51 |

| B3 (19 seeds) | non-orth | skew | aspect | vol. ratio | area jump | growth | `lambda_max` | Peclet | residence |
|---|---|---|---|---|---|---|---|---|---|
| site | 48.5 | 0.09 | 7.56 | 13.9 | 1.06 | 5.50 | 0.84 | 172 | 2.96 |
| same (i, j) at mid-span | 60.4 | 0.15 | 19.1 | 2.85 | 1.48 | 2.41 | 0.70 | 382 | 1.69 |
| same (j, k) at mid-chord | 47.0 | 0.10 | 18.2 | 14.0 | 1.06 | 4.83 | 0.74 | 603 | 3.10 |
| same (j, k) at LE column | 37.7 | 0.39 | 11.0 | 11.9 | 3.30 | 6.29 | 0.88 | 872 | 1.22 |

- On B2 every site cell sits between the 6th and 90th percentile of every geometric metric; aspect (2.7-3.4, 6-9th
  percentile) makes them among the most isotropic cells of the mesh. This agrees with ADR 0174 (the wall metric does not
  flag them).
- On B3 the `k = 2` site cells have growth 21-22, volume ratio 34-41 and `lambda_max` 0.97 (93-98th percentile), but so
  does every `k = 2` cell at the same `(i, j)` at any chord position: it is the radial jump G1 below, not a property of
  the site (the mid-chord contrast has the same volume ratio, 14.0 against 13.9).
- **What does single the site out is the flow**: Peclet 20-214 on B2 (0-17th percentile; median 608) and a residence
  time 1.4-4.6 times the median; relative speed 0.5-7 m/s against about 20 m/s in the passage, with reversed axial
  velocity in c107 (B2) and c150, c114, c402 (B3). The site is a near-stagnant corner pocket behind the hub trailing
  edge, and the oscillation amplitude there at iteration 1000 (0.9-2.8 m/s on B2) is of the order of the local speed.

### What the generator does there

Nothing that it does only there. Two construction features meet at the site; each also acts elsewhere:

- **G1, the radial stack-to-middle jump** (`graded_two_sided`). The two exact stack cells are `h1` and `1.2 h1`
  (`h1` 0.565 mm); the first middle cell is `h1 g^n_bl` = 0.814 mm and the next one jumps to the capped middle size:
  - B3: 0.814 mm -> 29.4 mm, a **ratio of 36.2**, at r 0.1260;
  - B2: 0.814 -> 3.49 -> 14.9 mm, **ratio 4.28 twice**, at r 0.1260 and 0.1295.
  The site layers `k` 0-2 lie directly below that face. The same jump is at the tip end of the span and at every `i`.
  The family is not consistent here: the ratio changes 8.5 times from B3 to B2. On B3 it gives the `k = 2` faces a
  linear-interpolation weight of 0.97 (ADR 0180 reading 1: `kappa <= lambda` is anti-dissipative where `lambda > 1/2`).
- **G2, the wake axial growth at the trailing edge** (`capped_cells(len_w, n_down, dz_te)`). The wake starts at the last
  chord spacing `dz_te` (B3 2.72 mm, B2 1.62 mm at the hub; cosine clustering at the trailing edge) and grows by a solved
  ratio up to 1.5 times the mean wake cell. At the hub the first two wake steps are **3.85 and 3.69 on B3, 2.83 and 2.67
  on B2**; on the chord side of the trailing edge the step is 2.0 (B3) and 1.9 (B2). The site cells are the first wake
  column, so their downstream neighbour is 2.8-3.9 times longer. Every layer has this (mid-span B2: 3.5 and 2.9).
- Smaller or absent:
  - G3, the trailing-edge line normal (a central difference across the closed trailing edge) turns the in-plane line
    direction by +6 to +7 deg at the trailing-edge line and +3 deg at the first wake line; elsewhere on the chord at most
    2.6 deg. Small, not an arm.
  - Not found: the wake stacks are complete on every wake line near the trailing edge (`stack_cells` = `n_bl`), the
    first heights are `h1` times 1.00-1.03 (lean fade), there are no wedge cells; the seam turns by at most 2.8 deg near
    the trailing edge (5-15 deg three to four lines downstream, where the wake curve bends axial); the construction step
    is 0 (full smoothing) on both meshes, so no fallback setting acts.
- A local Jacobian (optional in the brief) was **not built**. The cycle is close to period two, so `|du|` at iteration
  1000 is used as the picture of the mode (the shares above).

## Arms (fixed now)

Two features, G1 and G2, give the arms. Each is a construction rule, applied by the generator the same way on B3 and on
B2 (B3 calibration, B2 held out). Neither has a tuned parameter: each replaces a capped growth by **one geometric ratio
solved to fill the length with the given count**, which for a fixed count and first cell is the distribution with the
smallest largest neighbour ratio (a constant ratio; any other distribution of the same count and end cells has a larger
step somewhere). Cell counts, the first cell `h1` and every other rule of `camber_seam_h` stay as they are.

| Id | Opt-in value (`mesh.fan.c_grid_grading`) | Change | Feature |
|---|---|---|---|
| **M-R** | `"radial_geometric"` | span layers `h1 rho^min(k, n_span - 1 - k)`, `rho` solved (no stack, no cap); tip gap unchanged | G1 |
| **M-W** | `"wake_geometric"` | wake axial cells `dz_te rho^m`, `rho` solved per layer (no cap) | G2 |
| **M-RW** | `"radial_wake_geometric"` | both | G1 and G2 |

Expected values (computed from the formulas, not from a mesh): M-R `rho` 2.914 (B3) and 1.810 (B2), so the largest
radial step falls from 36.2 to 2.91 and from 4.28 to 1.81; the mid-span layers grow from 30.3 to 40.8 mm (B3) and from
16.5 to 36.0 mm (B2), and the hub layers 1-2 from 0.68 / 0.81 mm to 1.65 / 4.80 mm (B3) and 1.02 / 1.85 mm (B2). M-W at
the hub: `rho` 2.143 (B3) and 1.810 (B2), so the first wake step falls from 3.85 to 2.14 and from 2.83 to 1.81; the last
wake cell grows to about 0.12 m (B3) and 0.10 m (B2).

The default (key absent or `"capped"`) is the unchanged construction. The key is read only by `axial_fan_sector` with
`topology = "c_grid"`; no other default changes.

## Protocol (fixed now)

The mesh cannot be changed during a run, so **every arm runs from iteration 1**: the default case (`case_toml(F3)` or
`case_toml(F2)` of the harness: SST, SIMPLEC, `linear_upwind` + Barth-Jespersen, rotating frame, the ADR 0086 set-up)
with only the `c_grid_grading` line added, **3000 iterations** (the 1000 + 2000 of ADR 0180). No relaxation, mask,
selector, frozen limiter or scheme switch.

Normalisation (stated because the switch protocol of ADRs 0175-0180 does not apply): the reference of mesh m is the
**unchanged mesh's default run** of step 1:
- `rho_du,m = |du|_2 at iteration 3000 of the arm / |du|_2 at iteration 1000 of the unchanged-mesh run` (5.742 m/s B3,
  8.248 m/s B2); cell counts are equal, so the norms compare like with like;
- `rho_med,m` = the arm's medians of `res_u` and `res_p` over iterations 2501-3000 against the reference medians
  (report only);
- `T_ref,m`, `P_ref,m`: the unchanged-mesh cycle means over iterations 501-1000 (B3 1.9176 N m, 46.704 Pa; B2 2.0308
  N m, 63.860 Pa).

## Validity checks (a failure is a defective set-up: amendment before any run is read)

- **V1** the unchanged-mesh references reproduce step 1 (above) to the printed digits; B3 and B2 are rebuilt each run.
- **V2** `w49r_default_meshes_are_bit_identical` (the two hashes above), the existing non-ignored tests of
  `rcfd-mesh` (`cgrid_w35d`, `fan_design_w46a` and the unit tests), `fan01_stall_w43c.rs`' non-ignored tests,
  `w49l_empty_mask_is_bit_identical`, `w49n_field_none_is_bit_identical` and `baseline_hashes.rs` pass unchanged.
- **V3 (per arm and mesh, before its run counts)** the arm mesh builds with the same cell count (4 920 / 13 627),
  `Simulation::new` accepts it, and the targeted feature has changed at the site:
  - M-R, M-RW: the first span layer is `h1` (relative difference below 1e-9) and the largest ratio of neighbouring span
    layer thicknesses is at most **3.0 on B3 and 1.9 on B2**;
  - M-W, M-RW: at layer `k = 0` the largest ratio of successive wake axial cells is at most **2.25 on B3 and 1.9 on B2**.
  The construction step and the mesh-wide largest non-orthogonality and skewness are reported, not limited.

## Acceptance limits (per arm; the criteria of ADRs 0178-0180)

| Id | Limit |
|---|---|
| **C1** | B3 and B2: `res_u` and `res_p` both below 1e-5 at some iteration in 1001-3000 |
| **C2** | B3 and B2: `is_converged()` true at iteration 3000 |
| **C3** | B3 and B2: `rho_du <= 0.01` |
| **RN** (refinement, not grow) | with `D_f,m = abs(f_end,m - f_ref,m) / abs(f_ref,m)`: `D_T,B2 <= max(D_T,B3, 0.002)` **and** `D_P,B2 <= max(D_P,B3, 0.005)` |
| **SB** (sanity band) | on B3 and on B2: `D_T,m <= 0.02` and `D_P,m <= 0.08` |

- `D` is the effect of the mesh change on the functional against the unchanged mesh's cycle mean. A mesh change that is a
  consistent modification must move the answer less on the finer mesh (RN, the floors of ADR 0180).
- SB: a mesh fix should not move the answer much. The bands are about a third (torque, 6.3 %) and a fifth (Delta p_ts,
  37 %) of the B3-to-B2 change of ADR 0086, and above the largest B2 effect of a second-order scheme change in ADR 0180
  (1.4 % torque, 6.0 % Delta p_ts), which is the scale of a change of discretisation that the cycle-mean comparison
  cannot resolve. Delta p_ts gets the wider band because it moves several times more than torque in every earlier job.
- **An arm is accepted** iff C1, C2, C3 hold on B3 and on B2, RN holds, and SB holds on both meshes.

**Choice rule**: if more than one arm is accepted, recommend the one with the smallest `max(D_T,B2 / 0.02, D_P,B2 /
0.08)` (the smallest move of the answer on the finer mesh); a tie goes to the arm that changes fewer cells' geometry, in
the order M-W, M-R, M-RW.

Reported without a limit, per run: `reached`, `rho_med`, the end residuals, `res_nut` at the end, torque and Delta p_ts
at the end and their `D`, `rho_du` against the run's own `|du|_2` at iteration 1000, the five largest-`|du|` cells at
the end with their tags, r and z, the share of `sum |du|^2` in the hub trailing-edge columns, the gap to the measured
values (2.142 N m, 126.5 Pa, ADR 0075; not a limit), the ratio `D_B2 / D_B3`, and the wall-clock seconds.

## Predictions (mine)

| Id | Prediction |
|---|---|
| **P1** | M-R misses C3 on B3: the B3 default stall is at the leading-edge root, which G1 does not change |
| **P2** | M-R on B2: `rho_du` below 0.5 (the hub trailing-edge cycle weakens), but C1-C3 are missed |
| **P3** | M-W misses C1-C3 on B3 and on B2, with `rho_du` above 0.5 on B2 |
| **P4** | M-RW misses C1-C3 on B2 |
| **P5** | no arm is accepted |
| **PS** | M-R moves the B2 torque by more than M-W does (it changes every span layer) |

Reasoning: the diagnosis finds no geometric outlier at the site; what singles it out is a near-stagnant corner pocket, so
I expect the cycle to be a property of the discrete flow there more than of a mesh defect. G1 is the strongest
candidate because it is the one feature that differs strongly between B3 and B2. My confidence in P1-P5 is moderate.

## Reading rules (fixed now; made only if V1-V3 hold)

1. **At least one arm accepted**: a mesh change at the hub trailing-edge site removes the limit cycle with the baseline
   second-order scheme on B3 and B2, and moves the functionals within the band, less on B2. Recommend the arm by the
   choice rule. Next job (not this one): B1, the empty passages, the shipped cases, and whether the key should become a
   default (a maintainer decision; it changes every FAN-01 result).
2. **C1-C3 on B3 only**: the change does not transfer to the finer mesh; not accepted; report where the B2 cycle sits.
3. **C1-C3 on both meshes, RN or SB missed**: the mesh change converges the passage but moves the answer too much or
   more on the finer mesh; not accepted; the size is reported.
4. **Residual-converged, C2 missed**: as ADR 0180 rule 3; not accepted by the limits.
5. **Mechanism** (no verdict): if an arm moves the largest-`|du|` cells away from the hub trailing-edge columns (share
   below a third of the default's), the targeted feature takes part in the cycle; if the cycle stays there, it does not
   depend on that feature at the tested size.
6. **All arms miss** (none accepted): **stop and report**. No further arm, combination, parameter, scheme or relaxation
   is run in this job.

## Time budgets and run order

- B3 run: 10 min wall clock (expected about 2 min); B2 run: 30 min (expected about 5 min). A run that hits its budget
  stops, writes its record with `budget_hit = true` and counts as NOT EVALUABLE (every convergence limit of that run
  missed); it is not rerun without an amendment. A run stopped by the steady divergence guard or a non-finite residual
  misses C1-C3.
- Order: M-R-B3, M-W-B3, M-RW-B3 (at most two at a time); commit the three B3 records; then M-R-B2, M-W-B2, M-RW-B2,
  every arm on B2 whatever its B3 result.
- Records: `W49R_OUT` is an absolute path (the harness refuses a relative one; ADR 0180 deviation 1).

## Not run (and why)

- A local Jacobian of the map at the site: optional, not needed for the arms; not done.
- G3 (the trailing-edge normal): a 6-7 deg turn, not singular. Uniform wake stacks (ADR 0072 W2): the wake stacks are
  already complete at the site.
- More cells (local refinement by count): it changes B3 and B2 into other members of the family and confounds the
  refinement check; not an arm here.
- TVD schemes on the changed meshes: the baseline scheme is the question; ADR 0180 already ran the TVD schemes.

## Deliverables

- `crates/rcfd-mesh/src/fan_cgrid.rs`: two opt-in fields of `CGridOptions` (default false).
- `crates/rcfd-api/src/fan_case.rs`: the opt-in case value `mesh.fan.c_grid_grading`; one line in
  `docs/case-schema.md`.
- `crates/rcfd-api/tests/fan01_hub_mesh_w49r.rs`: the diagnosis, V2-V3, the arms (ignored, one per process), RN and SB.
- `docs/investigations/job49r-fan01-hub-mesh/`: the records, `results.csv` and logs.
- This ADR's addendum; at most one sentence in `docs/milestone-fan.md`.

## Amendment 1 (2026-10-31, before any arm run): V3 fails for the wake rule; M-W and M-RW withdrawn; G2 numbers corrected

Written after the generator change (opt-in, defaults bit-identical: `w49r_default_meshes_are_bit_identical` passes with
the key absent and with `c_grid_grading = "capped"`) and after V3 was evaluated on the six arm meshes
(`docs/investigations/job49r-fan01-hub-mesh/V3.json`). **No arm has run.** Limits, M-R and the protocol are unchanged.

1. **V3 failed for the registered wake rule.**
   - M-W-B3 and M-RW-B3 do not build: "a directed edge appears twice ... at C block (i, j) = (27, 3), layer k = 7" (a
     tangled cell family at mid-span).
   - M-W-B2 does not build: "the line family is folded at layer 8 (r = 0.19400), C block around line 25, row 4".
   - M-RW-B2 builds (largest hub wake step 1.85, radial 1.81), M-R-B3 and M-R-B2 build and meet V3 (radial 2.914 and
     1.810, first layer `h1` to 7e-15, cell counts kept, construction step 0).

   Cause (read from the code, not run): with `n_down` fixed, a smaller first wake step forces outlet cells of about 0.1 m
   (B2: 1.4 mm growing by 1.85 to 106 mm). The C-block seam stations follow the wake-edge positions (`z_avg`, `z_out`
   in `build_layer`), so the passage lines of the mid-span layers move with them and fold. A wake rule that keeps the
   junction step near 2 within the given counts therefore does not build on this family.

   **Decision (fixed now)**: M-W and M-RW are **withdrawn as defective set-ups and not run**. An arm must be applied to
   both B3 and B2, so M-RW goes with M-W although it builds on B2. No replacement arm is registered: any other wake rule
   would be chosen by trying it until it builds, and the brief allows no extra arms. **M-R runs as registered.** The
   choice rule then has one candidate. The registered tests `m_w_*` and `m_rw_*` stay in the harness, ignored, and say
   that they are withdrawn. G2 stays untested (a question for the maintainer in the addendum).
2. **G2 numbers corrected.** The step-1 record lists wall-curve segment lengths along `j = 0`, and the diagnosis text
   above quoted their ratios as axial wake steps. The axial wake cells at the hub (from the V3 records, the unchanged
   wake of M-R) are 2.41, 10.34, 44.4, 57.1, ... mm on B3 and 1.44, 4.30, 12.9, 38.5, 42.8, ... mm on B2: the axial
   steps from the trailing edge are **4.29 (B3) and 2.99 (B2)**, not 3.85 / 2.83; `dz_te` is 2.41 / 1.44 mm, not 2.72 /
   1.62 mm (those are along the curve). The chord-side steps of 2.0 / 1.9 are surface-length ratios. The reading of G2
   does not change (the first wake step is 3-4 times, the chord-side step about 2).
3. V3 is recorded for all six meshes; the test asserts it for M-R only.
