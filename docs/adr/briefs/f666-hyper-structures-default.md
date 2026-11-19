# Brief: F-666, the refusal of hyperelastic materials on structural elements lifted by default

Branch: f666-hyper-structures-default
Items: F-666
ADR: 0746
Rules version: 3
Model: Claude Fable 5.2
Issued: 2026-11-19
Size (lead): done; 0; 0; 0; 172312

The rules are `docs/process/agent-rules.md`, version 3, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f666-hyper-structures-default` (the lead creates it from main). Scratch folder: `E:/projects/cfd/w49i-work/f666-hyper-structures-default-scratch`.

## Reading list

- docs/process/agent-rules.md 11903
- docs/adr/0593-f666-registration-hyperelastic-structural-elements-on-the-cyclic-rotating-cms-and-mpc-paths.md 31741
- docs/adr/0744-f672-registration-beam2-in-the-cyclic-constraint-by-default.md 35010
- docs/adr/0555-f620-registration-hyperelastic-materials-on-the-method-step-and-the-linear-paths.md 48454
- crates/rcfd-fem/tests/sa_hyper_structures_f666.rs 32879
- crates/rcfd-fem/src/mat_hyper.rs 12325

## Task

A further registration of F-666: `docs/adr/0746-f666-registration-<slug>.md`. The maintainer approved the default change on
2026-11-18 (written into ADR 0593, "Answers of the maintainer"): the refusal of ADR 0555 D3, hyperelastic materials on
structural elements (Beam2, Shell4, Shell3) on the linear paths, is lifted by default, with RF1 retired as a record and a
copy that asserts acceptance.

- The registration lists the changed results case by case and the records it re-records, before the code: each test that
  asserts the refusal without the setting (RF1 of ADR 0555, T2 of ADR 0593, H-D of ADR 0744 in
  `sa_cyclic_shells_refusals_adr0744.rs`, and any other you find by reading; name every one), what it becomes, and what
  still refuses a hyperelastic material afterwards. Say what the setting `hyperelastic_structures` is after the lift (a
  documented no-op is the lead's expectation; register what you choose and why).
- A test that asserts the old refusal is retired with one `#[ignore = "..."]` line naming ADR 0746, its body untouched,
  and restated in a new test file that asserts acceptance and the result of the isotropic twin of the same small-strain
  moduli within the 1e-10 of the item's acceptance. A-B of ADR 0593 is already an ignored record since ADR 0744; leave it.
- Every model without a hyperelastic material on a structural element keeps its bits: show it with the existing bit records
  of the paths you touch and `baseline_hashes`. A changed registered hash is not re-recorded: stop and report it.
- Rule 14: the kind of every row of the table of checks is one of the four, a qualifier after it in round brackets.
- `docs/structural-solver.md` carries one line on this lift ("approved, not yet made (F-666)"); bring it up to date.
