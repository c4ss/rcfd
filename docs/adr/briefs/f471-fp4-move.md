# Brief: F-471, the moved fingerprint FP4 of ADR 0374 N0 under the new default

Branch: f471-coarse-lu-default
Items: F-471
ADR: 0747
Rules version: 3
Model: Claude Fable 5.2
Issued: 2026-11-19
Size (lead): done; 0; 0; 1; 161175

The rules are `docs/process/agent-rules.md`, version 3, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f471-coarse-lu-default`, the existing branch of ADR 0743 at `5c8a18ff`; the work continues on it and nothing of it is rewritten. Scratch folder: `E:/projects/cfd/w49i-work/f471-coarse-lu-default-scratch`.

## Reading list

- docs/process/agent-rules.md 11903
- docs/adr/0743-f471-registration-the-corrected-order-of-the-coarsest-dense-lu-as-the-default.md 42957 (on the branch)
- docs/adr/0374-f363-registration-immersed-flux-with-one-piso-corrector.md 43988
- crates/rcfd-fv/tests/immersed_single_corrector_f363.rs 32478
- crates/rcfd-linalg/src/amg.rs 29849 (on the branch)

## The maintainer's answer

Asked by the lead on 2026-11-19 (question 1 of ADR 0743, addendum 1), as three options with a recommendation. The maintainer
answered on 2026-11-19: "Approve the move". That is option (a): the move of FP4 of ADR 0374 N0 under the corrected order of
the coarsest dense LU is approved, under a further registration that restates the test and runs the four long files before
anything merges. Option (c), an approval in advance of further fingerprints, was not given: any other registered hash or
fingerprint that moves stops the work again and is reported.

## Task

A further registration of F-471: `docs/adr/0747-f471-registration-<slug>.md`, the first commit of this work, alone.

- Append to ADR 0743, in the same commit as the registration or before it, a section "Answer of the maintainer (2026-11-19),
  written into this ADR by the lead's brief" with the paragraph above. Nothing else of ADR 0743 is changed.
- Register the diagnosis that addendum 1 of ADR 0743 gives as unregistered: under the old order, selected with
  `LinearOptions::amg_coarse_lu_interleaved`, the set-up of FP4 gives the registered `0x279c4c4ed4cf89be` bit for bit; under
  the new default it gives the new value; and which factorisation takes the other interchange. This is the evidence that
  the move comes from the approved change and from nothing else, and it becomes a running test.
- `n0_fingerprints_of_the_defaults` is retired with one `#[ignore = "..."]` line naming ADR 0747, its body untouched, and
  restated in a new test file: FP1 to FP3 with their registered values, FP4 with the new value, and FP4 under the old order
  with the old value.
- Run the four files ADR 0743 left out, under the new default, each once: `immersed_solid_f219`,
  `immersed_unsteady_force_f346`, `immersed_piso_stop_f355`, `vof_waves`. Two immersed tests of the full gate have a
  registered time limit (ADR 0740, `scripts/gate_limits.tsv`); run them in the background and report their times. A test of
  these files that fails, or a registered value that moves, is not edited: stop there and report it, with the same
  old-order evidence if you can give it.
- A3 of ADR 0743 is then repeated in full and reported.
