# ADR 0136: Fidelity tiers xhigh to xlow: one `fidelity` setting expanded into explicit knobs, error budgets against an xhigh reference, target speedups

Status: **PROPOSED 2026-10-28** (scoping only; nothing implemented or run). No decision below is taken: the maintainer decides F-1 to
F-13. Written at main a89ccb3. House rules as in [ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed
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
  transforms that apply to any case. Schemes stay as written (Table B1 notes).
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
| xlow | optimiser inner loops, tolerance studies (ADR 0121 driver), instant curves in the Studio, the bound a CFD result must respect (C7) | an evaluator in the design loop (strip theory, lumped network, correlation), no run folder; offered only where such a model exists |

Table B1, the numerical axis (class S), relative to the case as written (cell size `h`, time step `dt`):

| Knob | xhigh | high | medium | low | Available today |
|---|---|---|---|---|---|
| Precision | f64 | f64 | f64 | f64 in presets v1; mixed from v2, after T3 (D6) | f64 only |
| Cell size, generator counts | h/2 | h/sqrt 2 | h | sqrt 2 h | counts by `--set`; imported mesh: not applied (D3) |
| First cell | refined with h only where y+ <= 1 | as xhigh | as written | as written, never scaled | keys exist |
| Time step, transient | dt/2 | dt/sqrt 2 | dt | sqrt 2 dt | keys exist |
| Inner linear tolerance | `pressure_rel_tol` 1e-6 steady, 1e-10 transient | as xhigh | as written | 1e-2 steady | keys exist |
| Outer stop | `tolerance_u`, `tolerance_p` 1e-9 and a relative `change` monitor below 1e-7 on each report | as xhigh | as written | 1e-4 and a relative `change` monitor at 1e-3 | keys exist |
| Time means | until each mean's SE <= budget/12 (D8) | as xhigh | as written | as written | `statistics`, `piso` and `pimple` only |

Notes on Table B1:
* **Schemes stay as written at every tier.** Space and time schemes are not tier knobs. Medium, high and xhigh therefore differ in h
  and dt only, and form a refinement family.
* **First-order cases.** A case written with a first-order scheme keeps it at every tier. Its xhigh then rarely meets the D8
  uncertainty rule, so its checks are reported as non-discriminating.
* **Scheme upgrades.** An upgrade, for example euler to bdf2, would be a separate step outside this ladder; none is proposed here.
* **Tolerances.** They change the iteration error only. D8 checks that every member's iteration error is below a tenth of the
  differences between members.

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
| Mean Nusselt number, laminar | S | 1 % | 3 % | 7 % | 15 % (correlation, canonical geometry only) | turbulent heat transfer (not computable today) |
| Drag coefficient, laminar bluff body | S | 1 % | 3 % | 7 % | no claim | |
| Strouhal number (absolute) | S | 0.002 | 0.005 | 0.01 | no claim | |
| Reattachment length (step heights) | S | 0.1 | 0.3 | 0.7 | no claim | |
| Velocity extremum of a laminar recirculation | S | 1 % | 3 % | 7 % | no claim | |
| Skin friction, attached boundary layer | S | 1 % | 3 % | 7 % | 15 % (correlation) | transition |
| Pressure drop, internal flow | S | 1 % | 3 % | 7 % | 15 % (lumped network) | |

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
* **Laminar Nusselt number and drag.** The fine-mesh GCI of heated-cavity Nu is 0.014-0.053 % (MEASURED: ADR 0061 lines 257-259; ADR
  0062 line 118, which supersedes ADR 0061's 0.13 % at Ra 1e6). The 1 % high budget is therefore discriminating for Nu under the D8
  rule. It is not discriminating at high for cylinder C_D (GCI 0.96 %) or St (GCI 1.62 %, about 0.0026 absolute), both from ADR 0062
  lines 165-166. At medium, C_D passes the rule only if the time-mean SE is below 0.02 %: the allowance is 1 %, the GCI takes 0.96 %,
  and twice the SE must fit in the rest. Otherwise it is non-discriminating there too. Such checks are reported as
  non-discriminating unless a finer reference is run.

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
  * It is set with `rcfd run --fidelity <tier>`, `RunSettings.fidelity` in the runner, an optional `fidelity` argument of the MCP
    project tool `start_run`, or a select box in the Studio's start dialog. That dialog is proposed in
    [ADR 0133](0133-wave48d-run-control-and-live-monitoring.md) and is not in `crates/rcfd-studio/src` at a89ccb3.
  * Case files are unchanged, and `tools_list_default.json` stays byte-identical.
  * `rcfd schema` is unchanged by T1 and T2; T3 adds the key `solver.precision` [F-6 (a)].
* **One expansion function, `rcfd_api::fidelity::expand(case, tier, presets)`.**
  * It returns an ordered list of `path=value` entries in the `--set` syntax, and the knobs it could not apply, each with a located
    reason.
  * Presets are data compiled into the binary, with a version (`rcfd-fidelity-1`). A changed preset is a new version, never an edit
    (T1-2).
  * A dry run (`rcfd fidelity <case> --tier <t>`) shows the expansion and the estimated cost multiplier before a start. So does a
    read-class MCP tool, which exists in `--project` mode only.
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
* **xlow is not a run.** The design loop sends the variant to the strip-theory or lumped evaluator. The ledger records it with
  `fidelity = reduced_order` and tier xlow.

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
* `RunSummary` gains an optional `tier`, omitted when absent, so the Studio run list, `list_runs` and `compare_runs` show and filter
  it. `compare_runs` adds relative differences. When the first run is a higher tier of the same case, it adds the D7 report.
* **Ledger.**
  * A new optional field `tier`; old readers ignore it. ADR 0121 already calls the three-valued `fidelity` a tier, and it keeps its
    three values: a tier value there would make old readers skip the line.
  * Old readers do not know `tier`, so their Pareto and best queries could mix tiers. For one release the evaluator id also carries
    the tier (`cfd@low`), which old readers already keep apart [F-7 (c)].
  * A new entry kind `cross_tier_check` (kind is free text) records re-verification (D8).
  * In T1 the tier reaches the ledger through the run path and the Rust API only. The default `design_ledger_*` MCP tools keep their
    schema, which is in the golden that is never re-recorded [F-13 (a)].

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

* **Calibration sets.** Class S (T2) uses cases whose references are in the repository and which already run mesh families:
  * lid-driven cavity Re 100 and 400, `crates/rcfd-api/tests/validation_cavity_ghia.rs`;
  * laminar step, Re_D 800, `validation_bfs_step.rs`;
  * Blasius plate, `validation_blasius_plate.rs`;
  * cylinder Re 100, C_D and St, `validation_cylinder_re100_w31.rs`;
  * heated cavity, Ra 1e3 to 1e6, Nu, `crates/rcfd-fv/tests/validation_natural_convection.rs`;
  * Graetz plates, Nu_x, `validation_graetz.rs`;
  * Poiseuille pressure drop, `crates/rcfd-cli/tests/study_w20c.rs`;
  * SA and SST flat plates, Cf, `crates/rcfd-turb/tests/flat_plate_sa.rs` and `validation_w33c_sst_plate.rs`.

  Class F has no calibration set until T5.
* **Per-run estimate from cheap runs.** When a case runs at medium and high only, the two runs give an estimate of medium's
  difference from xhigh: `abs(f_m - f_h) (1 - 2^-p) / (1 - 2^(-p/2))`, a factor of 1.5 to 1.7 for p in [1, 2]. It is reported as
  "estimated difference from xhigh". (The difference from the exact solution would use `r^p / (r^p - 1)`, a factor of 2 to 3.4.)
  The estimate holds only when the two runs use the same schemes. For a transient pair, which scales h and dt together, p is the
  smaller of the space and time orders: 1 for euler.
  Model-axis error (closure, reduced-order) cannot be estimated this way; it is reported as the class's calibration result, "largest
  difference over N calibration cases".
* **Report.** A run at tier T reports the tier's budget, the class's calibration result and status, the per-run estimate when a pair
  exists, the no-claim flags that apply, and the platform. It is served by `compare_runs` and a `--project` read-class MCP tool, and
  shown in the Studio's compare view.
* **Correction (later, T6).** Additive or multiplicative bias models fitted on calibration pairs (H7). A corrected value is reported
  next to the raw one and never changes a registered limit.

### D8 How a claim is validated, and the ledger rule

* **What a claim is.** A claim is (tier, class, quantity, budget). A miss becomes `*_known_miss` with the limit intact. The claim then
  reads MISSED in the presets' claim table, and the tier stays available for that class without a claim.
* **Order of a calibration (limits before the runs they judge).**
  * The budgets are fixed before any run, in this ADR or at T2 registration.
  * The xhigh member and its finer partner run first.
  * An amendment then records which budget checks are discriminating, before any lower-tier run.
  * High, medium and low run last.
* **When a check is discriminating.** The xhigh reference's uncertainty, U = GCI_fine plus 2 SE for time means, must be at most a third
  of the budget. Hence SE <= budget/12 in Table B1, which leaves budget/6 for the GCI. Otherwise the check is reported as
  non-discriminating.
  * For a transient case the GCI combines two separate studies: a space study at xhigh's dt and a time study on xhigh's mesh.
    Refining h and dt together mixes the two errors, a set-up defect ADR 0062 corrected (lines 65-67). Each family member's iteration error must be below 0.1 of the differences between members (the ADR 0086 rule,
  line 101), because medium keeps its own tolerances.
* **Speedups.**
  * Reported as medians of 3 runs: release build, gate host, `--threads 1`, no other run on the machine.
  * One sanity limit: every tier below xhigh is faster than xhigh. This follows ADR 0038 check F1, where every other benchmark number
    is reported, not limited.
  * Timing noise is real: [`docs/gpu.md`](../gpu.md) records 1.94 s and 2.84 s for the same solve.
* **Ledger rule.**
  * Every entry from a tiered run carries its tier.
  * A design is reported as chosen only with a `cross_tier_check` entry at high or above [F-8 (b)].
  * A new function compares the higher tier's metrics against the lower tier's budget and records the difference. `rerun_check`
    refuses other evaluator versions, so it is not reused.
  * The report names both tiers.
  * Pareto and best refuse sets that mix tiers.

## Staged plan

Stages T1 to T6. Wave numbers are the maintainer's: waves 49 and 50 are planned for the Studio
([`docs/ui-restart-plan.md`](../ui-restart-plan.md) section 4). Each stage is its own branch and ADR (or amendment), gated as in
[`docs/test-process.md`](../test-process.md). The checks are a proposal, to be fixed before any run.

```text
T1 records, expansion --> T2 calibration, class S --+--> T3 mixed precision, CPU --> T4 GPU f32
                                                    +--> T6 ledger rule, correction
T5 class F: needs a converging fan route (ADRs 0110-0117), F-250, a wall-bounded LES validation,
            F-100 or device assembly, and measured fan data (part G2)
```

### T1: records, expansion, tools (no solver change)

| Id | Check | Limit | Negative control |
|---|---|---|---|
| T1-1 | `baseline_hashes`; on 4 class S cases (cavity `simplec`, cylinder `piso` 50 steps, heated cavity, SST plate 200 iterations) `rcfd run` without `--fidelity` and with `--fidelity medium` | hashes pass unchanged; `solution.vtu`, `monitors.json` byte-identical; status streams equal without `wall_s` | `--fidelity high` changes `solution.vtu` |
| T1-2 | expansions of every tier for 6 fixture cases (2D, 3D, transient, fan passage, imported mesh, refused model) | equal to golden strings and hashes, in a unit test that runs on the Linux and Windows CI jobs | a preset edited without a version change fails |
| T1-3 | a user `--set` after the expansion | the user's value is used; `claim` reads `void: <path>` for a claimed knob, not for `output.store_interval` | the same entry placed before the expansion is overwritten |
| T1-4 | old readers: `run.json` and ledger lines written by the new code, read by main a89ccb3's readers (fixture approach of ADR 0130 A4); an old runner started from the record | listed, not skipped; the same solution bytes from `set` | a ledger line with `fidelity: "low"` is skipped by the old reader |
| T1-5 | imported mesh at high; RANS with `energy.enabled` at any tier | "not applied: mesh" recorded, no claim; a located error | a generator mesh at high is applied |
| T1-6 | `start_run` with `fidelity`, `list_runs` with `tier`, the dry-run tool | untiered rows unchanged (`tier` omitted); a tiered run added to `project_shapes.json`; `PROJECT_READ_TOOLS` gains the new read tools and the `project_w48f.rs` equality holds; `tools_list_default.json` byte-identical (ADR 0135 check F1) | the dry run of an unknown tier is a located error |
| T1-7 | ledger Pareto and best (Rust API) on a set with mixed tiers | refused by new readers, and by old readers through the evaluator suffix | a single-tier set is accepted |
| T1-8 | no `powi` and no libm call reachable from `expand` | unit test over all presets; code review recorded | (built in) |

### T2: calibration of class S (no solver change)

* **Set-up.** Medium is each case's coarsest registered mesh. The meshes of the existing families are reused. Every member is re-run
  at the T2 commit with its tier's settings, because the recorded results used other tolerances and time steps (the cylinder sequence,
  for example, a common dt of 0.025).
  * In the ratio-2 families (h, h/2, h/4: cavity, step, Blasius, heated cavity, Graetz, SST plate), the middle mesh is xhigh's and
    the finest gives xhigh's uncertainty. High and low need new meshes.
  * The cylinder study meshes (64x50, 90x70, 128x100, ADR 0062 line 60) are already a sqrt 2 family: they give medium, high and
    xhigh, and low needs a new mesh.
  * The SA plate has no refinement family (140 x 110 cells, `crates/rcfd-turb/tests/flat_plate_sa.rs` line 6), so all its members
    are new.
  * The calibration therefore measures the ladder on each case's registered coarse mesh, not on a case written at another
    resolution. D7 reports a per-run estimate wherever a high partner exists.
* **Order and limits.**
  * Runs follow the D8 order.
  * Limits are Table C's class S budgets, registered only where the D8 rule holds.
  * Speedups are reported against the sanity limit.
  * The Poiseuille family's own order is about 1 (first-order wall treatment, known miss `study_w20c.rs` line 744). The p = 2 ratios
    of the budgets do not hold there.
* **Checks.** T2-1 cavity centreline u_min; T2-2 step x_r/h; T2-3 Blasius Cf; T2-4 cylinder C_D and St, time means with their SE;
  T2-5 heated cavity Nu; T2-6 Graetz Nu_x; T2-7 Poiseuille Delta p; T2-8 SA plate Cf; T2-9 SST plate Cf; T2-10 ranking of 8
  heated-cavity variants (Ra, aspect ratio) by Nu at low against xhigh.
* **Negative control.** Each check also compares its tier against an xhigh run at a different Re or Ra, which must miss.
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
| T3-6 | SpMV and whole-run time, 1M and 5M Poisson and the T2 cases | reported; sanity: `mixed` SpMV faster than `double` | |

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
    * T5-1 ranking of 8 or more variants at medium, low and xlow against high;
    * T5-2 Table C's fan budgets against xhigh on two variants;
    * T5-3 every tier against measured data, reported apart from T5-2.
* **T6 ledger rule and correction.**
  * T6-1 `cross_tier_check` entries, and a `design_*` report of a chosen design without one is refused with a located message;
  * T6-2 a bias correction fitted on class S pairs reduces the held-out low-tier error (limits registered at T6 start);
  * T6-3 the design tools refuse Pareto sets with mixed tiers.

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
13. **F-13, the tier in the default MCP tools.**
    * (a) Only through `--project` tools and the Rust API; the default `design_ledger_*` tools and `tools_list_default.json` are
      unchanged.
    * (b) Add `tier` to the default ledger tools. This changes a golden that ADR 0135 check F1 never re-records.

    **Recommendation: (a).**

## Proposed roadmap items (ids to be assigned by the maintainer)

* Fidelity setting, expansion, records and tools (T1).
* Tier calibration of class S (T2).
* Mixed-precision solves on CPU and GPU (T3, T4).
* Cross-tier ledger rule and multi-fidelity correction (T6; joins part H7 of the fan breakdown).

## Files

This ADR only: `docs/adr/0136-fidelity-tiers-xhigh-to-xlow.md`. No code, test, default or baseline is changed.
