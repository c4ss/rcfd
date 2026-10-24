# `solver.component_diagonal = "corrected"` as the default: what would change

Analysis for a decision that is the user's (ADR 0074 part C, wave 34C). Nothing here changes the default; `averaged` stays the default and no
baseline hash was re-recorded. All numbers below were measured in wave 34C on the tree at HEAD 1f571fb plus the wave-34C changes; the harness is
`crates/rcfd-api/tests/w34c_component_diagonal_default.rs` (ignored diagnostics, C1 and C2) and one full workspace test run of a scratch copy whose
schema default was flipped to `corrected` (never committed).

## What the key does

The segregated pressure-based kernels build the pressure equation and `HbyA = H/a_P` with `a_P`, the mean of the diagonals of the velocity-component
matrices. Where a condition acts on one component only (a `symmetry` plane puts a Dirichlet term on the normal component; `shear`, `partial_slip`,
`marangoni` do the same), the component diagonals differ in the adjacent cell, and the converged tangential velocity then satisfies
`a_P u_t = H_t + V (f - grad p)_t` instead of `a_t u_t = ...`: an extra sink `(a_P - a_t) u_t` (with under-relaxation, of the relaxed diagonals) that
acts like a partial no-slip wall. `corrected` adds `(a_P - a_k) u_k` to `H_k` (ADR 0010), which removes it exactly.

| kernel | with `corrected` | default today |
|--------|------------------|---------------|
| steady SIMPLE/SIMPLEC (`simple`, `simplec`) | correction in every cell whose component diagonals are not bit-equal; bit-identical elsewhere | averaged |
| transient PISO/PIMPLE constant density (`piso`, `pimple`) | the key reaches this kernel since ADR 0074 W1 (before: silently ignored although documented); the split is applied in every cell, without the bit-equality guard of the steady kernel | averaged |
| variable density (`vd.rs`) | `VdSettings.component_diag_correction` | averaged |
| MRF zones | always corrected, whatever the key | corrected |
| VOF (`multiphase.enabled`) | not affected by this key; its own key `multiphase.component_diagonal` (library `VofSettings::component_diagonal`, ADR 0103) is `corrected` by default since ADR 0114 | corrected |
| coupled (block) solver, compressible density-based | not affected by the key (not examined further) | n/a |

## Why it matters: exact solutions (ADR 0074 part A)

| case | averaged (default) | corrected |
|------|--------------------|-----------|
| laminar half-channel Poiseuille, `Symmetry` top, 4 x 40 | u_max 12.75 / 15.57 (h_from_solution off / on) against 50; wall shear -49.5 / -44.2 percent | derived discrete solution to 1e-10, wall shear exact to 7e-11 |
| graded half channel against the mirrored full channel | not run | equal to 2e-11 of u_max |
| shear-free plug (moving wall, `Symmetry` top), exact u = U_w | error 81 percent | 1e-13 |
| plane Couette with `Symmetry` side planes (3-D) | error 8.1e-2 U, spurious cross-flow 7e-4 U | 6e-13 |
| transient plug started at the exact solution, 40 PISO steps | drifts to an error of 0.79 U | 2e-16 |
| packed bed with PISO (dt 0.01, 100 steps), exact u_y = 0 | max abs(u_y) 8.4e-7 | 4.2e-14 |
| SST half channel Re_tau 395 against the mirrored full channel | bulk velocity -73 percent, the wall carries only 24 percent of the driving force | equal to the printed digits, force balance 1 + 3.5e-8 |
| SST Re_tau 1e6 half channel (ADR 0070 P3, Karman measure against NASA TMR) | KM 1.07 to 1.91 against 0.37 to 0.41 (ADR 0070) | KM within 0.01 of TMR, identical to the full channel |
| SST flat plate with a `Symmetry` run-up upstream of the leading edge (wall-resolved cells on the symmetry plane) | not converged in 14000 iterations, Cf at x = 0.97 12 to 13 percent low | converges in 594 / 1863 iterations |
| SST flat plate, `Symmetry` top only (ADR 0070 meshes) | reference | Cf(x = 0.97) changes by -1e-6 (coarse) and < 1e-6 (medium) relative |

The defect matters where the cells next to a symmetry-type plane are diffusion-dominated (wall-resolved meshes touching a symmetry plane, half channels,
run-ups, low-Re flows, Stokes-like regions); it is negligible on a coarse free-stream symmetry top at high Reynolds number.

## Baseline hashes (`crates/rcfd-api/tests/baseline_hashes.rs`), measured with the key injected (C1)

| case | registered (steps, hash) | with `corrected` | moves? |
|------|--------------------------|------------------|--------|
| cavity-simple | (60, 07e7461c36893384) | same | no |
| cavity-simplec | (60, 58d5c4fd29432b05) | same | no |
| cavity-coupled | (59, 123b5330dae3f36a) | same | no |
| cavity-piso | (30, 2a042a42594c418b) | same (the transient split is now active; 2-D walls only, bit-identical) | no |
| cavity-pimple | (30, 2a042a42594c418b) | same | no |
| heated-simple | (60, 30a23d946786fd8a) | same | no |
| heated-simplec | (60, 7d35b1f2f7fcc4b7) | same | no |
| heated-piso | (40, fe7b5aa0a252ec0f) | same | no |
| packed_bed | (60, 19e53df373637262) | (16, fb46cdfee5d0d781) | YES |
| sod | (60, e1612365139bf898) | same (density-based, key not used) | no |
| wedge | (60, e198a93b898bd79b) | same | no |
| kepsilon_channel | (60, 6ddc55969af2a835) | same | no |
| rotating_channel (ignored test) | (60, 54d9bee17b5895e8) registered; today (60, dd45d309f7acc28a) with either setting | same as today | no (the registered value is already stale since ADR 0005, unrelated) |

Exactly one registered hash moves: `packed_bed`.

## Shipped cases (`cases/*/*.toml`), averaged against corrected at the same budget (2000 iterations or the case's own limit), C2

| case | iterations averaged / corrected | converged averaged / corrected | largest change (max abs, relative to the field's max) |
|------|---------------------------------|--------------------------------|-------------------------------------------------------|
| packed_bed (symmetry side walls) | 1700 (cap) / 16 | no / yes | u_x 2.0e-6 (1.3e-5), u_y 9.6e-7 (exact 0; the corrected value is 1e-13 level), p 1.0e-3 (6.8e-6) |
| axial_fan_cgrid, axial_fan_sector | 358 / 358, 356 / 356 | yes / yes | bit-identical |
| channel: kepsilon_channel, kepsilon_inlet, profile_inlet, sst_channel | 2000 / 2000, 232 / 232, 172 / 172, 2000 / 2000 | as averaged | bit-identical |
| rotating_channel | 875 / 875 | yes / yes | bit-identical |
| sod (density-based) | 172 / 172 | as averaged | bit-identical |
| wedge | 1122 / 1122 | as averaged | bit-identical |

`cases/*/case.json` are benchmark-registry descriptors (`rcfd-io::cases`), not solver cases; they carry no solver settings and are not affected.

## Test suite with the default flipped

One `cargo test --workspace --release --no-fail-fast` of a scratch copy (this wave's tree with only the schema default flipped to `corrected`):
1777 passed, 6 failed, 422 ignored. The six failures:

| test | why it fails with a `corrected` default | kind |
|------|------------------------------------------|------|
| `baseline_hashes::shipped_cases_match_registered_hashes` | `packed_bed` (60, 19e53df373637262) becomes (16, fb46cdfee5d0d781) | registered hash moves (would need an explicit re-registration) |
| `component_diagonal::averaged_is_the_default_and_accepted_explicitly` | asserts that the default equals `averaged` | true by definition today |
| `w34c_component_diagonal_transient::w1_component_diagonal_key_reaches_the_transient_kernel` | W1a asserts that the default equals `averaged` | true by definition today |
| `duct_diagnosis::a6_default_deviation_is_located_in_the_wall_cells` | asserts that the default run shows its deviation in the wall cells (it documents the averaged-diagonal defect recorded in ADR 0022/0033); with `corrected` the wall-cell deviation is 4.4e-13 and the interior one 1.6e-12, i.e. the defect is gone | test written for the averaged behaviour |
| `lumped::z4_heat_exchanger_energy_balance` and `lumped::z4_effectiveness_duty_follows_reported_inlet_temperature` | the heat-exchanger channel (`channel_2d` 100 x 2, `symmetry` on both walls, SIMPLEC, `h_operator = "from_solution"`, a `heat_exchanger` component) is not stationary after 1500 iterations (p changes by 0.18 of its maximum over the next 300). A probe of the fixed-duty case: `averaged` is stationary to 4e-10 from iteration 1000 on; `corrected` keeps changing p by 3 to 26 percent per 500 iterations up to 10000 iterations | a genuine convergence regression of `corrected` on this case; cause not investigated |

All other tests pass with the flipped default (the other lumped fan and porous-jump channels with the same symmetry walls, the duct, MRF, rotating-frame,
slip-wall and packed-bed tests included).

## What was not measured

* Numbers that change inside tests that still pass (only outcomes were compared); ignored heavy tests were not run with the flipped default.
* Library defaults (`SimpleSolver::new`, `PimpleSettings`, `VdSettings`) are not part of the case key; fv-level and turbulence-crate tests set them directly
  and do not move with the key.
* User cases outside the repository: any case with `symmetry`, `shear`, `partial_slip` or `marangoni` velocity boundaries and a segregated algorithm would
  change; how much depends on how diffusion-dominated the adjacent cells are (from 1e-6 on the high-Re plate top to O(1) in a laminar half channel).
* Convergence cost: the corrected iteration is not uniformly faster. In the laminar half channel it converged to the right answer in 60302 iterations
  (h_from_solution on) and 220481 (off), where the averaged iteration stopped earlier (6636 / 21284) at a wrong answer; on the packed bed it is
  faster (16 against a stall at 1700).

## Options

1. Keep `averaged` as the default (today): no hash moves; users must know to set `corrected` with symmetry-type boundaries; the failure is silent.
2. Make `corrected` the default: one registered baseline (`packed_bed`) moves and must be re-registered by an explicit decision; every shipped case other
   than `packed_bed` is bit-identical; three tests that assert today's default or its defect need rewording; the lumped heat-exchanger channel stops
   converging and would need a diagnosis (or the key set to `averaged` in that case) first.
3. Keep `averaged` but warn (a run-time diagnostic) when a segregated kernel meets a symmetry-type boundary with the averaged diagonal (not implemented).

## Update 2026-10-19 (wave 35B, ADR 0077 part D): the heat-exchanger channel is not a convergence regression

The `lumped` z4 channel has the exact solution u = (2, 0, 0) m/s, p = 0 (both walls `symmetry`, uniform inlet, a heat exchanger without pressure
loss, no buoyancy, constant properties). Measured with `crates/rcfd-api/tests/w35b_hx_corrected.rs` (the three z4 set-ups, 10 000 iterations):

| | `averaged` (default) | `corrected` |
|---|---|---|
| max abs(ux - 2) / U at iteration 1 500 | 9.4e-6 | 1.9e-13 (1.2e-13 to 1.5e-13 at 5 000 and 10 000) |
| max abs(p) / (U^2 / 2) at iteration 1 500 | 3.6e-4 | 7.5e-14 (3.2e-14 at 10 000) |
| change of ux / U, of p / (U^2 / 2) over 500 iterations | - | at most 2.5e-13, 1.3e-13 |
| ADR 0012 energy balance (enthalpy change vs duty, 1e-6; reported duty 1e-9; effectiveness rise vs duty 1e-6; duty vs eps C_min (T_sec - T_in) 1e-9) | met (ADR 0012) | met: 1.7e-12, 0, 5.6e-13 / 1.3e-12, 1.1e-15 / 0 |

`corrected` reaches the exact plug flow to round-off and stays there; `averaged` is stationary on a solution that is off by the symmetry-plane sink of ADR 0010.
The "not stationary" verdict of the table above comes from the `run()` helper of `lumped.rs`, which divides the change of p by `max(max|p|, 1e-12)`: with
`corrected` max|p| is about 6e-14 and the change over 300 iterations 1.8e-13, so the ratio reads 0.18 (reproduced: 0.177). It is a relative criterion applied
to a round-off pressure field, not a solver defect. Option 2 above therefore needs no diagnosis of this channel; the two z4 tests would need an absolute
(dynamic-pressure scaled) stationarity criterion, or `averaged` set explicitly, if the default were changed. The default is unchanged (the user's decision).
