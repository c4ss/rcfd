# ADR 0010: Component-consistent diagonal in the pressure-based kernels

Status: accepted. The checks, decision rule and numeric limits below were written before anything was run. They are
not changed after a run; a miss is reported as measured.
Roadmap: F-031 (MRF, ADR 0008 follow-up), F-050 (transient variable density, ADR 0009), F-029-class accuracy items.

## Problem

The SIMPLE/SIMPLEC kernels (`rcfd-fv::simple`, `rcfd-fv::varden`) build the pressure equation and the H operator
with `a_P`, the mean of the component diagonals. A symmetry plane puts its Dirichlet term on the normal component
only, so in the cell next to it the component diagonals differ. The converged tangential velocity then carries the
factor `a_k / a_P < 1` (ADR 0008: spurious end-wall drag in Taylor-Couette; ADR 0009: BDF2 order 1.34 on a
one-cell-high strip against 2.07 on a taller one). `SimpleSolver::component_diag_correction` adds
`(a_P - a_k) u_k` to the H operator of component `k` (as OpenFOAM's `fvMatrix::H` does) and has been enabled only for
MRF cases so far.

## Decision rule

1. SIMPLE kernel. Enable the correction for every case. It is a bitwise no-op wherever the component diagonals are
   bit-equal (walls only, 2-D, pure inflow/outflow), so only cases with symmetry planes or mixed component BCs can
   change. Run `baseline_hashes` and the whole workspace test suite. If every registered test passes and the
   baseline hashes are unchanged, the correction becomes the default. If any hash or registered test changes, the
   old behaviour stays the default and the correction is selected by `solver.component_diagonal = "corrected"`
   (`"averaged"` default); the differing cases and the size of the difference are reported, hashes are not re-recorded.
2. Variable-density kernel (`varden.rs`). Same correction in the momentum/H assembly, selected by the same option
   (`VdSettings.component_diag_correction`), default off. Steady ideal-gas and real-gas results (`varden_ideal_hash`,
   `baseline_hashes`, every `varden_*` test) must be bit-identical with the option off (check V0).
3. Nothing about a registered criterion is changed: limits are asserted as registered, tests are moved between
   `#[ignore]` and active only when the unchanged assertion decides it.

## Checks and numeric limits (all existing, unchanged)

* V0: option off: `baseline_hashes` (3 pass, 1 ignored as now), `varden_ideal_hash` and all `varden_*` tests unchanged.
* V1: option on, constant-density: every non-ignored test in `rcfd-fv` and `rcfd-api` still passes with its registered
  limits; cases whose result changes are listed with the change of the reported quantity.
* V2: BDF2 order of the refined acoustic run, `t3c_order_bdf2`, limits `[1.80, 2.20]` (measured before: 1.34), run
  unchanged with the correction on. Pass: drop the `known_miss` suffix and the ignore. Fail: report the number.
  Euler order `[0.85, 1.15]` must still hold.
* V3: T5 transient nozzle known misses (`t5_transient_steady_limit_mass_flow_known_miss`) rerun with the correction
  on only if the nozzle has a symmetry plane; the 400-step budget and the limits are unchanged.
* V4: with the SIMPLE option on, the MRF ignored tests `m3_stationary_frame_without_source_known_miss` (velocity
  1.5e-2 V_i, spurious 1e-3 V_i, pairwise 1e-2 V_i), `m3_spurious_speed_known_miss`,
  `m3b_spurious_speed_open_ends_known_miss`, `m5b_interface_conservation_open_ends_known_miss` are rerun with their
  registered limits and the new numbers reported.
* V5: against existing analytic/benchmark references (Poiseuille, Couette, cavity vs Ghia): a changed case counts as
  more accurate if its error against the reference in the existing test does not increase.
* Hygiene: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`.

## Results

Decision: option, not default. With the correction enabled for every case `baseline_hashes::shipped_cases_match_registered_hashes`
fails: `packed_bed` (symmetry side walls) changes from (60 iterations, 19e53df373637262) to (16, fb46cdfee5d0d781), i.e. it
now converges instead of stalling at the 1700-iteration cap. Compared at convergence (200 allowed iterations, `solver.component_diagonal`
averaged / corrected): iterations 1700 / 16, max |ux - 0.15| 6.41e-2 / 6.41e-2 (identical), max |uy| (exact 0) 9.56e-7 / 4.71e-7, pressure
range 145.6168 / 145.6158. `wedge`, `kepsilon_channel`, `rotating_channel` (60 and up to 2000 iterations) are bit-identical with the
option on (no component-specific condition). Hashes were not re-recorded; the default is `averaged`, `solver.component_diagonal = "corrected"`
selects the correction (also in the variable-density kernel via `VdSettings.component_diag_correction`).

* V0: pass. `baseline_hashes` 3 passed / 1 ignored, `varden_ideal_hash` and all `varden_*` and rcfd-fv tests pass with the
  option off (nothing changes when it is off: the new work sits in `if component_diag_correction` branches).
* V1: the cases whose result changes are listed above (only `packed_bed` among the shipped ones). The whole workspace test suite passes
  with the new defaults, except `rcfd-cli run_control::journal_runs_the_recorded_case` (cells 108 against 81), which comes from
  the concurrent mesh-generator change of the other agent (`cavity_2d` now honours `ny`), not from this work.
* V2: `t3c_order_bdf2` with the correction on: order 2.08 (errors 1.053e-2, 2.372e-3, 5.60e-4; the first pair gives 2.15), registered
  [1.80, 2.20]: PASS. The `known_miss` suffix and the ignore are dropped from the test (it now runs with the option on); the averaged
  diagonal run (1.34) is kept as the ignored `t3c_order_bdf2_averaged_diagonal_known_miss`. Euler order 0.929 with the correction (0.85-1.15: pass).
* V3: T5 (nozzle strip with symmetry planes, correction in the steady reference and in the transient run, 400 steps): Mach 3.0e-4 (2e-3) pass,
  p 1.1e-4 (5e-4) pass, mass flow in/out versus steady 9.3e-4 / 9.2e-4 (1e-3) PASS (1.64e-3 / 1.62e-3 before), in/out equality 1.56e-5
  (1e-6) still MISS. Kept as ignored tests `t5_transient_steady_limit_corrected_diagonal_*`.
* V4: with the option on, M3(a) without source: velocity 6.76e-3 V_i (1.5e-2) pass (3.06e-2 before), pairwise 4.3e-4 / 8.8e-4 / 1.7e-3 V_i (1e-2)
  pass (2.57e-2 before), torque -3.41 % (report-consistent target, unchanged miss of the registered full-stress target), spurious
  1.41e-3 V_i (1e-3) MISS (1.39e-3 before). MRF-based known misses are unchanged because MRF zones already used the correction: M3
  spurious 1.41e-3 (frame Omega 0 with an empty zone), M3b spurious 8.6e-3, M5b net flux 3.15e-5 V_i A and sum |flux| 1.0003e-3.
* V5: Packed bed (exact solution uniform flow): see above, not less accurate. Poiseuille/Couette/cavity cases have no
  component-specific conditions and are bit-identical.

