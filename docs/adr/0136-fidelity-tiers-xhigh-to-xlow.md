# ADR 0136: Fidelity tiers xhigh to xlow: one `fidelity` setting expanded into explicit knobs, error budgets against an xhigh reference, target speedups

Status: **PROPOSED 2026-10-28** (scoping only; nothing implemented or run). The maintainer decided F-13 on 2026-10-28: the tier is
reachable through the MCP project tools (D9). F-1 to F-12 remain proposals. Written at main a89ccb3, revised at main 83343d9 with D9
and the validation plan. House rules as in [ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed
before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up is
corrected by an amendment registered before the corrected run; defaults of existing code stay bit-identical;
`crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded.

Parents: [`docs/fan-project-breakdown.md`](../fan-project-breakdown.md) parts C7, H3, H4 and H7;
ADRs [0119](0119-wave46b-reduced-order-axial-fan-tier.md), [0121](0121-wave46d-design-ledger-and-tolerance-study-driver.md), 0130,
[0038](0038-wave22-gpu-cuda.md) and [0058](0058-wave29d-distributed-parallel.md).

Every accuracy and speedup number in this ADR is a TARGET or an ESTIMATE, stated with the way it would be measured. Numbers marked
MEASURED are earlier results of this repository, cited where they were recorded; none is a result of this ADR. **Every number for the
computer-fan class is an estimate, because no fan reference at any fidelity exists in the repository.**

Naming:
* The decision ids F-1 to F-13 belong to this ADR. They are not roadmap items (F-001 to F-269).
* "Part F1" is the fan breakdown's tolerance study; "check F1" of ADRs 0038 and 0135 are those ADRs' checks.
* Proposed roadmap items get ids from the maintainer, as in ADR 0130.
* The D sections describe the design under the recommended option of each open decision, named in brackets. Another choice changes
  the section named.

Maintainer's framing: build the foundation for an extra high precision simulation, and lower tiers that deliver a stated percentage
accuracy for a multiplier speedup.

## Context (read at main a89ccb3)

### Knobs that exist

| Knob | What exists | Where | What limits a tier |
|---|---|---|---|
| Working precision | f64 only: `Real = f64`; the `Scalar` trait (f32, f64) has no user outside `rcfd-core`; no `f32` in the sources of `rcfd-fv`, `rcfd-linalg`, `rcfd-gpu`, `rcfd-turb`, `rcfd-mesh`, `rcfd-dist`, `rcfd-reactor` | `crates/rcfd-core/src/scalar.rs` lines 7-11 | `rcfd run --precision` accepts `double` only; anything else is exit 2, "only double precision is supported" (`crates/rcfd-cli/src/main.rs` lines 364-370) |
| Storage precision | stored fields as `Precision::F64` (bit-exact) or `F32` (lossy, bound `abs(x) 2^-24 + 2^-150`) | `crates/rcfd-tsstore/src/lib.rs` lines 72-80, 110-113 | output only |
| Matrices | CSR `row_ptr` and `col` as `Vec<usize>`, `val` as `Vec<f64>`; LDU addressing `Vec<usize>`, coefficients `Vec<f64>` | `crates/rcfd-linalg/src/matrix.rs` lines 11-35, 146-159 | 16 bytes per stored nonzero (value and column) |
| Linear solvers | CG, BiCGStab, GMRES; Jacobi, IC0, ILU0, Chebyshev, AMG; reductions in fixed 4096-value chunks | `crates/rcfd-linalg/src/solvers.rs`, `precond.rs`, `amg.rs`, `vecops.rs` lines 6-17 | `SolveControl` default `tol_rel` 1e-8 (`solvers.rs` lines 36-53); SIMPLE inner solves `tol_rel` 1e-3 (`crates/rcfd-fv/src/simple.rs` lines 210-219); case keys `solver.momentum_rel_tol`, `pressure_rel_tol`, `pressure_abs_tol`, `*_max_iter`, 0 = algorithm default (`crates/rcfd-api/src/schema.rs` lines 1119-1150) |
| Outer convergence | steady stop when `res_u < solver.tolerance_u` and `res_p < tolerance_p` (1e-6 each) or at `max_iterations` (2000); `convergence.<name>` monitors (`window_range`, `change`, `below`, `above`, optionally relative) can stop earlier; steady divergence guard on by default | `schema.rs` lines 884-896, 918, 1001-1009, 1266-1291; `crates/rcfd-api/src/sim.rs` lines 2857-2870 | the bladed FAN-01 study runs ended at the 8000-iteration cap and one empty-passage run at the 60-minute guard ([ADR 0086](0086-wave37c-fan01-solution-verification.md) lines 167-174) |
| GPU | CUDA CG and BiCGStab with Jacobi or Chebyshev for the pressure solve of `simple`, `piso`, `pimple`; matrix assembled on the CPU and uploaded per solve; kernels in `double` with `int` indices; the host converts to `i32` after `check_index_range` refuses row or nonzero counts above `i32::MAX` | [`docs/gpu.md`](../gpu.md); `crates/rcfd-gpu/src/cuda/kernels.cu` line 8; `crates/rcfd-gpu/src/cuda/system.rs` lines 47, 83-84; `crates/rcfd-linalg/src/backend.rs` lines 160-174 | MEASURED: 0.22x against one CPU thread at 4096 unknowns; matrix upload 63-74 % of the one-shot time; 10x to 15x one-shot at 5M unknowns |
| Threads, distributed | `--threads n` gives byte-identical solution files (MEASURED, [`docs/run-control.md`](../run-control.md)); `rcfd-dist` has an in-process transport only, no MPI (F-100 todo) | ADR 0058 | MEASURED B9: parallel efficiency 0.56, 0.38, 0.20 at P = 2, 4, 8 on 36 864 cells (not a cluster) |
| Space and time schemes | `solver.convection` (default `linear_upwind`; upwind, central, quick, blended, high_resolution, compressive, bounded_central, limiters); `solver.time_scheme` euler (default), bdf2, bdf2_bounded; adaptive steps (`max_courant`, `error_tolerance`, ...; all unset is the fixed `dt` bit for bit) | `schema.rs` lines 8, 853-856, 938-940; [`docs/case-schema.md`](../case-schema.md) "Wave 13" and "Adaptive time stepping" | `turbulence.convection` defaults to upwind |
| Mesh resolution | generator counts `mesh.nx`, `ny`, `nz` (`schema.rs` lines 83-108); wall layers `mesh.wall_layers`, `target_y_plus` (lines 224-273); fan passage `mesh.fan.n_up` to `n_gap`, `n_bl`, `y_plus` default 30 (`crates/rcfd-api/src/fan_case.rs` lines 54-64); `--set` applies in order, replacing the value, before the mesh is built (`crates/rcfd-cli/src/cmd_w19c.rs` lines 44-58, `crates/rcfd-api/src/chain.rs` line 77) | | no key scales all counts; imported meshes cannot be refined or coarsened; adaptation is a library for box lattices with no case hook (`rcfd_mesh::adapt`, ADR 0054 D7) |
| Solution verification | GCI in test helpers only, in two forms: `crates/rcfd-api/tests/validation_common/mod.rs` line 44 (constant ratio, order clamped to [1, 2], Fs 1.25) and `crates/rcfd-api/tests/fan01_gci_w37c.rs` (unequal ratios, ADR 0086) | | not a library |
| Turbulence closures | laminar; `mixing_length`; `k_epsilon` (standard, rng, realizable); `k_omega_sst` (sst, bsl); `spalart_allmaras` with `sa.des` = des or ddes; `smagorinsky`, `wale`; `rsm`; plugin | [`docs/case-schema.md`](../case-schema.md) "`turbulence`"; `crates/rcfd-api/src/turb.rs` lines 349-399 | `k_epsilon`, `k_omega_sst`, `spalart_allmaras` steady only; `mixing_length` steady or transient; `rsm`, SA-DES/DDES and LES transient only; `rsm` without walls; every model refused with `energy.enabled` ("RANS turbulence together with the energy equation is not supported", lines 394-399); no model with `ideal_gas` or the compressible solver |
| Steady, transient, time means | `simple`, `simplec` (default), `coupled` (laminar), `piso` (option `solver.time_advance = fractional_step`), `pimple`, `compressible`; `statistics` (means, rms) for `piso` and `pimple` only (`crates/rcfd-api/src/stats.rs` lines 87-94) | `schema.rs` lines 848-851, 1348-1361 | |
| Rotating machinery | steady rotating frame on a periodic passage, MRF zones, cyclic seams (opt-in), mixing plane, frozen-rotor cylindrical interface | [`docs/milestone-fan.md`](../milestone-fan.md) | FAN-01 stall open (ADRs 0110-0117); no transient sliding interface (F-250 todo) |
| Reduced-order, lumped | strip-theory axial-fan model `crates/rcfd-post/src/fan_rom.rs` with `FanRomEvaluator` (Rust only, not reachable from a case file: `crates/rcfd-api/src/design/fan_rom_eval.rs` line 2); lumped fan curve, porous jump, network pressure (`crates/rcfd-fv/src/lumped.rs`, F-113); actuator disk (`crates/rcfd-fv/src/actuator.rs`, F-218 partial); potential-flow start (`crates/rcfd-fv/src/potential.rs`) | ADR 0119 | MEASURED against FAN-01 data, not against CFD: design point dp_s -64 %, power -7.4 %, 33 of 40 strips stalled (ADR 0119 V1) |
| Chemistry acceleration | `chemistry.acceleration` = off, cluster, isat, with `accel_tolerance`; DRG is library only | `crates/rcfd-api/src/chemistry.rs` lines 95-102; [ADR 0053](0053-wave28b-chemistry-acceleration-combustion.md) | MEASURED: DRG 9x and 1.2x; ISAT 0.23x (a slowdown, I4) to 118x (I5) |
| Run records and tools | `run.json` `settings` is free JSON (`crates/rcfd-project/src/records.rs` lines 82-83); runner `RunSettings {priority, threads, mem_mb, set}` without `deny_unknown_fields` (`crates/rcfd-runner/src/records.rs` lines 18-33); runner memory budget 4096 MB at an estimated 4 KB per cell, a run above the whole budget is refused (`crates/rcfd-runner/src/runner.rs` lines 30-58, 554-564); `RunSummary` has no settings (`crates/rcfd-project/src/project.rs` lines 82-97); MCP `start_run` takes `set` (`crates/rcfd-mcp/src/project.rs` lines 228-244); `compare_runs` gives differences to the first run | ADRs 0130, 0135 | the default MCP tool list, `design_ledger_*` with a three-value `fidelity` enum among it, is a golden never re-recorded (`crates/rcfd-mcp/tests/default_f1.rs`); the read-class project tools must equal the bridge's first allow-list (`crates/rcfd-claude/src/allow.rs` line 301, checked at `crates/rcfd-mcp/tests/project_w48f.rs` line 85) |
| Design ledger | `fidelity` in {analytic, reduced_order, cfd}, which ADR 0121 and the ledger's messages call a tier (`ledger.rs` line 518); `Fidelity::parse` rejects other values and the reader then skips the line; Pareto and best refuse sets that mix evaluators or fidelities; `rerun_check` refuses another evaluator id or version (line 812); `kind` is free text; unknown keys are ignored (fields read by name) | `crates/rcfd-api/src/design/ledger.rs` lines 71-99, 290-320, 502-522; ADR 0121 D1, D3, D4 | |

### What does not exist

* An f32 or mixed-precision solve, iterative refinement, an f32 GPU kernel, 32-bit indices in CPU matrices (the GPU path already
  converts to `i32`).
* A `fidelity` setting, preset, tier or profile in the case tree, CLI, runner, MCP tools or Studio, other than the ledger's
  three-valued `fidelity`.
* A transition model ([`docs/audit/partials-group-3.md`](../audit/partials-group-3.md): "No transition or intermittency code exists in
  any crate"), wall-modelled LES, IDDES, SAS, transient SST or SA (URANS), any turbulence model with the energy equation. Turbulent
  Nusselt numbers cannot be computed at any tier today.
* A transient rotor-stator solve, a full-annulus fan transient, MPI, device-resident assembly.
* Case-level mesh adaptation, a GCI library, a surrogate model (H3), a multi-fidelity correction (H7).
* A validated fan result at any fidelity: the FAN-01 rotor misses its measurement ([`docs/milestone-fan.md`](../milestone-fan.md)); no
  computer fan has measured data (part G2).

## Decision (proposed)

### D1 What a tier is

A tier is a named, versioned preset with three parts:
* **Expansion**: the ordered list of explicit settings it applies.
* **Error budget**: limits on named quantities against the xhigh tier of the same case.
* **Speedup band**: its expected speedup against xhigh.

The budget and the speedup band are claims for a case class. Each holds only after that class's calibration checks pass (D7, D8);
until then a run says "tier applied, claim not validated".

The knobs fall on two axes:
* **Numerical axis**: mesh counts of generator meshes, time step, linear and outer tolerances, precision mode. These are relative
  transforms that apply to any case written as a case file. Schemes stay as written (Table B1 notes).
* **Model axis**: closure, steady or transient, passage or full annulus, CFD or reduced-order. These are absolute presets per case
  class [F-1 (c)].

Two classes are defined now:
* **Class S (same model)**: laminar flows and RANS flows with a fixed closure. Tiers differ on the numerical axis only, and xhigh
  is reachable today.
* **Class F (computer fan)**: absolute presets on both axes [F-9]. They are delivered in T5, and xhigh is not reachable today.

### D2 The tier matrix

Table A, purpose:

| Tier | Who uses it, for what decision | Delivered as |
|---|---|---|
| xhigh | the maintainer, by explicit approval: the reference every lower tier is measured against; the last check of a design before it is built or published; broadband noise | a CFD run |
| high | the design loop's confirmation stage (H7, "CFD to confirm"): a shortlist of 3 to 10 variants; tones and stall margin once class F has transient runs | a CFD run |
| medium | everyday runs from the Studio and from Claude: operating points, fan curves (C5), comparisons | the case as written (class S) |
| low | screening sweeps and optimiser exploration (DOE, Nelder-Mead), the CFD points of a tolerance study (part F1) | a CFD run |
| xlow | optimiser inner loops, tolerance studies (ADR 0121 driver), instant curves in the Studio, the bound a CFD result must respect (C7) | an evaluator in the design loop or the MCP tool `evaluate_xlow` (strip theory, lumped network, correlation), no run folder; offered only where such a model exists |

Table B1, the numerical axis (class S), relative to the case as written (cell size `h`, time step `dt`):

| Knob | xhigh | high | medium | low | Available today |
|---|---|---|---|---|---|
| Precision | f64 | f64 | f64 | f64 in presets v1; mixed from v2, after T3 (D6) | f64 only |
| Cell size, generator counts | h/2 | h/sqrt 2 | h | sqrt 2 h | counts by `--set`; imported mesh: not applied (D3) |
| First cell | refined with h only where y+ <= 1 | as xhigh | as written | as written, never scaled | keys exist |
| Time step, transient | dt/2 | dt/sqrt 2 | dt | sqrt 2 dt | keys exist |
| Inner linear tolerance | `pressure_rel_tol` 1e-6 steady, 1e-10 transient | as xhigh | as written | 1e-2 steady | keys exist |
| Outer stop | `tolerance_u`, `tolerance_p` 1e-9 and a relative `change` monitor below 1e-7 on each report | as xhigh | as written | 1e-4 and a relative `change` monitor at 1e-3 | keys exist |
| Time means | until each mean's SE <= budget/24, from at least 40 batches (V-e) | as xhigh | as written | as written | `statistics`, `piso` and `pimple` only |

Notes on Table B1:
* **Schemes stay as written at every tier.** Space and time schemes are not tier knobs. Medium, high and xhigh therefore differ in h
  and dt only, and form a refinement family.
* **First-order cases.** A case written with a first-order scheme keeps it at every tier. Its xhigh then rarely meets the D8
  uncertainty rule, so its checks are reported as non-discriminating.
* **Scheme upgrades.** An upgrade, for example euler to bdf2, would be a separate step outside this ladder; none is proposed here.
* **Tolerances.** They change the iteration error only. VR4 checks xhigh and its partner; the lower tiers' iteration error is part of
  the error they are judged on.
* **First cell.** Where the first cell is not scaled, the near-wall cells leave the refinement family. That covers wall functions at
  every tier, and low on wall-resolved meshes. The wall-normal error is then outside every band, as in ADR 0086, and low's pair
  estimate is refused (`not_in_family`, V-f).

Table B2, class F presets (absolute, delivered in T5, definitions per F-9):

| Knob | xhigh | high | medium | low | xlow | Available today |
|---|---|---|---|---|---|---|
| Closure | wall-resolved LES (`wale`) | SA-DDES | steady SST | steady SST, wall-function first cell (`mesh.fan.y_plus` 30-100) | strip theory | LES transient, no wall-bounded validation; SA-DDES transient, not validated on separation; URANS missing |
| Time dependence | transient | transient | steady | steady | algebraic | sliding interface missing (F-250) |
| Domain | full annulus, struts, inlet | full annulus | one periodic passage | one passage | B blades, no mesh | annulus mesh partial (part B3) |
| Wall resolution | y+ <= 1 | y+ <= 1 | y+ <= 1 (the passage generator defaults to `y_plus` 30, `fan_case.rs` line 64) | medium's counts, wall-function first cell | | `mesh.fan.*` keys exist |
| Schemes | bounded second order in space, bdf2 in time | same | same in space (steady) | same in space (steady) | | exist (ADR 0086 used limited `linear_upwind`) |
| Precision, tolerances, time means | as B1 xhigh | as B1 high | as B1 medium | as B1 low | | |

Table C, error budgets: TARGETS against the xhigh run of the same case, in percent of the xhigh value unless a unit is given:

| Quantity | Class | high | medium | low | xlow | No claim below xhigh |
|---|---|---|---|---|---|---|
| Fan pressure rise at a set flow | F | 3 % | 8 % | 15 % | 30 % | a rising characteristic (stall region), and flows less than 0.1 of design flow above the stall-onset flow |
| Fan flow at the operating point against a system curve | F | 1.5 % | 4 % | 7.5 % | 15 % | as above, and system curves that cross in the stall region |
| Fan efficiency | F | 1.5 points | 3 points | 6 points | 10 points | as above |
| Shaft power, torque | F | 2 % | 5 % | 10 % | 40 % | as above |
| Stall-onset flow | F | 0.05 of design flow | reported only | no claim | no claim | |
| Tones at blade-passing harmonics | F | 3 dB | no claim | no claim | lower bound only | broadband noise at every tier below xhigh |
| Ranking of 8 or more variants | F, S | | | rank correlation >= 0.9, true best in the tier's top 3 | >= 0.7, true best in the top 5 | |
| Mean Nusselt number, laminar | S | 1 % | 3 % | 7 % | 15 % (correlation, canonical geometry only; no evaluator in T1-T6) | turbulent heat transfer (not computable today) |
| Drag coefficient, laminar bluff body | S | 1 % | 3 % | 7 % | no claim | |
| Strouhal number (absolute) | S | 0.002 | 0.005 | 0.01 | no claim | |
| Reattachment length (step heights) | S | 0.1 | 0.3 | 0.7 | no claim | |
| Velocity extremum of a laminar recirculation | S | 1 % | 3 % | 7 % | no claim | |
| Skin friction, attached boundary layer | S | 1 % | 3 % | 7 % | 15 % (correlation; no evaluator in T1-T6) | transition |
| Pressure drop or flow resistance, internal flow | S | 1 % | 3 % | 7 % | 15 % (lumped network; no evaluator in T1-T6) | |

Reasons for the numbers:
* **Class S, numerical axis only.** Take error `C h^p`, p in [1, 2], and sqrt 2 steps. The differences to xhigh of high, medium and
  low are then 0.33-0.41, 1 and 1.8-2.3 times the medium difference. (For p = 1: 0.207, 0.5, 0.914 of `C h`; for p = 2: 0.25, 0.75,
  1.75 of `C h^2`.) The budgets 1, 3 and 7 % follow the p = 2 ratios; at p = 1 they would be 1.24 % and 5.5 %. The ratios hold
  only for runs that differ in h and dt alone (Table B1 notes).
* **Class F, flow against pressure.**
  * Take a system curve `dp = k Q^2 + l Q` (the form of ADR 0119 D-M8) with slope `S`, crossed by a fan curve of slope
    `s <= 0`. Then `dQ/Q = (d dp/dp) / (S Q/dp - s Q/dp)`, with `S Q/dp` between 1 (`k = 0`) and 2 (`l = 0`).
  * The flow budgets are half the pressure budgets, which holds for a quadratic system.
  * With a linear term the check divides the pressure budget by the case's `S Q/dp`, up to the full pressure budget.
* **Class F, model-axis budgets.** They are set so that medium is useful for trends, and they cannot be checked until a fan xhigh
  exists (T5). The only fan evidence: coarsening the FAN-01 passage by two sqrt 2 steps moved Delta p_ts by -37 % and torque by -9 %.
  This is MEASURED in ADR 0086, B3 against B1, on runs ended by the cap rule. That is why the class F low preset keeps medium's counts
  (Table B2, Limits 5).
* **xlow.** The only evidence is ADR 0119 V1, which compares the strip model with measured data, not with xhigh, so it is evidence
  only.
  * At flows above 1.7 m^3/s, where no strip is stalled: within 2 to 14 Pa, and power 32-38 % high.
  * At the design point, with 33 of 40 strips stalled: -64 %.
* **Laminar Nusselt number and drag: which checks can discriminate.** These are ESTIMATES, converted from the earlier fine-mesh GCIs
  to V-a's band (Fs 3, the coarser member of the pair). That multiplies them by about 9.6 for the ratio-2 Nu families (`2.4 r^p`
  with r = 2 and p = 2: the quoted GCI is that of the partner's mesh). For the cylinder the factor is about 2.4: the quoted GCI is
  that of xhigh's own mesh, and the partner is one sqrt 2 step finer. VR1 measures the actual bands.
  * **Nu.** The quoted GCIs are 0.014-0.053 % (MEASURED: ADR 0061 lines 257-259; ADR 0062 line 118, which supersedes ADR 0061's
    0.13 % at Ra 1e6), giving bands of about 0.13-0.51 %. Nu is then discriminating at high (band at most 0.33 %) for Ra 1e3 to 1e5,
    not for Ra 1e6. At medium and low it is discriminating for every Ra.
  * **Cylinder.** The quoted GCIs are C_D 0.96 % and St 1.62 % (ADR 0062 lines 165-166), giving bands of about 2.3 % and 0.0063
    before the time part, which first-order time makes large (ADR 0062 lines 158-161). C_D is at most borderline at low, and St
    discriminates at no tier. A transient class S claim therefore needs a case written with `time_scheme = bdf2`, as the ADR 0069
    cylinder run was, or a finer reference.

Table D, speedup TARGETS against xhigh: products of Table E's factors. Fixed costs lower the upper ends (Limits 6).

| Tier | Class S, 2D steady | Class S, 3D steady | Class S, transient 2D / 3D | Class F |
|---|---|---|---|---|
| high | 2-3.6 | 2.8-5.1 | 2.8 / 4 | 6-50 |
| medium | 6-40 | 12-78 | 12-24 / 24-48 | 350-7000 |
| low (presets v1, f64) | 18-420 | 51-1200 | 51-204 / 144-576 | 800-63 000 |
| xlow | 1e4 or more where a lumped model or correlation exists | same | same | 1e6-1e9 |

Mixed precision (presets v2) multiplies the low row by 1.2-1.6.

Table E, the factors:

| Knob | Factor | Basis | Kind |
|---|---|---|---|
| One sqrt 2 step coarser, steady | cells / 2 (2D), / 2.83 (3D); iterations / 1.0-1.8 | heated cavity Ra 1e6: 336, 1057, 3592 iterations at n = 32, 64, 128 (ADR 0062 line 117); FAN-01 at a fixed 8000 iterations: 7.1, 25.6, 58.8 min for 4 920, 13 627, 38 184 cells (ADR 0086 lines 172-174), two runs at a time on a shared machine (lines 159-160) | MEASURED anchors |
| One sqrt 2 step coarser, transient | cells as steady, steps / 1.41 at a fixed Courant number | constant CFL | arithmetic |
| Tolerances | xhigh and high cost x1.5-3 over medium; low costs /1.5-3 under medium | Krylov iterations grow like log(1/tol) | ESTIMATE; no effect where runs end at a cap |
| Mixed precision (presets v2) | SpMV bytes per nonzero 16 -> 12 with `usize` columns (at most 1.33x), 16 -> 8 with u32 columns (at most 2x); GPU, already 32-bit indices: 12 -> 8 (at most 1.5x) and half the value upload; whole run 1.2-1.6x | bytes moved; assembly, gradients and reductions stay f64 (Amdahl) | arithmetic, ESTIMATE |
| Wall function instead of y+ about 1 (class F low) | cells / 1.5-3, faster convergence | fewer stiff near-wall cells | ESTIMATE |
| LES to steady RANS (class F) | cells / 10-50, transient with averaging to steady / 5-20 | wall-resolved LES needs wall-parallel spacing of tens of wall units (Chapman 1979; Choi and Moin 2012; recollection, UNVERIFIED) | ESTIMATE |
| Hybrid instead of wall-resolved LES (class F high) | cells / 3-10, steps / 2-5 | RANS treatment of attached boundary layers | ESTIMATE |
| Full annulus to one passage | / B (7 for set-up C of ADR 0119) | periodicity | arithmetic; needs steady periodic flow |
| CFD to strip theory | milliseconds against hours to days | evaluation time is not recorded in ADR 0119 | ESTIMATE |

How Table D follows from Table E:
* **Class S**, per step:
  * steady 2D 2-3.6, 3D 2.83-5.1;
  * transient 2D 2.83, 3D 4.

  High is one step. Medium is two steps times the tolerance factor 1.5-3. Low is three steps times both tolerance factors (2.25-9).
* **Class F**:
  * high: (3-10) x (2-5);
  * medium: 7 x (10-50) x (5-20);
  * low: medium x wall function (1.5-3) x tolerance (1.5-3).

### D3 How a tier is expressed

* **One setting, `fidelity`, carried by the run [F-2 (a)].**
  * It is set with `rcfd run --fidelity <tier>`, `RunSettings.fidelity` in the runner, the MCP project tools of D9, or a select box
    in the Studio's start dialog. That dialog is proposed in [ADR 0133](0133-wave48d-run-control-and-live-monitoring.md) and is not in
    `crates/rcfd-studio/src` at a89ccb3.
  * Case files are unchanged, and `tools_list_default.json` stays byte-identical (F-13 as decided).
  * `rcfd schema` is unchanged by T1 and T2; T3 adds the key `solver.precision` [F-6 (a)].
* **One expansion function, `rcfd_api::fidelity::expand(case, tier, presets)`.**
  * It returns an ordered list of `path=value` entries in the `--set` syntax, and the knobs it could not apply, each with a located
    reason.
  * Presets are data compiled into the binary, with a version (`rcfd-fidelity-1`). A changed preset is a new version, never an edit
    (T1-2).
  * A dry run (`rcfd fidelity <case> --tier <t>`, and the MCP tool `list_tiers` with `case` and `tier`) shows the expansion and the
    estimated cost multiplier before a start, and writes nothing.
* **Order and overrides.** The expansion comes first, then the user's `--set` entries. A later entry wins (`cmd_w19c.rs` lines
  44-58), so every knob stays overridable. Overriding a claimed knob voids the claim: every knob in the expansion is claimed, and so is
  `solver.max_iterations`, which can end a run before its outer stop. The record then reads `claim: "void: <path>"`.
* **Arithmetic.**
  * Mesh counts scale in integers, never below the generator's minimum. One step up is `(n * 1414 + 500) / 1000`, one step down
    `(n * 1000 + 707) / 1414`; both round half up. xhigh takes `2 n` exactly.
  * Time steps and tolerances use IEEE basic operations or decimal literals.
  * No `powi` (a compiler intrinsic whose rounding differs between platforms) and no libm function (`powf`, `exp`, ...) is reachable
    from `expand`. An expansion is therefore identical on every platform (D4).
* **A knob that cannot apply [F-4 (b) and (a)].**
  * A numerical knob on an imported mesh is recorded as not applied, and the run carries no claim.
  * A model knob the case cannot run is refused with a located error by the existing validation, for example any turbulence model
    with `energy.enabled` (`turb.rs` lines 394-399).
* **xlow is not a run.** The design loop, or the MCP tool `evaluate_xlow` (D9), sends the variant to the strip-theory or lumped
  evaluator. The ledger records it with `fidelity = reduced_order` and tier xlow.

### D4 Default and bit-identity

* **Without `--fidelity`, `expand` is not called**, under every option of F-1. The solver gets today's input, so these are unchanged:
  fields, face fluxes, `solution.vtu`, `monitors.json`, the status stream apart from `wall_s`, and every `baseline_hashes` value.
* **`--fidelity medium` is the identity** for every case without absolute presets. That is all cases under F-1 (a); under (c), every
  class not yet given absolute presets, so class S always and class F before T5. On those cases `--fidelity medium` gives the same
  files (T1-1).
* **What does change once a tier is given**: `run.json` gains `settings.fidelity`. That is the record, not the solution.
* **Bit-identity is per platform and build.**
  * `powi` rounds differently between platforms (`crates/rcfd-api/src/sim.rs`, the heated-cavity probe comment of commit 54e64a5).
  * The mesh generators call the platform libm, so generator hashes are recorded per platform
    (`crates/rcfd-mesh/src/non_conformal.rs`, commit e56e931).
  * The maintainer's gate runs the integration tests, `baseline_hashes` among them, on the Windows gate host.
    `.github/workflows/gate.yml` can run the full gate on `ubuntu-latest` on request. CI runs unit tests on Linux for every pull
    request and on Windows on main (`.github/workflows/ci.yml` lines 53-61).
  * Tier checks name their platform. Comparisons across platforms use tolerances, never bits.

### D5 Records: run.json, status, Studio, MCP, ledger

`run.json` `settings`, as the runner would write it for a run at low with one user override, for a class S case with
`mesh.nx = mesh.ny = 40` and one report `lid_drag`:

```json
"set": ["mesh.nx=28", "mesh.ny=28", "solver.pressure_rel_tol=0.01", "solver.tolerance_u=0.0001", "solver.tolerance_p=0.0001",
        "convergence.tier_lid_drag.monitor=lid_drag", "convergence.tier_lid_drag.type=change",
        "convergence.tier_lid_drag.relative=true", "convergence.tier_lid_drag.tolerance=0.001", "output.store_interval=50"],
"fidelity": {"tier": "low", "presets": "rcfd-fidelity-1", "class": "S", "expansion": ["(the first nine entries of set)"],
             "user_set": ["output.store_interval=50"], "not_applied": [], "claim": "not validated"}
```

* `set` holds the effective list, so a runner that does not know `fidelity` reproduces the run: it ignores the key, because
  `RunSettings` has no `deny_unknown_fields`.
* `status.json` is unchanged: a run's tier is fixed.
* `RunSummary` gains an optional `tier`, omitted when absent, so the Studio run list and `list_runs` show and filter it.
  `compare_runs` is unchanged; cross-tier comparison is the new `tier_report` (D9).
* **Ledger.**
  * A new optional field `tier`; old readers ignore it. ADR 0121 already calls the three-valued `fidelity` a tier, and it keeps its
    three values: a tier value there would make old readers skip the line.
  * Old readers do not know `tier`, so their Pareto and best queries could mix tiers. For one release the evaluator id also carries
    the tier (`cfd@low`), which old readers already keep apart [F-7 (c)].
  * A new entry kind `cross_tier_check` (kind is free text) records re-verification (D8).
  * Through MCP, the tier reaches the ledger by the project tools `record_design`, `verify_design` and `query_designs` (D9). The
    default `design_ledger_*` tools keep their schema, which is in the golden that is never re-recorded (F-13 as decided).

### D6 Mixed precision in the numerics

| Data or kernel | In `mixed` mode | Why |
|---|---|---|
| Field state, face fluxes, cell sums of fluxes | f64 | increments per step are small against the state; conservation sums cancel |
| Gradients, limiters, wall-law switches, upwind direction | f64 | a flipped limiter changes the discrete operator; limiter switching is the ADR 0110 limit cycle |
| Dot products, norms, residuals, monitors | f64, fixed chunk order | determinism for any thread count (`vecops.rs`) |
| Matrices of momentum, turbulence and scalar equations | f32 values, u32 indices [F-5 (b)] | diagonally dominant; already solved to `tol_rel` 1e-3 inside SIMPLE |
| Preconditioners (AMG levels, IC0, ILU0, Jacobi, Chebyshev) | f32 [F-5 (a)] | changes the iteration count, not the converged answer |
| Pressure system | f64 matrix and Krylov vectors; f32 correction solves inside f64 iterative refinement only after T3-4 [F-5 (c)] | refinement reaches f64 accuracy only while the condition number times the f32 unit roundoff (6e-8) stays below about 1, and pressure conditioning grows like h^-2 (Higham; Carson and Higham 2018; recollection, UNVERIFIED) |
| Checkpoints | f64 | bit-identical restart is a documented property ([`docs/run-control.md`](../run-control.md)) |
| Stored output fields | f32 allowed | exists (`rcfd-tsstore`) |
| GPU | f32 values (indices are already 32-bit), f64 dot accumulation and refinement on the device | consumer GPUs run f64 at a small fraction of f32 throughput (ratio of the RTX 5080 UNVERIFIED); the value upload halves |

Delivery:
* **Matrix types.** A new generic `CsrMatrixOf<S, I>` over the existing `Scalar` trait and an index type, with
  `type CsrMatrix = CsrMatrixOf<f64, usize>`. This keeps the existing name, constructors and call sites. Default type parameters
  alone do not drive inference in expressions. The preconditioners follow the same pattern.
* **Precision setting.** A case key `solver.precision` takes `double` (default) or `mixed`, with `--precision mixed` as its CLI form
  [F-6 (a)]. `single` stays refused.
* **Presets.** v1 (T1, T2) keeps low in f64; v2 sets low to `mixed` only after T3-2. The low claims carry over to v2 without
  recalibration, because T3-2 bounds the `mixed` against `double` difference at a hundredth of each budget.
* **GPU index range.** The existing range check (`check_index_range`) already refuses device matrices above `i32::MAX` rows or
  nonzeros; f32 values do not change it.

### D7 Cross-tier comparison and calibration

* **Calibration sets.** The case lists, per class, are in the validation plan (V-b). Class F has no calibration set until T5.
* **Per-run estimate from cheap runs.** A pair of one tier and the next finer tier of the same case gives an estimate of the
  coarser run's difference from xhigh.
  * Medium from (medium, high): `abs(f_m - f_h) (1 - 2^-p) / (1 - 2^(-p/2))`, a factor of 1.5 to 1.71 for p in [1, 2].
  * Low from (low, medium): `abs(f_l - f_m) (2^(p/2) - 2^-p) / (2^(p/2) - 1)`, a factor of 1.75 to 2.21.
  * The stated bound uses p = 0.5, factors 1.84 (medium) and 2.55 (low), because orders below 1 occur in the calibration cases:
    * Nu at Ra 1e5, 0.88 (ADR 0061 line 259);
    * the cylinder's time order, 0.68-0.87 (ADR 0062 line 158).

    A pair whose own triplet shows a lower order, or a change of sign, is refused. VE1 tests whether p = 0.5 suffices.
  * It is reported as "estimated difference from xhigh". The difference from the exact solution would use `r^p / (r^p - 1)`, a
    factor of 2 to 3.4 for p in [1, 2].
  * It needs no calibration, so it is offered from T1, labelled "coverage not yet validated" until VE1 passes.
  * The estimate holds only when the two runs use the same schemes. For a transient pair, which scales h and dt together, p is the
    smaller of the space and time orders: 1 for euler.
  * Model-axis error (closure, reduced-order) cannot be estimated this way; it is reported as the class's calibration bound
    (V-f).
* **Report.** A run at tier T reports the tier's budget, the class's calibration result and status, the per-run estimate when a pair
  exists, the refusals and no-claim flags that apply, and the platform. It is served by the MCP tool `tier_report` (D9) and shown
  in the Studio's compare view.
* **Correction (later, T6).** Additive or multiplicative bias models fitted on calibration pairs (H7). A corrected value is reported
  next to the raw one and never changes a registered limit.

### D8 How a claim is validated, and the ledger rule

The full procedure is the validation plan below; this section states its rules in short.

* **What a claim is.** A claim is (tier, class, quantity, budget). A miss becomes `*_known_miss` with the limit intact. The claim then
  reads MISSED in the claim table, and the tier stays available for that class without a claim.
* **Order (limits before the runs they judge).**
  * The budgets are fixed before any run.
  * xhigh and its finer partner run first.
  * An amendment then records xhigh's band and which checks are discriminating.
  * High, medium and low run last.

  The band uses only runs that no tier check judges (VR1), so a failing run cannot widen its own reference band.
* **When a check is discriminating.** xhigh's band U (VR1, VR3) must be at most a third of the budget. Hence SE <= budget/24 in
  Table B1, which leaves at least budget/4 for the mesh and time-step parts. Otherwise the check is reported as non-discriminating.
* **Speedups.** Measured by the protocol of V-d, with one sanity limit (every tier below xhigh is faster than xhigh), as ADR 0038
  check F1.
* **Ledger rule.**
  * Every entry from a tiered run or an xlow evaluation carries its tier.
  * A design is reported as chosen only with a `cross_tier_check` entry at high or above [F-8 (b)]. A new function writes it,
    comparing the higher tier's metrics against the lower tier's budget; `rerun_check` refuses other evaluator versions, so it is
    not reused.
  * The report names both tiers; Pareto and best refuse sets that mix tiers. VL1 checks the rule.

### D9 Tier access through the MCP project tools (F-13, decided by the maintainer)

Decision (maintainer, 2026-10-28): the tier must be reachable through the MCP tools, for rapid prototyping. The tools go on the
`--project` tool set. The default tool list stays untouched: 33 tools, `crates/rcfd-mcp/tests/golden/tools_list_default.json`, ADR
0135 check F1, never re-recorded.

| Tool | Status | Access | Arguments added | Result added |
|---|---|---|---|---|
| `start_run` | changed | execute | `fidelity` (optional): `xhigh`, `high`, `medium`, `low` or `xlow` | the `fidelity` block as recorded in `run.json` (D5); `xlow` returns the new error 1203, "xlow is not a run", naming `evaluate_xlow`; a tier knob the case cannot run returns 1203 with the located reasons; a value outside the enum is -32602 (the schema validator checks `enum`, `crates/rcfd-mcp/src/lib.rs` lines 537-541) |
| `list_runs` | changed | read | none | `tier` on each tiered row, omitted for untiered rows |
| `run_status` | unchanged | read | none | `settings.fidelity` is already visible in `settings` |
| `list_tiers` | new | read | `case`, `tier` (both optional; `tier` needs `case`) | the presets version; per tier the knob rules, Table C budgets, Table D bands, and the claim status per class and quantity with its check ids (V-c); with `case` and `tier`, the dry run: expansion, knobs not applied, estimated cost multiplier; nothing written |
| `tier_report` | new | read | `run`; `against` (optional, 1 to 5 higher-tier runs of the same case snapshot) | the recorded expansion and claim; per quantity the budget, and the estimate with its confidence statement (V-f) or `estimate: null` with refusal codes; with `against`, the measured differences and pass or fail against the budget |
| `evaluate_xlow` | new | read | `case` (an `axial_fan_sector` case), `conditions` (1 to 20, each of `rpm`, `rho`, `nu`, `system_k`, `system_l`), `flows` (optional) | strip-model results built by `fan_case::fan_of` (`crates/rcfd-api/src/fan_case.rs` line 187) and `RomFan::from_axial_fan` (`crates/rcfd-post/src/fan_rom.rs` line 65), with `RomModel::default()` (line 190) and the ADR 0119 evaluator's tone settings (`c0`, observer, `crates/rcfd-api/src/design/fan_rom_eval.rs` lines 25-29). With `flows`: curve points from `FanRom::curve` (line 524), each with its stalled strips (line 309), and points with stalled strips carry `no_claim_zone`. Without `flows`: operating points from `operating_point` (line 530), where a stalled point or a rising characteristic is a located refusal, not a number (lines 529-546). Values: `q`, `dp_s`, `dp_t`, `power`, `torque`, `eta_s`, `eta_t`, `tone1_pa`. Nothing written |
| `query_designs` | new | read | `ledger` (a path under `designs/`); `tier`, `study`, `pareto` ([{metric, sense}]), `chosen` (all optional) | entries with their tier; Pareto and best within one tier, mixed tiers refused; with `chosen`, exactly the designs that have a `cross_tier_check` at high or above with verdict `within_budget` (D8), with those entries |
| `record_design` | new | write_files | `ledger`, `study`, `variant`, `params`, and `run` or an `evaluate_xlow` result | one appended entry with `tier` and the suffixed evaluator id; its index |
| `verify_design` | new | write_files | `ledger`, `entry` (index of a lower-tier entry), `run` (a higher-tier run of the same case snapshot) | one appended `cross_tier_check` entry: both tiers, per-quantity differences, the lower tier's budgets, verdict `within_budget` or `budget_exceeded`; a run of another snapshot, or at the same or a lower tier, is refused. A check at medium is recorded, but only one at high or above makes the design chosen |

Names, allow-list and tests:
* **Names.** Every name passes the `AllowRule::Tool` rule (an ASCII letter, then letters, digits and `_`; `crates/rcfd-claude/src/allow.rs`
  lines 134-145), so none has a `-`. The new full names have at most 39 characters (for example
  `mcp__plugin_rcfd_project__evaluate_xlow`), under the 128 limit. No new `AllowRule` kind.
* **Allow-list.**
  * `PROJECT_READ_TOOLS` (`allow.rs` line 301) grows from 11 to 15 names: `list_tiers`, `tier_report`, `evaluate_xlow`,
    `query_designs`. `AllowList::project_initial()` (line 401) then seeds 15 rules `mcp__rcfd_project__<tool>` for a new project.
  * An existing `.rcfd/claude-allow.json` is not rewritten. The new read tools are offered through the existing "allow this" path
    (`suggest`, line 609) the first time they are denied.
  * `record_design`, `verify_design` and `start_run` stay off the first list (read-class only, ADR 0134 W48-11 (a)), so each call needs
    the user's approval unless the user adds a rule.
* **Argument checks.** The schema validator has no `minItems`, `maxItems` or conditional `required`. The counts ("1 to 5", "1 to 20")
  and the conditions (`tier` needs `case`; `run` or an `evaluate_xlow` result) are therefore handler checks. They return -32602, as
  `run_list_arg` does (`crates/rcfd-mcp/src/project.rs` lines 639-646).
* **Tests and texts that change.**
  * `crates/rcfd-mcp/tests/project_w48f.rs`:
    * `PROJECT_TOOLS` goes from 17 to 23 entries, and the line-85 equality with `PROJECT_READ_TOOLS` must still hold;
    * the `--project` tool count at line 99 (`33 + 17`, then `33 + 23`) and the F3 summary text at line 132;
    * F3's read-only loop (lines 117-127) needs argument fixtures for `record_design` and `verify_design`. Arguments are validated
      before the policy check (`lib.rs` lines 706 and 716), so a bare `run` argument would give -32602, not 1100;
    * the ADR 0135 checks F4 (paths) and F8 (output size) cover the new path arguments (`case`, `ledger`) and the new read tools;
    * F12's skill list (line 697) gains `tier-prototype`.
  * `crates/rcfd-mcp/tests/project_runs_w48f.rs`: the tool count at line 100 goes from 50 to 56, and the shapes test calls the new
    tools, because every golden key must be recorded.
  * `crates/rcfd-mcp/tests/golden/project_shapes.json`: the new result shapes; untiered `list_runs` rows stay unchanged.
  * Two texts name the project tools and need updating: `PROJECT_INSTRUCTIONS` (`crates/rcfd-mcp/src/lib.rs` line 57) and
    [`docs/mcp-tools.md`](../mcp-tools.md) line 95 ("adds 17 tools").
  * `crates/rcfd-claude/tests/w48e_library.rs` needs no edit: line 405 follows the list's length.
* **What stays fixed.**
  * `tools_list_default.json` and the 33-tool count (`crates/rcfd-mcp/src/tests.rs` line 295) do not change.
  * The default `design_ledger_*` tools keep their schemas.
  * Their results on ledgers without tiered entries do not change. On a ledger with tiered entries, their Pareto and best refuse
    mixed tiers, as builds without this change already do through the evaluator suffix.
* **Plugin and docs.**
  * A new skill, `clients/claude-plugin/skills/tier-prototype/SKILL.md`: preview a tier, start it, report against a higher tier,
    record and verify the design. Its `allowed-tools` name only project tools (ADR 0135 check F12).
  * `start-and-watch` mentions `fidelity`, and [`docs/mcp-tools.md`](../mcp-tools.md) gains the rows.
  * Every result stays under the project tools' 36 000-byte budget; `list_tiers` pages its claim table.

## Validation plan for the accuracy claims

This plan validates three things: the budgets of Table C (per tier, class and quantity), the speedup bands of Table D, and the
per-run estimator of `tier_report`. Each check is a proposal; the stage ADR fixes its set-up and limit before any run it judges.

House rules apply:
* a missed limit becomes an ignored `*_known_miss` test with the limit intact;
* a defective set-up gets an amendment registered before the corrected run;
* what was known before registration, and the smoke results, are disclosed (rules c and d of ADRs 0110-0135).

### V-a The reference ladder: xhigh and its own band

| Class | xhigh | xhigh's band U |
|---|---|---|
| S, steady | the case at h/2, Table B1 settings | from xhigh and its finer partner: h/4 where a ratio-2 family supplies it, otherwise h/(2 sqrt 2). `U = Fs abs(f_x - f_p) r^p / (r^p - 1)`, the Richardson error of the coarser member of the pair. Fs is 3, the conservative factor of a two-grid study (ADR 0086 also uses 3 when its order is not confirmed, lines 95-96); p is the formal order of the schemes as written |
| S, transient | h/2 and dt/2 | the space part as above at xhigh's dt, plus the time part from xhigh's mesh at dt/2 and dt/4, plus 2 SE of the time mean |
| F | wall-resolved LES of the full annulus (T5) | two LES resolutions reported as a sensitivity (a GCI is not defined when the filter width is part of the model), plus 2 SE; no budget is discriminating until measured data bound the model error |

| Id | Check | Set-up | Limit (registered before any run) | Negative control |
|---|---|---|---|---|
| VR1 | xhigh's band per quantity | xhigh and its partner run first, before any lower tier | U recorded in an amendment before any lower-tier run; a quantity's checks are discriminating only where U <= budget/3 | a partner identical to xhigh (no refinement) is refused as a degenerate pair |
| VR2 | the band is not optimistic | after all runs: the observed order from (high, xhigh, partner) and its Richardson value | the Richardson value lies within U of xhigh. Reported only: the triplet contains a judged run (high), so a miss is a known miss that flags the band as optimistic in the claim table, but it changes no check's status | a triplet whose differences change sign reports "order undefined" |
| VR3 | the transient band | the cylinder: space pair at xhigh's dt, time pair on xhigh's mesh, SE by batch means (VQ2) | each part recorded; U as above | a sequence that refines h and dt together (the defect ADR 0062 corrected, lines 65-66) is not accepted as a band |
| VR4 | iteration error of xhigh and its partner | the ADR 0086 rule (line 101): the last-1000-iteration change of each quantity | below 0.1 abs(f_x - f_p); otherwise an amendment tightens the tolerances before the lower tiers run | a run stopped at a tenth of its iterations fails |
| VR5 | the class F reference (T5) | two LES resolutions, SE, comparison with measured data (part G2) | registered in T5 | |
| VR6 | xhigh against the published or exact value | each class S case's vendored reference (V-b) | `abs(f_x - f_ref) <= U` plus the reference's own stated accuracy, with U as frozen in the VR1 amendment. It is decided only where U is at most a third of the quantity's smallest budget; otherwise it reads "non-discriminating", never "pass". A miss becomes a known miss, and that quantity's claim reads "consistency with xhigh only" | the swept pipe should miss: its centreline error is 3.93, 3.46, 3.33 and 3.30 % at nx = 5, 9, 17 and 33, a bias refinement does not remove (`crates/rcfd-api/tests/pipe_sweep.rs` lines 148-151, ADR 0047) |

### V-b The calibration set per class

**VC0, the case-file gate (the first step of T2).** The tiers act on case files through `expand`. Most validation tests, however,
build their solvers directly through `rcfd_fv`; only `pipe_sweep.rs`, `study_w20c.rs` and `fan01_gci_w37c.rs` go through the case
tree.
* **Check.** Each calibration case first gets a case file. VC0 checks that the case file, on the harness's own mesh, reproduces the
  harness's registered value.
* **Limit.** Within 0.1 %, or within the harness's own fine-mesh GCI if that is larger.
* **Failure.** A case that misses VC0, or that the case tree cannot express, stays out of T2 until an amendment fixes the cause.
* **Negative control.** The case file with `physics.nu` doubled misses.

| Class | Case and its registered reference (path, ADR) | Case file today | Table C row | Members |
|---|---|---|---|---|
| S, 2D steady | lid-driven cavity Re 100, 400 and 1000 (`crates/rcfd-api/tests/validation_cavity_ghia.rs`, ADR 0060) | yes: `cavity_2d`, as in `crates/rcfd-api/tests/baseline_hashes.rs` line 16 | velocity extremum (u_min) | medium is the coarsest registered mesh; xhigh (2n) and the partner (4n) are the registered ratio-2 members; high and low are new |
| S, 2D steady | heated cavity Ra 1e3, 1e4, 1e5 and 1e6 (`crates/rcfd-fv/tests/validation_natural_convection.rs`, ADRs 0061, 0062) | yes: `box_2d` with energy, as in the case-tree test at `crates/rcfd-api/src/sim.rs` line 4380 (Ra 1e3) | mean Nu; velocity extremum (u_max) | as the cavity |
| S, 2D steady | Poiseuille channel (`crates/rcfd-cli/tests/study_w20c.rs`, ADR 0034; observed order about 2.7, from 1.90 % at ny 10 and 0.29 % at ny 20, ADR 0099 lines 31-32) | yes | pressure drop | new members from `expand` |
| S, 2D steady | developing turbulent channel (`cases/channel/kepsilon_inlet.toml`) | yes | pressure drop | new members |
| S, 3D steady | swept pipe (`crates/rcfd-api/tests/pipe_sweep.rs`, ADR 0047) | yes | flow resistance (U_b at fixed forcing); VR6's bias case | new members: the registered nx 5, 9, 17, 33 are not `expand`'s counts |
| S, 2D steady | SST and k-epsilon channels, Re_tau 395 (`cases/channel/sst_channel.toml`, `kepsilon_channel.toml`) | yes | skin friction | new members |
| S, 2D transient | cylinder Re 100 (`validation_cylinder_re100_w31.rs`, ADR 0062), written with `time_scheme = bdf2` as the ADR 0069 run | yes: `cylinder_ogrid` (`crates/rcfd-api/src/schema.rs` line 78), equivalence by VC0 | drag, Strouhal | medium 64x50, xhigh 128x100 and partner 181x141 are registered meshes; high is `expand`'s 90x71 (the registered study used 90x70); low is 45x35 |
| S, 3D steady, fixed SST closure | FAN-01 passage (`crates/rcfd-api/tests/fan01_gci_w37c.rs`, ADR 0086); a 500 mm fan, not a computer fan | yes | torque, pressure rise | B3, B2 and B1 as medium, high and xhigh (a sqrt 2 family), re-run at the T2 commit (about 1.5 h, by hand). There is no partner (a finer member exceeds the 60-minute guard), so there is no band and every check is reported, not limited |
| S, 2D steady | laminar step Re_D 800 (`validation_bfs_step.rs`, ADR 0060) and Re 200 and 400 (`crates/rcfd-fv/tests/step_laminar.rs`) | no: the case tree has no step generator | reattachment | after a case-file equivalent passes VC0 |
| S, 2D steady, separated RANS | turbulent step BFS-C, RNG k-epsilon (`crates/rcfd-turb/tests/step_kepsilon_corrected.rs`, ADR 0015) | no: needs a step geometry and the measured inlet profile | reattachment | as above; its own grid study has ratio 1.5, not `expand`'s steps |
| S, 2D steady | Blasius, SST and SA plates (`validation_blasius_plate.rs`, ADR 0060; `crates/rcfd-turb/tests/validation_w33c_sst_plate.rs`, ADR 0070; `crates/rcfd-turb/tests/flat_plate_sa.rs`, ADR 0014) | to be shown: `box_2d` with wall layers; the SST family also scales the layer count and growth, which no tier knob does | skin friction | after VC0 |
| S, 2D steady | Graetz plates (`crates/rcfd-fv/tests/validation_graetz.rs`, ADR 0061) | no: a prescribed analytic velocity | local Nu_x, no Table C row | excluded |
| S, ranking | 8 heated-cavity variants at Ra 1e5 and 8 at Ra 1e4, aspect ratios 0.85 to 1.2 (`box_2d`, `lx`, `ly`). Adjacent xhigh values must differ by 0.5 to 2 low budgets, checked on the xhigh runs before the lower tiers run; an amendment re-spaces a set that misses | yes | ranking | all new (cheap) |
| F, model axis | the computer fan (T5) | | the fan rows of Table C, tones, stall onset, ranking | blocked on the T5 prerequisites and measured data |
| xlow evidence | strip model against FAN-01 measurements (ADR 0119 V1) | | pressure rise, power | reported against reality, not against xhigh |

Cases per Table C row that can be discriminating, from case files today (and after the harness-only cases pass VC0):
* velocity extremum: 7;
* mean Nu: 4;
* pressure drop or flow resistance: 3;
* skin friction: 2 (5);
* reattachment: 0 (4);
* drag: 1;
* Strouhal number: 1.

A row needs at least 3 discriminating cases for a claim (V-c). Drag and Strouhal therefore need new cases: proposed, the cylinder at Re 40 (steady drag) and at Re 200 (transient), both with `cylinder_ogrid`. Reattachment needs the harness-only cases to pass VC0.

Not calibrated by this set:
* the 3D transient speedup band, which stays an estimate;
* turbulent heat transfer;
* every class F model-axis claim.

### V-c Error metrics and pass or fail

| Quantity type | Metric | Pass or fail per case |
|---|---|---|
| integrated scalars (Nu, C_D, Cf, Delta p, velocity extremum, torque, pressure rise, flow) | `e = abs(q_T - q_x) / abs(q_x)` | `e <= budget` |
| scalars on an absolute scale (St, x_r/h, efficiency) | `abs(q_T - q_x)` | `<= budget` |
| time means | as above, with SE_d the combined SE of the difference d | pass if `abs(d) + 4 SE_d <= budget`; fail if `abs(d) - 4 SE_d > budget`. Otherwise the averaging is extended once to 4 times its length and the same rule is applied again; if it is still undecided, the comparison is reported as undecided and counts as non-discriminating |
| ranking of 8 or more variants | Spearman rho between the tier's order and xhigh's, with the exact permutation p-value over all orders (new code); whether xhigh's best is in the tier's top k | low: rho >= 0.9 and top 3; xlow: rho >= 0.7 and top 5 |
| fields | relative L2 norm of `q_T - q_x` on xhigh's mesh after `conservative` transfer (`rcfd-post::remap`, [`docs/run-control.md`](../run-control.md)) | reported only in T2; any limit comes in a later ADR, from these numbers |

Claim status per (tier, class, quantity), computed from the registered results and returned by `list_tiers`:
* **calibrated**: every discriminating case passes, and there are at least 3 discriminating cases;
* **MISSED**: a discriminating case fails, and its test becomes `*_known_miss`;
* **non-discriminating**: fewer than 3 discriminating cases;
* **not validated**: not yet run.

VM1 (unit test): the claim table equals the one rebuilt from the registered outcomes, and every claim names its check ids.

### V-d Speedup measurement protocol

| Item | Rule |
|---|---|
| Machine and build | the Windows gate host, a release build at the stage commit, no gate or other run on the machine; `--threads 1`, with `--threads 16` reported beside it |
| What is timed | **tier time**: from the start of `rcfd run` to the tier's own stop, mesh generation and output included; **same-criterion time**: every tier stopped when each quantity changes by less than 1e-4 relative over 100 iterations (steady) or between two averaging windows (transient) |
| Repeats | one discarded warm-up run, then 5 timed runs: median, min and max. A spread (max/min) above 1.3 repeats the set once, and both sets are reported. The cold time (the first run after a restart) is reported once per case |
| Memory | peak working set of each run, beside the runner's estimate (4 KB per cell) |
| Measured or estimated | a Table D cell becomes MEASURED with case, commit and platform; every other cell stays ESTIMATE |
| Limit | tier time below xhigh's for every tier (sanity); everything else reported, as ADR 0038 check F1 |

The checks:
* VS1, class S timings (every V-b case and tier);
* VS2, memory;
* VS3, FAN-01 numerical-axis timings, reported. The FAN-01 anchors of Table E were taken two at a time on a shared machine (ADR 0086
  lines 159-160); VS3 repeats them under this protocol.

### V-e Statistical rigour

* **Size of the set (VQ1).** A claim whose row passes on all of its n discriminating cases is stated with the one-sided 95 %
  Clopper-Pearson lower bound on the pass rate of similar cases, `0.05^(1/n)`.
  * It is computed with `clopper_pearson` (`crates/rcfd-api/src/design/uq.rs` line 403) called with confidence 0.90, because the
    function is two-sided (line 404).
  * The values are 0.37 for n = 3, 0.47 for n = 4, 0.55 for n = 5 and 0.65 for n = 7 (V-b's rows).
  * A statement pooled over the rows of a class assumes the rows are exchangeable, and is labelled as pooled.
* **Spread of the errors.** Per (tier, class, quantity), the median error, the largest error and the number of discriminating cases are
  reported, not only pass or fail, so a claim that passes narrowly is visible.
* **Time means (VQ2).**
  * Batch means with at least 40 batches, each at least one shedding period (two integral times for a signal that is not periodic).
  * Stationarity: the means of the two halves differ by at most 2 SE.
  * SE <= budget/24 (Table B1). The undecided band of V-c is then at most 0.24 of the budget on each side of it.
* **Ranking (VQ3).** At least 8 variants per set, spaced as V-b requires, so that the ranking is not trivially monotone. The exact
  permutation p-value is reported beside rho; for 8 variants that is 40 320 orders, computed rather than read from a table.
  Negative control: the reversed order (rho = -1) fails.
* **Multiple comparisons.** Deterministic quantities have no chance misses. For time means, the 4 SE margin (ADR 0121 uses the same
  K = 4) is a t-quantile when the SE comes from 40 batches (39 degrees of freedom). A false fail then has a one-sided probability of
  1.4e-4 per comparison, about 2 % over 150 comparisons (computed for this ADR; with 20 batches it would be 3.8e-4 and about 6 %).
* **Tolerance spread (VQ4).**
  * A tier may evaluate a manufacturing-tolerance study (part F1, ADR 0121) only if its budget for the quantity is at most a third of
    the output standard deviation being estimated.
  * The tier's error is a bias that the study's SE does not contain, so the study reports the SE and the tier budget separately.
* **Seeds.** The variant sets are fixed lists. Any sampled set registers its seed, as ADR 0121 does.

### V-f The per-run estimator of `tier_report`

* **How it is built.**
  * Same-case pair (D7): the coarser run's difference from xhigh, bounded with the p = 0.5 factor, 1.84 for medium and 2.55 for low.
    It needs no calibration and is offered from T1, labelled "coverage not yet validated" until VE1 passes. Only the pair refusals
    below apply to it, not `class_not_calibrated` or `outside_envelope`.
  * Class bound: the largest calibration error of the tier in that quantity over the row's n cases, with the VQ1 statement. For a new
    case exchangeable with them, it covers with probability n/(n+1), a rank argument (0.8 for n = 4).
  * When both exist, both are reported and the stated bound is the larger.
* **Envelope.** Per class, the ranges spanned by the calibration cases:
  * Reynolds or Rayleigh number, and Prandtl number;
  * first-cell y+;
  * medium's cells per characteristic length;
  * 2D or 3D, steady or transient.

  A run outside any range gets no class bound: refusal `outside_envelope`, naming the parameter.
* **Refusals.** `estimate: null`, with a code and a located reason.
  * Class bound only:
    * `class_not_calibrated`;
    * `outside_envelope`.
  * Pair estimate only:
    * `schemes_differ`;
    * `not_in_family`: a first cell not scaled, Table B1 notes;
    * `non_monotone`: the pair's own triplet, where one exists, changes sign or shows an order below 0.5.
  * Both:
    * `claim_void`;
    * `not_applied`;
    * `not_converged`: stopped by a cap, a guard or divergence;
    * `no_claim_zone`: stall or a rising fan characteristic, separation onset outside a calibrated separated case, a transitional
      chord Reynolds number, tones, broadband noise, turbulent heat transfer.

| Id | Check | Set-up | Limit | Negative control |
|---|---|---|---|---|
| VE1 | coverage of the pair bound | every class S case and quantity: (medium, high) and (low, medium) pairs, against the measured difference from xhigh | at least 90 % of (case, quantity) pairs covered; the Clopper-Pearson lower bound is reported, and that bound, not 90 %, is the rate `tier_report` states | the medium pair's bound applied to low's difference covers under 90 % (in the asymptotic range low's difference is 1.8 to 2.3 times medium's, D2) |
| VE2 | coverage of the class bound on new cases | Leave-one-out cannot test a largest-error bound: every held-out case except the largest is covered. Each case added to a row after T2 is therefore a VE2 check, registered before its runs, against the bound built without it | the new case's error is within the bound; a miss is a known miss, and the bound is rebuilt with the case included | high's bound applied to the new case's low error misses (low's difference is 4.4 to 7 times high's, D2) |
| VE3 | refusals | fixture records for each refusal code (no solver runs) | each returns `estimate: null` with the expected code and location | an in-envelope, converged class S pair returns an estimate |
| VE4 | stated rates never exceed validated ones | every `tier_report` output on the T2 runs | the stated coverage equals VE1's lower bound; the class bound carries VQ1's statement | (built in) |

### V-g Regression guards

| Where | Guards |
|---|---|
| CI unit tests (Linux on every pull request, Windows on main) | written as unit tests inside `src`, because CI runs `--lib --bins` only: T1-2 expansion goldens; T1-8 no libm; the VE3 refusal fixtures; the estimator arithmetic on synthetic families (exact power laws give the D7 factors to 1e-12); VM1 |
| Gate (integration tests, per-test limit 600-3600 s) | `baseline_hashes` and T1-1 identity; T1-4, T1-6, T1-9 and T1-10 records and tools, with the default tool list byte-identical; VC0 where it fits; VG2; the T3 checks that fit |
| By hand, recorded in the addendum | xhigh and partner runs over the per-test limit; the V-d timing sets (they need an idle machine); VE1 over the whole set; GPU (T4); the Linux gate on request; the Studio view-and-report |

**VG2, the claim drift guard (gate).** Low, medium and high of the cavity Re 100 and the heated cavity Ra 1e4 are re-run and compared
with the xhigh values recorded at T2. Those values sit in a fixture file written once at T2; only a new registered stage replaces it,
and the old file is kept. Limit: each within its budget. A failure follows the house rule: the test becomes an ignored
`*_known_miss` with the limit intact, the claim table marks the class "stale", and recalibration is a new registered stage.

### V-h The ledger rule

| Id | Check | Set-up | Limit | Negative control |
|---|---|---|---|---|
| VL1 | the ledger records re-verification | through the MCP project tools on a fixture project: 8 cavity variants (lid Re 100 to 400) recorded at low with `record_design`; the best by `query_designs` (tier low); `verify_design` against a high run of that variant | one `cross_tier_check` entry naming the low entry's index, the high run id, both tiers, per-quantity differences, the low budgets and the verdict; `query_designs` with `chosen` lists exactly this design, with both tiers | `chosen` excludes a design whose only check is at medium, and one whose check reads `budget_exceeded`; `verify_design` refuses a run of another snapshot and a run at the same tier, with located messages |
| VL2 | the tier on every entry | every entry written in VL1 | each carries `tier` and the suffixed evaluator; main a89ccb3's reader skips none and refuses a Pareto set that mixes tiers | an entry written with `fidelity: "low"` is skipped by the old reader (T1-4) |

### V-i What the validation cannot prove

* **That xhigh is right.**
  * VR1-VR4 bound numerical error under refinement, and VR6 compares with published values only where they exist.
  * Model-form error (closure, transition, installation) is outside every band.
  * Class F has no measured data.
* **That a claim transfers beyond its envelope**: other geometries, imported or unstructured meshes, other Reynolds ranges, other
  platforms.
* **More than the set supports.** With 3 to 7 discriminating cases per row, the strongest per-row statement is that at least 37 % to
  65 % of similar cases pass, at 95 % confidence. Rare failures are not excluded, and rows with fewer than 3 cases carry no claim.
* **Bias that refinement does not remove.** A band built from refinement cannot see it (the swept pipe, VR6); only a published or
  exact value can.
* **Speedups on other hardware or under load**, and the 3D transient band, which has no calibration case.
* **The no-claim physics**: rankings near stall or separation onset, tones below the transient tiers, broadband noise below xhigh,
  turbulent heat transfer.
* **Mixed precision outside the calibrated cases**: that it is harmless for ill-conditioned or stiff systems.
* **Permanence.** Claims hold for the solver version that produced them; VG2 detects drift only on its guarded cases.

## Staged plan

Stages T1 to T6. Wave numbers are the maintainer's: waves 49 and 50 are planned for the Studio
([`docs/ui-restart-plan.md`](../ui-restart-plan.md) section 4). Each stage is its own branch and ADR (or amendment), gated as in
[`docs/test-process.md`](../test-process.md). The checks are a proposal, to be fixed before any run.

```text
T1 records, expansion, MCP tier tools  -->  T2 validation of class S  --+-->  T3 mixed precision, CPU  -->  T4 GPU f32
   (T1-1..T1-10, VE3, VM1, VL1, VL2)        (VC0, VR1-VR4, VR6, V-b,    |     (T3-1..T3-7)
                                             V-c, VS1-VS3, VQ1-VQ4,     +-->  T6 correction, ledger rule in the Rust design loop
                                             VE1, VE4, VG2; VE2 after)
T5 class F (VR5, the fan rows of V-b, V-c, V-f): needs a converging fan route (ADRs 0110-0117), F-250, a wall-bounded
   LES validation, F-100 or device assembly, and measured fan data (part G2)
```

### T1: records, expansion, MCP tier tools (no solver change)

| Id | Check | Limit | Negative control |
|---|---|---|---|
| T1-1 | `baseline_hashes`; on 4 class S cases (cavity `simplec`, cylinder `piso` 50 steps, heated cavity, SST plate 200 iterations) `rcfd run` without `--fidelity` and with `--fidelity medium` | hashes pass unchanged; `solution.vtu`, `monitors.json` byte-identical; status streams equal without `wall_s` | `--fidelity high` changes `solution.vtu` |
| T1-2 | expansions of every tier for 6 fixture cases (2D, 3D, transient, fan passage, imported mesh, refused model) | equal to golden strings and hashes, in a unit test that runs on the Linux and Windows CI jobs | a preset edited without a version change fails |
| T1-3 | a user `--set` after the expansion | the user's value is used; `claim` reads `void: <path>` for a claimed knob, not for `output.store_interval` | the same entry placed before the expansion is overwritten |
| T1-4 | old readers: `run.json` and ledger lines written by the new code, read by main a89ccb3's readers (fixture approach of ADR 0130 A4); an old runner started from the record | listed, not skipped; the same solution bytes from `set` | a ledger line with `fidelity: "low"` is skipped by the old reader |
| T1-5 | imported mesh at high; RANS with `energy.enabled` at any tier | "not applied: mesh" recorded, no claim; a located error | a generator mesh at high is applied |
| T1-6 | `start_run` with `fidelity`, `list_runs` with `tier` (D9) | untiered rows unchanged (`tier` omitted); a tiered run added to `project_shapes.json`; `tools_list_default.json` byte-identical (ADR 0135 check F1) | `fidelity: "ultra"` is -32602 |
| T1-7 | ledger Pareto and best (Rust API) on a set with mixed tiers | refused by new readers, and by old readers through the evaluator suffix | a single-tier set is accepted |
| T1-8 | no `powi` and no libm call reachable from `expand` | unit test over all presets; code review recorded | (built in) |
| T1-9 | tool names and the allow-list (D9) | the 6 new names pass `AllowRule::Tool` in the plugin and the app-owned forms, at most 128 characters, no `-`; `PROJECT_READ_TOOLS` equals the 15 read-class project tools (`project_w48f.rs` line 85); `project_initial()` seeds 15 rules; an existing allow-list file is byte-identical after the upgrade | a name with `-` is refused, as in ADR 0135 check F2 |
| T1-10 | the tier tools on a fixture project (D9) | `list_tiers` shapes, every claim "not validated" before T2; `start_run` at low records the expansion; `xlow` gives 1203 naming `evaluate_xlow`; `tier_report` on a cavity (medium, high) pair returns the D7 estimate labelled "coverage not yet validated"; `evaluate_xlow` on the default `axial_fan_sector` case at 3000 rpm, rho 1.2 and nu 1.5e-5 (set-up C of ADR 0119), with `flows` from 0.5 to 1.5 times the case's inlet flow, returns finite points with any stalled strips flagged; the new skill's `allowed-tools` name only project tools | `evaluate_xlow` on a cavity case is a located error |

T1 also runs VE3, VM1, VL1 and VL2 (fixtures and cheap cavity runs).

### T2: validation of class S (no solver change)

* **Set-up.**
  * VC0 comes first: each calibration case as a case file, checked against its harness (V-b).
  * Medium is each case's coarsest registered mesh. Registered meshes are reused only where `expand` reproduces their counts (V-b
    names them); the other members are new.
  * Every member is re-run at the T2 commit with its tier's settings, because the recorded results used other tolerances and time
    steps (the cylinder sequence, for example, a common dt of 0.025).
  * The calibration therefore measures the ladder on each case's registered coarse mesh, not on a case written at another
    resolution. D7 gives a per-run estimate for other cases.
* **Checks.**
  * VC0;
  * VR1-VR4 and VR6;
  * the V-b class S cases with the V-c metrics, against Table C's class S budgets;
  * the FAN-01 passage, reported only;
  * VS1-VS3, VQ1-VQ4, VE1 and VE4;
  * the VG2 fixture, written once at the end.

  VE2 starts with the first case added after T2.
* **Order.** As in D8: xhigh and the partners first, then the amendment with the bands and the discriminating checks, then high,
  medium and low.
* **Negative control per case.** The case's tiers compared with an xhigh run at another Reynolds or Rayleigh number must miss.
* **Poiseuille.** Its order is about 2.7: 1.90 % at ny 10 and 0.29 % at ny 20 (ADR 0099 lines 31-32). The ny 10 test stays a known
  miss (`study_w20c.rs` line 744).
* **Where the runs happen [F-11 (c)].** The gate's per-test limit is 5 times the recorded time, clamped to 600-3600 s (ADR 0129 D1).
  Longer runs are run by hand and recorded in the addendum. The laminar step's registered fine mesh alone took 2166 s (MEASURED, ADR
  0060 line 209).

### T3: mixed precision on the CPU (opt-in)

| Id | Check | Limit | Negative control |
|---|---|---|---|
| T3-1 | `double` default, run first | `baseline_hashes` unchanged; `--precision double` byte-identical | |
| T3-2 | T2 cases at medium, `mixed` against `double` | every quantity within a hundredth of its high budget (Table C) | an f32 pressure matrix without refinement misses on at least one case |
| T3-3 | pressure solve with f32 preconditioners, the 1M-unknown Poisson problem of [`docs/gpu.md`](../gpu.md) | reaches the f64 run's `tol_rel` (1e-6, as in that benchmark); iterations at most 1.3 times the f64 count | |
| T3-4 | iterative refinement on the 64^3 and 100^3 Poisson problems | true residual, computed in f64, at most 1e-10 relative | plain f32 CG stagnates above 1e-6 |
| T3-5 | `--threads 1` against `4` in `mixed` | byte-identical | |
| T3-6 | SpMV and whole-run time, 1M and 5M Poisson and the T2 cases | reported under the V-d protocol; sanity: `mixed` SpMV faster than `double` | |
| T3-7 | the low-tier V-b checks with presets v2 (`mixed`) | every low check that passed in T2 still passes | an f32 pressure solve without refinement misses one |

### T4, T5, T6

* **T4 GPU f32 path.**
  * T4-1 f32 SpMV and preconditioner with refinement reach the f64 device run's `tol_rel` on the 1M and 5M Poisson problems;
  * T4-2 every GPU test prints a located SKIP line without a device;
  * T4-3 speedup reported against the f64 device path and against CPU `mixed`.
* **T5 class F (blocked).**
  * Prerequisites:
    * a fan route that converges (FAN-01 stall, ADRs 0110-0117);
    * a transient sliding interface (F-250);
    * LES validated on a wall-bounded case;
    * distributed runs or device assembly for 1e7-1e8 cells: the runner's default budget refuses runs above about 1e6 cells;
    * measured computer-fan data (part G2).
  * Checks to register then:
    * VR5;
    * the class F rows of V-b, V-c and V-f, including VE1 and VE2 for class F;
    * T5-1 ranking of 8 or more variants at medium, low and xlow against high;
    * T5-2 Table C's fan budgets against xhigh on two variants;
    * T5-3 every tier against measured data, reported apart from T5-2.
* **T6 correction and the ledger rule in the Rust design loop.** The MCP project tools enforce the rule from T1 (VL1).
  * T6-1 the `design` Rust API refuses to report a chosen design without a `cross_tier_check` entry, with a located message;
  * T6-2 a bias correction fitted on class S pairs reduces the held-out low-tier error (limits registered at T6 start);
  * T6-3 the Rust API refuses Pareto sets with mixed tiers.

  The default `design_*` MCP tools keep their schemas (F-13); on ledgers with tiered entries their Pareto and best refuse mixed
  tiers (D9).

### What agents cannot check

* Agreement with physical reality. Tiers are measured against xhigh; xhigh against reality needs measured data (part G2 for fans).
* GPUs other than the gate host's RTX 5080.
* Linux bit-identity of the integration tests, unless the maintainer runs `gate.yml` on `ubuntu-latest`.
* xhigh fan runs, which take days and exceed this machine's memory, and anything that needs a cluster.
* How the Studio presents tiers: view-and-report by the user.
* Whether the budgets fit the decisions they serve: the maintainer's judgement (F-10).

### Risks

* **Generic matrix types move bits.** The change touches every solve. T3-1 runs before any other T3 check.
* **A tier read as a guarantee.** "low, 7 %", quoted by Claude or the Studio for a case outside a calibrated class, would mislead.
  Every report carries the claim status and the no-claim flags (D7).
* **Rapid prototyping makes cheap numbers easy to quote.** With the tier on the MCP tools (D9), a low or xlow number is one call away.
  Every tier tool result carries the claim status and the refusals, and `tier_report` refuses rather than guesses (V-f).
* **Medium is only as good as the author's mesh [F-1].** Under the relative ladder every class S claim is conditional on the case as
  written. The per-run estimate (D7) is the only per-case evidence.
* **Preset drift.** Presets changed in place would make two runs at "low" incomparable. Versions are in `run.json`, and T1-2 fixes
  them.
* **Expensive references.** xhigh runs can take hours, and xhigh fan runs do not fit this machine. F-11 moves long runs out of the
  gate, and the runner refuses runs over its memory budget.
* **One release of duplication.** The ledger's evaluator suffix duplicates `tier`, and evaluator filters on `cfd` stop matching. Its
  removal needs a later ADR, as `case_source` does in ADR 0130.

## Limits

1. **Separation and stall.** Lower tiers get separation onset wrong in both directions.
   * First-order schemes and coarse meshes add numerical viscosity, which delays separation.
   * RANS closures move it.
   * The strip model stalls early: 33 of 40 strips are stalled at the FAN-01 design point, and its onset is at 1.59 m^3/s against
     about 1.1 measured (ADR 0119 V1).
   * An efficiency optimiser on the strip model ran to the stall edge (ADR 0119).

   Hence the no-claim zones of Table C, and a stall-margin constraint on any optimisation below high.
2. **Transition.** Computer-fan chord Reynolds numbers, 2e4 to 2e5, are transitional, with laminar separation
   ([`docs/fan-project-breakdown.md`](../fan-project-breakdown.md) section 1). No transition model exists. Fully turbulent RANS and the
   strip model's laminar friction (a lower bound) bracket the loss without predicting it.
3. **Noise.**
   * The steady-loading tone of the ADR 0119 rotor is 1.25e-9 Pa at 1 m, which is inaudible.
   * Real tones come from unsteady loading, which needs the transient tiers.
   * Broadband noise needs scale-resolving flow, so only xhigh can predict it.
4. **Heat transfer.** No turbulence model runs with the energy equation, so turbulent Nusselt numbers are outside every tier.
5. **Pressure rise is the fragile quantity.**
   * On the FAN-01 passages, one sqrt 2 step moved Delta p_ts by -13 % and torque by -3.5 %; two steps moved them by -37 % and -9 %
     (ADR 0086).
   * The finest member's Richardson band is +-40 %.
   * A pressure budget can fail where torque passes.
6. **Speedups do not multiply exactly.**
   * The GPU is slower than one CPU thread at 4096 unknowns (about 90 us of latency per iteration), so a coarse tier can lose on the
     device.
   * The matrix upload is 63-74 % of the one-shot GPU time.
   * Distributed efficiency is 0.20 at 8 ranks (ADR 0058 B9).
   * ISAT ranges from 0.23x to 118x depending on reuse (ADR 0053).
   * Mesh generation, AMG set-up and output do not shrink with the tier.
   * Where the physics sets the step (steps per blade passage for tones), a transient tier has a floor on its step count.
7. **Cost set by caps and cycles.**
   * Where runs end at an iteration cap (FAN-01, 8000), tolerance knobs do not change the cost.
   * Where a scheme limit-cycles (ADR 0110), the speedup is undefined.
   * Convergence and accuracy can part ways. On the FAN-01 B3 passage, switching to first-order `upwind` after 1000 iterations
     brought the residuals below 1e-5 by iteration 1114, where limited `linear_upwind` had stalled. Even so, `is_converged()` stayed
     false, and the first-order torque, 1.335 N m, is far from the second-order 1.850 N m (ADR 0110 lines 228-230 and 281; ADR 0086
     line 172).
8. **Platform floats.** Bit-identity of every tier is per platform and build (D4). An expansion is platform-independent only because
   it uses no `powi` and no libm.
9. **Mixed precision.** These are why D6 keeps the state, limiters and refinement residuals in f64; T3-4's negative control shows the
   stagnation.
   * An f32 state loses small increments.
   * f32 limiter decisions can flip and start limit cycles.
   * f32 inner solves stagnate near 1e-6 relative.
10. **xhigh for the fan is a foundation, not a setting.**
    * Wall-resolved LES of a 120 mm fan needs of the order of 1e7 to 1e8 cells (ESTIMATE, UNVERIFIED). The FAN-01 passages run so
      far have 4 920 to 38 184 cells.
    * It needs F-250, distributed runs or device assembly, and LES validation.
    * Until then, class F budgets cannot be checked.

## Open decisions (maintainer)

1. **F-1, anchor of the ladder.**
   * (a) medium is the case as written; other tiers are relative steps.
   * (b) absolute presets per case class only.
   * (c) (a) for class S and for any case, absolute presets for a class once its definitions are decided (class F, T5).

   (a) works for any case and gives bit-identity by construction, but medium is only as good as the author's mesh; (b) leaves most
   cases without tiers. **Recommendation: (c).**
2. **F-2, carrier.**
   * (a) a run setting only (CLI flag, runner, MCP argument, Studio); case files and `rcfd schema` unchanged.
   * (b) a case key `fidelity.tier`.
   * (c) both.

   **Recommendation: (a).** A tier says how a case is run, and the expansion in `run.json` keeps it reproducible.
3. **F-3, step.** (a) sqrt 2 per direction, which makes medium, high and xhigh a GCI family; (b) 2 per direction, which makes xhigh
   64 times medium's cells in 3D. **Recommendation: (a).**
4. **F-4, a knob that cannot apply.** (a) refuse; (b) apply the rest, record "not applied", no claim; (c) skip silently.
   **Recommendation: (b) for numerical knobs, (a) for model knobs, never (c).**
5. **F-5, mixed-precision scope.**
   * (a) f32 preconditioners.
   * (b) also f32 values and u32 indices for non-pressure matrices.
   * (c) also f32 corrections in pressure refinement.
   * (d) a full f32 solve.

   **Recommendation: (a) and (b) in T3; (c) only after T3-4 passes; never (d) for pressure.**
6. **F-6, precision setting.** (a) case key `solver.precision` with `--precision mixed` as its CLI form; (b) the CLI flag only.
   **Recommendation: (a)**, so an expansion can set it and the snapshot records it.
7. **F-7, tier in the ledger.** (a) a new `tier` field; (b) the tier in the evaluator id (`cfd@low`); (c) both for one release, then
   (a). (a) alone lets older builds mix tiers in Pareto queries. (b) makes `rerun_check` (`ledger.rs` line 812) need suffixed model
   ids, and filters on `cfd` stop matching. **Recommendation: (c).**
8. **F-8, re-verification of a chosen design.** (a) at xhigh; (b) at high or above; (c) one tier up. **Recommendation: (b)**, with
   xhigh before a design is built or published.
9. **F-9, class F presets.**
   * (a) Table B2: xhigh is wall-resolved LES of the full annulus with struts; high is SA-DDES of the full annulus.
   * (b) xhigh is DDES; high is URANS, which needs transient SST or SA, which do not exist.
   * (c) until (a) runs, the highest available tier is labelled "xhigh-available" and no class F budget is claimed.

   **Recommendation: (a) as the definition, (c) meanwhile.**
10. **F-10, budgets.** (a) Table C, judged by the largest difference over the calibration set; (b) a quantile over a larger set;
    (c) other numbers. **Recommendation: (a)**: the sets are small, and a quantile needs more cases than exist.
11. **F-11, expensive calibration runs.** (a) in the gate; (b) by hand, recorded in the addendum; (c) (a) under the per-test limit
    and (b) above it. **Recommendation: (c).**
12. **F-12, placement.** (a) T1 now as its own track, in parallel with wave 49 (no solver change); (b) after wave 50.
    **Recommendation: (a)**, then T2, then T3.
13. **F-13, the tier in the MCP tools: DECIDED by the maintainer, 2026-10-28.** The tier must be reachable through the MCP tools for
    rapid prototyping. It goes on the `--project` tool set (D9), and the default tool list stays byte-identical (ADR 0135 check F1).
    This is no longer open. The earlier recommendation kept the tier off the MCP tools except through the run path; the decision
    replaces it.

## Proposed roadmap items (ids to be assigned by the maintainer)

* Fidelity setting, expansion, records, and the MCP tier tools of D9 (T1).
* Validation of the class S claims: reference ladder, calibration, speedup protocol, estimator coverage (T2, the validation plan).
* Mixed-precision solves on CPU and GPU (T3, T4).
* Cross-tier ledger rule and multi-fidelity correction (T6; joins part H7 of the fan breakdown).

## Files

This ADR only: `docs/adr/0136-fidelity-tiers-xhigh-to-xlow.md`. No code, test, default or baseline is changed.
