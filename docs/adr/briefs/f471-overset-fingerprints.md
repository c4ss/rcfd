# Brief: F-471, the nine moved fingerprints of the overset solver under the new default

Branch: f471-overset-fingerprints
Items: F-471
ADR: 0748
Rules version: 3
Model: Claude Fable 5.2
Issued: 2026-11-20
Size (lead): done; 0; 0; 9; 473792

The rules are `docs/process/agent-rules.md`, version 3, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f471-overset-fingerprints`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f471-overset-fingerprints-scratch`.

## Reading list

- docs/process/agent-rules.md 11903
- docs/adr/0743-f471-registration-the-corrected-order-of-the-coarsest-dense-lu-as-the-default.md 43679
- docs/adr/0747-f471-registration-the-moved-fingerprint-fp4-of-adr-0374-under-the-new-default.md 31071
- crates/rcfd-fv/src/overset.rs 111439
- crates/rcfd-linalg/src/amg.rs 29849
- crates/rcfd-fv/tests/immersed_single_corrector_fp_adr0747.rs 16346 (the form of a restated fingerprint test)
- crates/rcfd-fv/tests/overset_f487.rs 39613
- crates/rcfd-fv/tests/overset_f472.rs 28126
- crates/rcfd-fv/tests/overset_f335_records.rs 10364
- crates/rcfd-fv/tests/overset_motion_f442.rs 26556
- crates/rcfd-fv/tests/overset_f338.rs 41069
- crates/rcfd-fv/tests/overset_f450.rs 19638
- crates/rcfd-fv/tests/overset_f368.rs 31033
- crates/rcfd-fv/tests/overset_f479.rs 24720
- crates/rcfd-workspace/tests/shipped_store_f834.rs 8386

Outside the repository, for reading: the logs and the attribution of the failed gate in
`E:/projects/cfd/w49i-work/gate-252c9279` (`test.log`, `times.tsv`, `attribution.md`).

## What happened

The full gate of main at `252c9279` (run 44599067147, 2026-11-20) is red: 5908 passed, 10 failed, 1714 ignored; all ten
fail again on re-run. Nine are tests of the overset solver that assert a registered fingerprint or record of a default
result, and the attribution names the merge of F-471 (`075d8fc8`) as their only candidate:

- `overset_f487::d1c_l_fingerprint_equals_the_base_tree`
- `overset_f472::d1b_opt_in_fingerprint_equals_the_base_tree`
- `overset_f335_records::r2_prime_opted_in_with_one_outer_iteration_gives_the_records`
- `overset_f335_records::r2b_taylor_green_level_1_record`
- `overset_motion_f442::f0_defaults_fingerprint_equals_the_base_tree`
- `overset_f338::r2e_implicit_euler_chosen_by_the_method_gives_the_records`
- `overset_f450::b1_defaults_bit_identical`
- `overset_f368::d1_defaults_fingerprint_equals_the_base_tree`
- `overset_f479::a6_without_the_opt_in_every_bit_is_kept`

ADR 0743 D2 gave `overset.rs` the corrected order with no way back and stated "No record of the old behaviour runs through
them". For the overset solver that statement is false, and no overset file was in the list of A3 or in the merge's push
run. The tenth failure is `shipped_store_f834::d1_the_store_of_every_non_adaptive_shipped_case_is_unchanged`: the first
hash of `cases/axial_fan_cgrid/axial_fan_cgrid.toml` moved from `0xe453d1b2ca2a6c06` (16452657040940017478) to
`0x03d18f072e9caca5`; the other nine cases are unchanged. Its cause is not diagnosed; the attribution's candidates are the
merges of F-848, F-262, F-271, F-672, F-666 and F-471.

## The maintainer's answer

Asked by the lead on 2026-11-20 as three options with a recommendation (approve the moves; keep the overset solver on the
old order; revert the default). The maintainer answered on 2026-11-20: "Approve the moves". That is: the moves of the nine
registered values listed above under the corrected order of the coarsest dense LU are approved, under a further
registration that adds a way back to the old order for the overset solver, shows each old value bit for bit under it, and
restates the nine tests with the new values, as ADR 0747 did for FP4. Nothing else is approved: any other registered hash,
fingerprint or record that moves stops the work and is reported.

## Task

A further registration of F-471 (rule 13: under the parent item): `docs/adr/0748-f471-registration-<slug>.md`, the first
commit of this work, alone.

- Append to ADR 0743 and to ADR 0747, in the same commit as the registration or before it, a section "Answer of the
  maintainer (2026-11-20), written into this ADR by the lead's brief" with the paragraph above, and in ADR 0743 one
  sentence that the quoted statement of D2 is false for the overset solver and is corrected by ADR 0748. Nothing else of
  either ADR is changed.
- Register and build a way back for the overset solver: its AMG builds take their settings from the solver's linear
  options (the form D2 of ADR 0743 gave `pressure.rs`), so that `amg_coarse_lu_interleaved = true` gives the old order.
  The default stays the corrected order. No key of the case schema and no golden changes. Decide and register whether the
  coupled, potential and variable-density solvers get the same way back now; the gate shows no moved record through them,
  so leaving them is acceptable if the registration says so.
- For each of the nine tests: under the old order, selected by the way back, the set-up gives the registered old value bit
  for bit; under the default it gives the new value. This is the evidence that each move comes from the approved change
  and from nothing else, and it becomes a running test. Where a test asserts "equals the base tree" or "bit identical"
  against a second computation and not against a stored value, register what the two sides are and why they now differ,
  before changing anything; if the honest reading is that both sides should still agree, that is a defect of the way the
  default reaches one side, and it is fixed and not restated.
- Each of the nine tests is retired with one `#[ignore = "..."]` line naming ADR 0748, its body untouched, and restated in
  a new test file with the new value and with the old value under the old order. No other test is ignored, skipped,
  removed or edited, and no numeric limit of any ADR changes.
- Run every `overset*` test file of `rcfd-fv` (21 files), unedited except for the nine attributes, in release, and report
  each. Run in the background anything that takes longer than a few minutes.
- `shipped_store_f834` D1: diagnose which commit moved the hash of `axial_fan_cgrid` and why (the file names how its two
  hashes are formed; a bisect over the six candidate merges with this one test is cheap). Do not edit the test or its
  table. Report the cause with its evidence. If the cause is F-471, give the old value under the old order as for the
  nine; the lead brings it to the maintainer. If the cause is another item, say which and stop there.
- Mutations: at least one that the old-order evidence must catch (the way back ignored in the overset solver) and one that
  the restated values must catch, each with its registered catcher.

Decision rule to register: done only if all nine have the old value bit for bit under the old order and the new value
under the default, every overset file passes, and the mutations are caught; partial if any row is missed or not run;
stopped and reported if any registered value other than the nine moves.

The lead merges with `scripts/push_main.py`; the lead's push run will include the 21 overset files and
`shipped_store_f834`. Do not push and do not merge. Commit messages are plain: no agent marker, no trailers.

## Report (under 450 words)

The registration's commit and the code commits; the table of the nine (old value, value under the old order, new value);
the result of each overset file; the cause of the `axial_fan_cgrid` hash with its evidence; mutations; deviations and
amendments, each with the commit that precedes its corrected run; what was not run; your verdict by the registered rule.
