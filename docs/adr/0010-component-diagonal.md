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

(filled in after the runs)
