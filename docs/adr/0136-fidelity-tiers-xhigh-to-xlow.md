# ADR 0136: Fidelity tiers xhigh to xlow: one `fidelity` setting expanded into explicit knobs, error budgets against an xhigh reference, target speedups

Status: **PROPOSED 2026-10-28** (scoping only; nothing implemented or run). No decision below is taken: the maintainer decides F-1 to
F-12. Written at main a89ccb3. House rules as in [ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed before
the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up is corrected by an
amendment registered before the corrected run; defaults of existing code stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs`
is not re-recorded. Parents: [`docs/fan-project-breakdown.md`](../fan-project-breakdown.md) parts C7, H3, H4 and H7;
ADRs [0119](0119-wave46b-reduced-order-axial-fan-tier.md), [0121](0121-wave46d-design-ledger-and-tolerance-study-driver.md), 0130,
[0038](0038-wave22-gpu-cuda.md) and [0058](0058-wave29d-distributed-parallel.md).

Every accuracy and speedup number in this ADR is a TARGET or an ESTIMATE, stated with the way it would be measured. Numbers marked
MEASURED are earlier results of this repository, cited where they were recorded; none is a result of this ADR. **Every number for the
computer-fan class is an estimate, because no fan reference at any fidelity exists in the repository.** The decision ids F-1 to F-12
belong to this ADR and are not roadmap items (F-001 to F-269); proposed roadmap items get ids from the maintainer, as in ADR 0130.

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
| Outer convergence | steady stop when `res_u < solver.tolerance_u` and `res_p < tolerance_p` (1e-6 each) or at `max_iterations` (2000); `convergence.<name>` monitors (`window_range`, `change`, `below`, `above`) can stop earlier; steady divergence guard on by default | `schema.rs` lines 884-896, 918, 1001-1009, 1266-1291; `crates/rcfd-api/src/sim.rs` lines 2857-2870 | the FAN-01 study runs ended at an 8000-iteration cap, not at a tolerance ([ADR 0086](0086-wave37c-fan01-solution-verification.md) lines 167-174) |
| GPU | CUDA CG and BiCGStab with Jacobi or Chebyshev for the pressure solve of `simple`, `piso`, `pimple`; matrix assembled on the CPU and uploaded per solve; kernels in `double` with `int` indices; the host casts `usize` to `i32` with `as`, no range guard | [`docs/gpu.md`](../gpu.md); `crates/rcfd-gpu/src/cuda/kernels.cu` line 8; `crates/rcfd-gpu/src/cuda/system.rs` lines 83-84 | MEASURED: 0.22x against one CPU thread at 4096 unknowns; matrix upload 63-74 % of the one-shot time; 10x to 15x one-shot at 5M unknowns |
| Threads, distributed | `--threads n` gives byte-identical solution files (MEASURED, [`docs/run-control.md`](../run-control.md)); `rcfd-dist` has an in-process transport only, no MPI (F-100 todo) | ADR 0058 | MEASURED B9: parallel efficiency 0.56, 0.38, 0.20 at P = 2, 4, 8 on 36 864 cells (not a cluster) |
| Space and time schemes | `solver.convection` (default `linear_upwind`; upwind, central, quick, blended, high_resolution, compressive, bounded_central, limiters); `solver.time_scheme` euler (default), bdf2, bdf2_bounded; adaptive steps (`max_courant`, `error_tolerance`, ...; all unset is the fixed `dt` bit for bit) | `schema.rs` lines 8, 853-856, 938-940; [`docs/case-schema.md`](../case-schema.md) "Wave 13" and "Adaptive time stepping" | `turbulence.convection` defaults to upwind |
| Mesh resolution | generator counts `mesh.nx`, `ny`, `nz` (`schema.rs` lines 83-108); wall layers `mesh.wall_layers`, `target_y_plus` (lines 224-273); fan passage `mesh.fan.n_up` to `n_gap`, `n_bl`, `y_plus` default 30 (`crates/rcfd-api/src/fan_case.rs` lines 54-64); `--set` applies in order, replacing the value, before the mesh is built (`crates/rcfd-cli/src/cmd_w19c.rs` lines 44-58, `crates/rcfd-api/src/chain.rs` line 77) | | no key scales all counts; imported meshes cannot be refined or coarsened; adaptation is a library for box lattices with no case hook (`rcfd_mesh::adapt`, ADR 0054 D7) |
| Solution verification | GCI in test helpers only, in two forms: `crates/rcfd-api/tests/validation_common/mod.rs` line 44 (constant ratio, order clamped to [1, 2], Fs 1.25) and `crates/rcfd-api/tests/fan01_gci_w37c.rs` (unequal ratios, ADR 0086) | | not a library |
| Turbulence closures | laminar; `mixing_length`; `k_epsilon` (standard, rng, realizable); `k_omega_sst` (sst, bsl); `spalart_allmaras` with `sa.des` = des or ddes; `smagorinsky`, `wale`; `rsm`; plugin | [`docs/case-schema.md`](../case-schema.md) "`turbulence`" | RANS steady only; DES, DDES and LES transient only; `rsm` without walls; every model refused with `energy.enabled` ("RANS turbulence together with the energy equation is not supported", `crates/rcfd-api/src/turb.rs` lines 394-399); no model with `ideal_gas` or the compressible solver |
| Steady, transient, time means | `simple`, `simplec` (default), `coupled` (laminar), `piso`, `pimple`, fractional step, compressible; `statistics` (means, rms) for `piso` and `pimple` only (`crates/rcfd-api/src/stats.rs` lines 87-94) | `schema.rs` lines 848-851, 1348-1361 | |
| Rotating machinery | steady rotating frame on a periodic passage, MRF zones, cyclic seams (opt-in), mixing plane, frozen-rotor cylindrical interface | [`docs/milestone-fan.md`](../milestone-fan.md) | FAN-01 stall open (ADRs 0110-0117); no transient sliding interface (F-250 todo) |
| Reduced-order, lumped | strip-theory axial-fan model `crates/rcfd-post/src/fan_rom.rs` with `FanRomEvaluator` (Rust only, not reachable from a case file: `crates/rcfd-api/src/design/fan_rom_eval.rs` line 2); lumped fan curve, porous jump, network pressure (`crates/rcfd-fv/src/lumped.rs`, F-113); actuator disk (`crates/rcfd-fv/src/actuator.rs`, F-218 partial); potential-flow start (`crates/rcfd-fv/src/potential.rs`) | ADR 0119 | MEASURED against FAN-01 data, not against CFD: design point dp_s -64 %, power -7.4 %, 33 of 40 strips stalled (ADR 0119 V1) |
| Chemistry acceleration | `chemistry.acceleration` = off, cluster, isat, with `accel_tolerance`; DRG is library only | `crates/rcfd-api/src/chemistry.rs` lines 95-102; [ADR 0053](0053-wave28b-chemistry-acceleration-combustion.md) | MEASURED: DRG 9x and 1.2x; ISAT 0.23x (a slowdown, I4) to 118x (I5) |
| Run records and tools | `run.json` `settings` is free JSON (`crates/rcfd-project/src/records.rs` lines 82-83); runner `RunSettings {priority, threads, mem_mb, set}` without `deny_unknown_fields` (`crates/rcfd-runner/src/records.rs` lines 18-33); runner memory budget 4096 MB, estimate 4 KB per cell, a run above the whole budget is refused (`crates/rcfd-runner/src/runner.rs` lines 30-58, 554-564); `RunSummary` has no settings (`crates/rcfd-project/src/project.rs` lines 82-97); MCP `start_run` takes `set` (`crates/rcfd-mcp/src/project.rs` lines 228-244); `compare_runs` gives differences to the first run | ADRs 0130, 0135 | the default MCP tool list is a golden that is never re-recorded (`crates/rcfd-mcp/tests/default_f1.rs`) |
| Design ledger | `fidelity` in {analytic, reduced_order, cfd}; `Fidelity::parse` rejects other values and the reader then skips the line; Pareto and best refuse sets that mix evaluators or fidelities; `rerun_check` refuses another evaluator id or version; `kind` is free text; unknown keys are ignored (fields read by name) | `crates/rcfd-api/src/design/ledger.rs` lines 71-99, 290-320, 502-522; ADR 0121 D1, D3, D4 | |

### What does not exist

* An f32 or mixed-precision solve, iterative refinement, an f32 GPU kernel, 32-bit host indices.
* A `fidelity` setting, preset, tier or profile in the case tree, CLI, runner, MCP tools or Studio, other than the ledger's
  three-valued `fidelity`.
* A transition model ([`docs/audit/partials-group-3.md`](../audit/partials-group-3.md): "No transition or intermittency code exists in
  any crate"), wall-modelled LES, IDDES, SAS, any turbulence model with the energy equation. Turbulent Nusselt numbers cannot be
  computed at any tier today.
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
* **Numerical axis**: mesh counts of generator meshes, time step, scheme order, linear and outer tolerances, precision mode. These
  are relative transforms that apply to any case.
* **Model axis**: closure, steady or transient, passage or full annulus, CFD or reduced-order. These are absolute choices per case
  class.

Two classes are defined now:
* **Class S (same model)**: laminar flows and RANS flows with a fixed closure. Tiers differ on the numerical axis only, and xhigh
  is reachable today.
* **Class F (computer fan)**: the model axis moves too, and xhigh is not reachable today.

### D2 The tier matrix

Table A, purpose:

| Tier | Who uses it, for what decision | Delivered as |
|---|---|---|
| xhigh | the maintainer, by explicit approval: the reference every lower tier is measured against; the last check of a design before it is built or published; broadband noise | a CFD run |
| high | the design loop's confirmation stage (H7, "CFD to confirm"): a shortlist of 3 to 10 variants; tones and stall margin once class F has transient runs | a CFD run |
| medium | everyday runs from the Studio and from Claude: operating points, fan curves (C5), comparisons | the case as written |
| low | screening sweeps and optimiser exploration (DOE, Nelder-Mead); CFD points of a tolerance study (F1) | a CFD run |
| xlow | optimiser inner loops, tolerance studies (ADR 0121 driver), instant curves in the Studio, the bound a CFD result must respect (C7) | an evaluator choice in the design loop (strip theory, lumped network, correlation); no run folder |

Table B, knob settings (numerical axis relative to the case as written: cell size `h`, time step `dt`):

| Knob | xhigh | high | medium | low | xlow | Available today |
|---|---|---|---|---|---|---|
| Precision | f64 | f64 | f64 | mixed (D6) | f64 | f64 only; mixed is T3 |
| Cell size, generator counts | h/2 | h/sqrt 2 | h | sqrt 2 h | none | counts by `--set`; imported mesh: not applied (D3) |
| First cell | refined with h only where y+ <= 1 | as xhigh | as written | as written, never scaled | | keys exist |
| Time step, transient | dt/2 | dt/sqrt 2 | dt | sqrt 2 dt | | keys exist |
| Space scheme | bounded second order required | as xhigh | as written | as written; first order only as a start-up | | exists |
| Time scheme | bdf2 | bdf2 | as written | as written | | exists |
| Inner linear tolerance | `pressure_rel_tol` 1e-6 steady, 1e-10 transient | as xhigh | as written | 1e-2 steady | | keys exist |
| Outer stop | `tolerance_u`, `_p` 1e-9 and a `change` monitor below 1e-7 relative on each quantity | as xhigh | as written | 1e-4 and a `change` monitor at 1e-3 relative | | keys exist |
| Closure, class F | wall-resolved LES (`wale`) | SA-DDES or URANS (F-9) | steady SST, wall-resolved | steady SST, `y_plus` 30-100 | strip theory | LES transient, no wall-bounded validation; DDES on SA only, not validated on separation; URANS missing (RANS is steady only) |
| Time dependence, class F | transient | transient | steady | steady | algebraic | sliding interface missing (F-250) |
| Domain, class F | full annulus, struts, inlet | full annulus | one periodic passage | one passage | B blades, no mesh | annulus mesh partial (B3) |
| Time means | until each mean's SE <= budget/6 | as xhigh | | | | `statistics`, `piso` and `pimple` only |

Table C, error budgets: TARGETS against the xhigh run of the same case, in percent of the xhigh value unless a unit is given:

| Quantity | Class | high | medium | low | xlow | No claim below xhigh |
|---|---|---|---|---|---|---|
| Fan pressure rise at a set flow | F | 3 % | 8 % | 15 % | 30 % | rising characteristic (stall region) and flows within 0.1 of design flow of stall onset |
| Fan flow at the operating point against a system curve | F | 2 % | 4 % | 8 % | 15 % | as above, and system curves that cross in the stall region |
| Fan efficiency | F | 1.5 points | 3 points | 6 points | 10 points | as above |
| Shaft power, torque | F | 2 % | 5 % | 10 % | 40 % | as above |
| Stall-onset flow | F | 0.05 of design flow | reported only | no claim | no claim | |
| Tones at blade-passing harmonics | F | 3 dB | no claim | no claim | lower bound only | broadband noise at every tier below xhigh |
| Ranking of 8 or more variants | F, S | | | rank correlation >= 0.9, true best in the tier's top 3 | >= 0.7, true best in the top 5 | |
| Mean Nusselt number, laminar | S | 1 % | 3 % | 7 % | 15 % (correlation, canonical geometry only) | turbulent heat transfer (not computable today) |
| Drag coefficient, laminar bluff body | S | 1 % | 3 % | 7 % | no claim | |
| Strouhal number (absolute) | S | 0.002 | 0.005 | 0.01 | no claim | |
| Reattachment length (step heights) | S | 0.1 | 0.3 | 0.7 | no claim | |
| Skin friction, attached boundary layer | S | 1 % | 3 % | 7 % | 15 % (correlation) | transition |
| Pressure drop, internal flow | S | 1 % | 3 % | 7 % | 15 % (lumped network) | |

Reasons for the numbers:
* **Class S, numerical axis only.** With error `C h^p`, p in [1, 2] and sqrt 2 steps, the differences to xhigh of high, medium and
  low are 0.33-0.41, 1 and 1.8-2.3 times the medium difference. (For p = 1: 0.207, 0.5, 0.914 of `C h`; for p = 2: 0.25, 0.75, 1.75
  of `C h^2`.) The budgets 1, 3 and 7 % keep that ratio.
* **Class F, flow against pressure.** At the operating point of a system curve `dp = k Q^2`, crossed by a fan curve of slope
  `s <= 0`, `dQ/Q = (d dp/dp) / (2 - s Q/dp)`. That is at most half the pressure error, so the flow budget is half the pressure
  budget.
* **Class F, model-axis budgets.** They are set so that medium is useful for trends, and they cannot be checked until a fan xhigh
  exists (T5). The only fan evidence: coarsening the FAN-01 passage by two sqrt 2 steps moved Delta p_ts by -37 % and torque by -9 %.
  This is MEASURED in ADR 0086, B3 against B1, on runs ended by the cap rule. Mesh coarsening alone does not deliver the low pressure
  budget on such meshes (Limits 5).
* **xlow.** The only evidence is ADR 0119 V1, which compares the strip model with measured data, not with xhigh, so it is evidence
  only.
  * At flows above 1.7 m^3/s, where no strip is stalled: within 2 to 14 Pa, and power 32-38 % high.
  * At the design point, with 33 of 40 strips stalled: -64 %.
* **Laminar Nusselt number and drag.** The existing families give a fine-mesh GCI of 0.014-0.053 % on heated-cavity Nu (MEASURED,
  ADRs 0061 and 0062) and 0.96 % on cylinder C_D (MEASURED, `validation_cylinder_re100_w31.rs`, ADR 0062). The 1 % high budget is therefore
  discriminating for Nu and borderline for C_D (D8 rule).

Table D, speedup TARGETS against xhigh (cost taken as cells x iterations or steps x work per iteration):

| Tier | Class S, 2D steady | Class S, 3D steady | Class S, transient | Class F |
|---|---|---|---|---|
| high | 2.5-5 | 2.5-6 | 3-6 | 10-50 |
| medium | 8-20 | 8-30 | 10-40 | 300-3000 |
| low | 30-120 | 30-200 | 40-300 | 1e3-3e4 |
| xlow | 1e4 or more where a lumped model or correlation exists | same | same | 1e6-1e9 |

Table E, where the factors come from:

| Knob | Factor | Basis | Kind |
|---|---|---|---|
| One sqrt 2 step coarser, steady | cells / 2 (2D), / 2.83 (3D); iterations / 1.0-1.8 | heated cavity Ra 1e6: 336, 1057, 3592 iterations at n = 32, 64, 128 (ADR 0062 line 117); FAN-01 at a fixed 8000 iterations: 7.1, 25.6, 58.8 min for 4 920, 13 627, 38 184 cells (ADR 0086 lines 172-174) | MEASURED anchors |
| One sqrt 2 step coarser, transient | as steady, and / 1.41 steps at a fixed Courant number | constant CFL | arithmetic |
| Tolerances | xhigh over medium x1.5-3 cost; low under medium /1.5-3 | Krylov iterations grow like log(1/tol) | ESTIMATE; no effect where runs end at a cap |
| Mixed precision | SpMV bytes per nonzero 16 -> 12 with `usize` columns (at most 1.33x), 16 -> 8 with u32 columns (at most 2x); GPU, already 32-bit indices: 12 -> 8 (at most 1.5x) and half the value upload; whole run 1.2-1.6x | bytes moved; assembly, gradients and reductions stay f64 (Amdahl) | arithmetic, ESTIMATE |
| Wall function instead of y+ about 1 (class F low) | cells / 1.5-3, faster convergence | fewer stiff near-wall cells | ESTIMATE |
| LES to steady RANS (class F) | cells / 10-50, transient with averaging to steady / 5-20 | wall-resolved LES needs wall-parallel spacing of tens of wall units (Chapman 1979; Choi and Moin 2012; recollection, UNVERIFIED) | ESTIMATE |
| Full annulus to one passage | / B (7 for set-up C of ADR 0119) | periodicity | arithmetic; needs steady periodic flow |
| CFD to strip theory | milliseconds against hours to days | evaluation time is not recorded in ADR 0119 | ESTIMATE |

The class F bands combine these factors:
* medium: 7 x (10-50) x (5-20) = 350-7000 before fixed costs, so the band is 300-3000.
* high: cells / 3-10 and steps / 2-5 for hybrid against wall-resolved LES.
* low: medium x 2.7-14.

Speedups do not multiply exactly (Limits 6).

### D3 How a tier is expressed

* **One setting, `fidelity`, carried by the run (F-2).** It is set with `rcfd run --fidelity <tier>`, `RunSettings.fidelity` in the
  runner, an optional `fidelity` argument of the MCP project tool `start_run`, or a select box in the Studio's start dialog. Case
  files are unchanged and `tools_list_default.json` stays byte-identical. `rcfd schema` is unchanged by T1 and T2; T3 adds the key
  `solver.precision` (D6, F-6).
* **One expansion function, `rcfd_api::fidelity::expand(case, tier, presets)`.**
  * It returns an ordered list of `path=value` entries in the `--set` syntax, and the knobs it could not apply, each with a located
    reason.
  * Presets are data compiled into the binary, with a version (`rcfd-fidelity-1`). A changed preset is a new version, never an edit
    (T1-2).
  * A dry run (`rcfd fidelity <case> --tier <t>`, and a read-class MCP tool) shows the expansion and the estimated cost multiplier
    before a start.
* **Order and overrides.** The expansion comes first, then the user's `--set` entries. A later entry wins (`cmd_w19c.rs` lines
  44-58), so every knob stays overridable. Overriding a knob that the tier's claim depends on is recorded as `claim: "void: <path>"`.
* **Arithmetic.** Mesh counts scale with integers: `n * 1414 / 1000`, rounded, at least the generator's minimum; xhigh takes `2 n`
  exactly. Time steps and tolerances use IEEE basic operations or decimal literals. No `powi`, `powf`, `exp` or other libm call is
  reachable from `expand`, so an expansion is identical on every platform (D4).
* **A knob that cannot apply (F-4).** A numerical knob on an imported mesh is recorded as not applied, and the run carries no claim.
  A model knob the case cannot run is refused with a located error by the existing validation, for example any turbulence model with
  `energy.enabled` (`turb.rs` lines 394-399).
* **xlow is not a run.** The design loop sends the variant to the strip-theory or lumped evaluator, and records it in the ledger with
  `fidelity = reduced_order`, tier xlow.

### D4 Default and bit-identity

* **`medium` is the identity on both axes**: its expansion is empty.
  * Without `--fidelity`, `expand` is not called, so the solver gets today's input. Unchanged: fields, face fluxes,
    `solution.vtu`, `monitors.json`, the status stream apart from `wall_s`, and every `baseline_hashes` value.
  * `--fidelity medium` gives the same files (T1-1).
* **What does change once a tier is given**: `run.json` gains `settings.fidelity`. That is the record, not the solution.
* **Bit-identity is per platform and build.**
  * `powi` rounds differently between platforms (`crates/rcfd-api/src/sim.rs`, the heated-cavity probe comment of commit 54e64a5).
  * The mesh generators call the platform libm, so generator hashes are recorded per platform
    (`crates/rcfd-mesh/src/non_conformal.rs`, commit e56e931).
  * The gate runs the integration tests, `baseline_hashes` among them, on the Windows gate host. CI runs unit tests (`--lib
    --bins`) on Linux and Windows (`.github/workflows/ci.yml`).
  * Tier checks name their platform. Comparisons across platforms use tolerances, never bits.

### D5 Records: run.json, status, Studio, MCP, ledger

`run.json` `settings`, as the runner would write it for a run at low with one user override:

```json
"set": ["mesh.nx=28", "mesh.ny=28", "solver.pressure_rel_tol=0.01", "solver.max_iterations=500"],
"fidelity": {"tier": "low", "presets": "rcfd-fidelity-1", "class": "S",
             "expansion": ["mesh.nx=28", "mesh.ny=28", "solver.pressure_rel_tol=0.01"],
             "user_set": ["solver.max_iterations=500"], "not_applied": [], "claim": "not validated"}
```

* `set` holds the effective list, so a runner that does not know `fidelity` (it ignores the key; there is no `deny_unknown_fields`)
  reproduces the run.
* `status.json` is unchanged: a run's tier is fixed.
* `RunSummary` gains an optional `tier` (additive), so the Studio run list, `list_runs` and `compare_runs` show and filter it.
  `compare_runs` adds relative differences. When the first run is a higher tier of the same case, it adds the D7 report.
* **Ledger.**
  * A new optional field `tier`; old readers ignore it.
  * `fidelity` keeps its three values. A tier value there would make old readers skip the line.
  * Old readers do not know `tier`, so their Pareto and best queries could mix tiers. For one release the evaluator id also
    carries the tier (`cfd@low`), which old readers already keep apart (F-7).
  * A new entry kind `cross_tier_check` (kind is free text) records re-verification (D8).

### D6 Mixed precision in the numerics

| Data or kernel | In `mixed` mode | Why |
|---|---|---|
| Field state, face fluxes, cell sums of fluxes | f64 | increments per step are small against the state; conservation sums cancel |
| Gradients, limiters, wall-law switches, upwind direction | f64 | a flipped limiter changes the discrete operator; limiter switching is the ADR 0110 limit cycle |
| Dot products, norms, residuals, monitors | f64, fixed chunk order | determinism for any thread count (`vecops.rs`) |
| Matrices of momentum, turbulence and scalar equations | f32 values, u32 indices (F-5) | diagonally dominant; already solved to `tol_rel` 1e-3 inside SIMPLE |
| Preconditioners (AMG levels, IC0, ILU0, Jacobi, Chebyshev) | f32 | changes the iteration count, not the converged answer |
| Pressure system | f64 matrix and Krylov vectors; optionally an f32 correction solve inside f64 iterative refinement (residual `b - A x` in f64) | refinement reaches f64 accuracy only while the condition number times the f32 unit roundoff (6e-8) stays below about 1, and pressure conditioning grows like h^-2 (Higham; Carson and Higham 2018; recollection, UNVERIFIED) |
| Checkpoints | f64 | bit-identical restart is a documented property ([`docs/run-control.md`](../run-control.md)) |
| Stored output fields | f32 allowed | exists (`rcfd-tsstore`) |
| GPU | f32 values (indices are already 32-bit), f64 dot accumulation and refinement on the device | consumer GPUs run f64 at a small fraction of f32 throughput (ratio of the RTX 5080 UNVERIFIED); the value upload halves |

Delivery:
* `CsrMatrix` and the preconditioners become generic over the existing `Scalar` trait and an index type, with `f64` and `usize` as
  default type parameters. Existing code then compiles unchanged and stays bit-identical.
* A case key `solver.precision` takes `double` (default) or `mixed`, with `--precision mixed` as its CLI form (F-6). `single` stays
  refused.
* The GPU index cast gets a range guard. Today `as i32` truncates silently above 2^31 - 1 entries (`system.rs` lines 83-84); the
  guard is needed before xhigh-size matrices reach the device.

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
* **Per-run estimate from cheap runs.** When a case runs at medium and high only, medium's numerical error is estimated as
  `abs(f_m - f_h) r^p / (r^p - 1)`, with r = sqrt 2 and p in [1, 2] (a factor of 2 to 3.4). It is reported as "estimated numerical
  error". Model-axis error (closure, reduced-order) cannot be estimated this way; it is reported as the class's calibration result,
  "largest difference over N calibration cases".
* **Report.** A run at tier T reports:
  * the tier's budget;
  * the class's calibration result and status;
  * the per-run estimate, when a pair exists;
  * the no-claim flags that apply;
  * the platform.

  It is served by `compare_runs` and a read-class MCP tool, and shown in the Studio's compare view.
* **Correction (later, T6).** Additive or multiplicative bias models fitted on calibration pairs (H7). A corrected value is reported
  next to the raw one and never changes a registered limit.

### D8 How a claim is validated, and the ledger rule

* **What a claim is.** A claim is (tier, class, quantity, budget), and its checks are registered before any run. A miss becomes
  `*_known_miss` with the limit intact. The claim then reads MISSED in the presets' claim table, and the tier stays available for that
  class without a claim.
* **When a check is discriminating.** A budget check is registered only where the xhigh reference's uncertainty, GCI_fine plus
  2 SE for time means, is at most a third of the budget; otherwise it is reported as non-discriminating. Each family member's
  iteration error must be below 0.1 of the differences between members (the ADR 0086 rule, line 101), because medium keeps its own
  tolerances.
* **Speedups.** Reported as medians of 3 runs: release build, gate host, `--threads 1`, the same load. One sanity limit: every tier
  below xhigh is faster than xhigh. This follows ADR 0038 F1, where every other benchmark number is reported, not limited. Timing noise
  is real: [`docs/gpu.md`](../gpu.md) records 1.94 s and 2.84 s for the same solve.
* **Ledger rule.**
  * Every entry from a tiered run carries its tier.
  * A design is reported as chosen only with a `cross_tier_check` entry at high or above (F-8).
  * A new function compares the higher tier's metrics against the lower tier's budget and records the difference.
    `rerun_check` refuses other evaluator versions, so it is not reused.
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
            F-100 or device assembly, and measured fan data (G2)
```

### T1: records, expansion, tools (no solver change)

| Id | Check | Limit | Negative control |
|---|---|---|---|
| T1-1 | `baseline_hashes`; on 4 cases (cavity `simplec`, cylinder `piso` 50 steps, heated cavity, SST plate 200 iterations) `rcfd run` without `--fidelity` and with `--fidelity medium` | hashes pass unchanged; `solution.vtu`, `monitors.json` byte-identical; status streams equal without `wall_s` | `--fidelity high` changes `solution.vtu` |
| T1-2 | expansions of every tier for 6 fixture cases (2D, 3D, transient, fan passage, imported mesh, refused model) | equal to golden strings and hashes, in a unit test that runs on the Linux and Windows CI jobs | a preset edited without a version change fails |
| T1-3 | a user `--set` after the expansion | the user's value is used; `claim` reads `void: <path>` for a claimed knob | the same entry placed before the expansion is overwritten |
| T1-4 | old readers: `run.json` and ledger lines written by the new code, read by main a89ccb3's readers (fixture approach of ADR 0130 A4); an old runner started from the record | listed, not skipped; the same solution bytes from `set` | a ledger line with `fidelity: "low"` is skipped by the old reader |
| T1-5 | imported mesh at high; RANS with `energy.enabled` at any tier | "not applied: mesh" recorded, no claim; a located error | a generator mesh at high is applied |
| T1-6 | `start_run` with `fidelity`, `list_runs` with `tier`, the dry-run tool | shapes equal a new version of the `--project` golden (the old one kept); `tools_list_default.json` byte-identical (ADR 0135 F1) | the dry run of an unknown tier is a located error |
| T1-7 | ledger Pareto and best on a set with mixed tiers | refused by new readers, and by old readers through the evaluator suffix | a single-tier set is accepted |
| T1-8 | no libm call reachable from `expand` | unit test over all presets; code review recorded | (built in) |

### T2: calibration of class S (no solver change)

* **Runs.** Each D7 case runs at xhigh, high, medium and low.
* **Limits.** Table C's class S budgets, registered only where the D8 uncertainty rule holds. Speedups are reported against the
  sanity limit.
* **Checks.**
  * T2-1 cavity u_min;
  * T2-2 step x_r/h;
  * T2-3 Blasius Cf;
  * T2-4 cylinder C_D and St, time means with their SE;
  * T2-5 heated cavity Nu;
  * T2-6 Graetz Nu_x;
  * T2-7 Poiseuille Delta p;
  * T2-8 SA plate Cf;
  * T2-9 ranking: 8 heated-cavity variants (Ra, aspect ratio), ranked by Nu at low against xhigh.
* **Negative control.** Each check also compares its tier against an xhigh run at a different Re or Ra, which must miss.
* **Where the runs happen (F-11).** The gate's per-test limit is 5 times the recorded time, clamped to 600-3600 s (ADR 0129 D1). Runs
  longer than that are run by hand and recorded in the addendum. The laminar step's fine mesh alone took 2166 s (MEASURED).

### T3: mixed precision on the CPU (opt-in)

| Id | Check | Limit | Negative control |
|---|---|---|---|
| T3-1 | `double` default | `baseline_hashes` unchanged; `--precision double` byte-identical | |
| T3-2 | T2 cases at medium, `mixed` against `double` | every quantity within 1e-4 relative (a hundredth of the smallest budget, 1 %) | an f32 pressure matrix without refinement misses on at least one case |
| T3-3 | pressure solve with f32 preconditioners, the 1M-unknown Poisson problem of [`docs/gpu.md`](../gpu.md) | reaches the f64 `tol_rel` 1e-8; iterations at most 1.3 times the f64 count | |
| T3-4 | iterative refinement on the 64^3 and 100^3 Poisson problems | true residual, computed in f64, at most 1e-10 relative | plain f32 CG stagnates above 1e-6 |
| T3-5 | `--threads 1` against `4` in `mixed` | byte-identical | |
| T3-6 | SpMV and whole-run time, 1M and 5M Poisson and the T2 cases | reported; sanity: `mixed` SpMV faster than `double` | |

### T4, T5, T6

* **T4 GPU f32 path.**
  * T4-1 index range guard (a located error above 2^31 - 1);
  * T4-2 f32 SpMV and preconditioner with refinement reach `tol_rel` 1e-8 on the 1M and 5M Poisson problems;
  * T4-3 every GPU test prints a located SKIP line without a device;
  * T4-4 speedup reported against the f64 device path and against CPU `mixed`.
* **T5 class F (blocked).**
  * Prerequisites:
    * a fan route that converges (FAN-01 stall, ADRs 0110-0117);
    * a transient sliding interface (F-250);
    * LES validated on a wall-bounded case;
    * distributed runs or device assembly for 1e7-1e8 cells: the runner's default budget refuses runs above about 1e6 cells;
    * measured computer-fan data (G2).
  * Checks to register then:
    * T5-1 ranking of 8 or more variants at medium, low and xlow against high;
    * T5-2 Table C's fan budgets against xhigh on two variants;
    * T5-3 every tier against measured data, reported apart from T5-2.
* **T6 ledger rule and correction.**
  * T6-1 `cross_tier_check` entries, and a `design_*` report of a chosen design without one is refused with a located message;
  * T6-2 a bias correction fitted on class S pairs reduces the held-out low-tier error (limits registered at T6 start);
  * T6-3 the design tools refuse Pareto sets with mixed tiers.

### What agents cannot check

* Agreement with physical reality. Tiers are measured against xhigh; xhigh against reality needs measured data (G2 for fans).
* GPUs other than the gate host's RTX 5080, and Linux bit-identity of the integration tests (CI runs unit tests only).
* xhigh fan runs, which take days and exceed this machine's memory, and anything that needs a cluster.
* How the Studio presents tiers: view-and-report by the user.
* Whether the budgets fit the decisions they serve: the maintainer's judgement (F-10).

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
   * A cheaper setting can converge where a dearer one does not: on FAN-01, `upwind` converged in 114 iterations and limited
     `linear_upwind` did not converge at all.
8. **Platform floats.** Bit-identity of every tier is per platform and build (D4). An expansion is platform-independent only because
   it uses no libm.
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
   * (a) medium is the case as written; other tiers are relative steps (numerical axis) and class presets (model axis).
   * (b) absolute presets per case class only.
   * (c) (a) now, plus absolute presets for a class once its xhigh is calibrated.

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
   (a). **Recommendation: (c).** (a) alone lets older builds mix tiers in Pareto queries.
8. **F-8, re-verification of a chosen design.** (a) at xhigh; (b) at high or above; (c) one tier up. **Recommendation: (b)**, with
   xhigh before a design is built or published.
9. **F-9, xhigh and high for the computer fan.**
   * (a) xhigh is wall-resolved LES of the full annulus with struts; high is SA-DDES of the full annulus.
   * (b) xhigh is DDES; high is URANS, which needs a transient SST that does not exist.
   * (c) until (a) exists, the highest available tier is labelled "xhigh-available" and no class F budget is claimed.

   **Recommendation: (a) as the definition, (c) meanwhile.**
10. **F-10, budgets.** (a) Table C, judged by the largest difference over the calibration set; (b) a quantile over a larger set;
    (c) other numbers. **Recommendation: (a)**: the sets are small, and a quantile needs more cases than exist.
11. **F-11, expensive calibration runs.** (a) in the gate; (b) by hand, recorded in the addendum; (c) (a) under the per-test limit
    and (b) above it. **Recommendation: (c).**
12. **F-12, placement.** (a) T1 now as its own track, in parallel with wave 49 (no solver change); (b) after wave 50.
    **Recommendation: (a)**, then T2, then T3.

## Proposed roadmap items (ids to be assigned by the maintainer)

* Fidelity setting, expansion, records and tools (T1).
* Tier calibration of class S (T2).
* Mixed-precision solves on CPU and GPU, with the GPU index guard (T3, T4).
* Cross-tier ledger rule and multi-fidelity correction (T6; joins part H7 of the fan breakdown).

## Files

This ADR only: `docs/adr/0136-fidelity-tiers-xhigh-to-xlow.md`. No code, test, default or baseline is changed.
