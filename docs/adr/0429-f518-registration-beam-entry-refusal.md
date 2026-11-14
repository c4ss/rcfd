# ADR 0429: F-518 registration: a check that observes the entry refusal of beams on the transformation before the transformation is built

Status: **REGISTERED 2026-11-14, written before any code of F-518 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run. No arithmetic beyond
reading the code was done beforehand (nothing was run).

Parents: [ADR 0426](0426-f513-registration-beams-on-the-transformation.md) (F-513: its D2 entry check
`mpc::check_transformation_model`, its mutation MU7 "`check_beam_model` not called on the transformation entry" not caught,
its remaining question 1 option (b), which this item takes), [ADR 0417](0417-f391-registration-six-unknowns-and-beam.md) (D5:
a beam node shared with a solid is refused with `FemError::BeamSolidNode`; D6: `assemble::stiffness_with` repeats the check).
Roadmap item F-518 (deps F-513), size XS, test-mostly. Branch `f518-beam-entry-refusal`, worktree `w49i-work/f518`, scratch
`w49i-work/f518-scratch`. House rules as in `w49i-work/AGENT_RULES.md`: limits fixed before the run, each registered run made
once, defaults bit-identical, no edit of a hash table or golden file, no new dependency, no unsafe code. No library change is
planned; if one were needed it would keep default behaviour identical.

## What F-518 asks

The roadmap acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. A model with a beam node shared with a solid and a
> contradictory linear constraint is refused with BeamSolidNode naming the node, not with the contradiction, on
> solve_static_mpc and modal_analysis_mpc; the mutation that leaves the beam check out of the transformation entry is caught.

Clauses: (c1) `solve_static_mpc` refuses such a model with `BeamSolidNode` naming the shared node, not with the contradiction;
(c2) `modal_analysis_mpc` the same; (c3) mutation MU7 of ADR 0426 is caught by a registered check; (c4) every limit registered
here before the run.

## What was read (no run)

`docs/adr/0426-*` (D2, S2, amendment 1, addendum 1, MU7 diagnosis, remaining question 1), `crates/rcfd-fem/src/mpc.rs`
(`check_transformation_model`, `transformation`, `Elimination::process`, `solve_static_mpc`, `modal_analysis_mpc`,
`refuse_nonzero_values`), `crates/rcfd-fem/src/beam.rs` (`check_beam_model`), `model.rs` (`check_supported_with_beams`,
`validate`), `crates/rcfd-fem/tests/si_beam_transformation.rs` (S2 builder).

## Derivation: which error each set-up produces, and where it is raised (by reading the code)

Order of events in `solve_static_mpc`: `model.validate()` (materials only), then `check_transformation_model` (which calls
`check_supported_with_beams`, hence `check_beam_model`), then `transformation(model, prescribed)`, then the load vector, then
`stiffness_with` (the repeated beam check, ADR 0417 D6), then the reduction. In `modal_analysis_mpc`: `validate`,
`check_transformation_model`, the zero-values-only loops over the prescribed rows and the `Linear` values (`zero_values_only`),
then `transformation`, then `stiffness_with`.

- **Static, contradictory rows.** Two `Constraint::Linear` rows on one unknown `u(node, 0)`: `u = 0` and `u = 1e-3`. In
  `Elimination::process` the second row has its designated unknown already dependent; after substitution `c` is empty and
  `b = 1e-3`, above `tau sb`, so it returns `InvalidValue { key: "constraint", .. }` whose reason contains "contradicts"
  (mpc.rs near line 483). This is raised inside `transformation`, which runs before `stiffness_with`. So if the entry check
  were missing, `solve_static_mpc` would return the contradiction and never reach the assembly check: the set-up
  discriminates. With the entry check present the error is `BeamSolidNode`.
- **Modal, the same two rows.** The contradiction itself is never reached on the modal path: after the entry check the
  zero-values loop refuses the row `u = 1e-3` with `InvalidValue { key: "constraint" }` whose reason contains "takes zero
  values only" (`zero_values_only`). So on `modal_analysis_mpc` the error that replaces `BeamSolidNode` when the entry check
  is missing is the zero-values refusal, which still precedes the assembly check: the set-up discriminates, but "the
  contradiction" is, on that path, the refused non-zero value. A homogeneous system `A x = 0` cannot be contradictory (`b = 0`
  always), so a contradiction that reaches the elimination cannot exist on the modal path; I register this and add a second
  modal set-up whose error comes from `transformation` itself: one `Linear` row `u(node, 0) - u(node, 0) = 0` (value 0),
  refused with "its coefficients cancel to zero" (in `transformation`, before assembly). Both set-ups are run.
- **Controls without the shared node.** The same constraints on the same model with the beam joined to nothing (its two nodes
  new, no node of a solid element) must give exactly the three errors above ("contradicts", "takes zero values only",
  "cancel to zero"). They show that the errors named are what the constraints produce on their own, so a `BeamSolidNode`
  result on the shared model is the entry check's doing and not a vacuous pass.

Both sorts of constraint act on the beam's free node, which has six unknowns (component 0 exists).

## Design

One new test file `crates/rcfd-fem/tests/si_beam_entry_refusal.rs` (existing tests not edited). Model of S2 of ADR 0426: a
`hex_box([0,0,0], [2,1,1], [2,1,1])`, nodes added, `xmin` held on the three translations, a rigid set on `xmax` with reference
`[2, 0.5, 0.5]`, steel on all elements; the beam block with section `BeamSection::new(0.012, 3e-6, 5e-6, 4e-6, 0.85, 0.75)`
and orientation `[0,0,1]`. Shared model: beam from a new node at `[3,0,0]` to the block node at `[2,0,0]` (`shared`). Control
model: beam between two new nodes `[3,0,0]` and `[4,0,0]`. Constraint set A = the two rows `u(free,0) = 0`, `u(free,0) = 1e-3`;
set B = the one cancelling row. Errors are compared by variant and by text.

No change to the library is planned. If the first run shows the entry refusal is not observed, that is a finding and is
recorded, not tuned away.

## Checks (limits exact: variant and text matches, no estimate is needed)

| # | Check | Limit |
|---|---|---|
| E1 | `solve_static_mpc`, shared model, set A | `Err(BeamSolidNode { node == shared })`, text contains `node {shared}`, not `InvalidValue` |
| E2 | `modal_analysis_mpc`, shared model, set A | same |
| E3 | `modal_analysis_mpc`, shared model, set B | same |
| E4 | `solve_static_mpc`, control model, set A | `InvalidValue { key: "constraint" }`, reason contains "contradicts" |
| E5 | `modal_analysis_mpc`, control model, set A | `InvalidValue { key: "constraint" }`, reason contains "takes zero values only" |
| E6 | `modal_analysis_mpc`, control model, set B | `InvalidValue { key: "constraint" }`, reason contains "cancel to zero" |
| E7 | `solve_static_mpc`, shared model, set B | `BeamSolidNode` naming `shared` (the order does not depend on the sort of constraint) |
| E8 | the `beam` and `solid` fields of the error of E1 equal those of `beam::check_beam_model(&shared model)` | exact equality |

Clause mapping: c1 = E1, E4, E7, E8; c2 = E2, E3, E5, E6; c3 = the mutations below; c4 = this table.

## Negative controls and planned mutations

Negative controls: E4 to E6. Mutations, run once each on a scratch `git archive` copy under `w49i-work/f518-scratch`, the new
test run in release after each; a mutation is "caught" if at least the named checks fail:

| Mutation | Change | Must be caught by |
|---|---|---|
| MU7 | the line `model.check_supported_with_beams()?;` removed from `check_transformation_model` (ADR 0426 MU7) | E1, E2, E3, E7, E8 |
| MU7a | `check_transformation_model(model)?;` removed from `solve_static_mpc` only | E1, E7, E8 |
| MU7b | `check_transformation_model(model)?;` removed from `modal_analysis_mpc` only | E2, E3 |
| MU7c | in `solve_static_mpc`, the entry check moved to just before `stiffness_with` (after the transformation) | E1, E7, E8 |

Pre-run expectation, from the derivation: MU7 and MU7a replace the E1 error by the contradiction; MU7b replaces E2 by the
zero-values refusal and E3 by "cancel to zero"; MU7c lets the transformation error come first. S2 of
`si_beam_transformation` stays green under all four (as ADR 0426 found for MU7); that is not a check of this item.

## Decision rule

**done** if E1 to E8 are met and MU7, MU7a, MU7b, MU7c are each caught by the checks named; **partial** if a registered check
missed, a mutation was not caught, or a check was met only after an amendment written after its run; a miss is kept as an
ignored `*_known_miss` test with the limit intact.

## Limits (what is not covered)

Beams on the other transformation users (transient, harmonic, buckling, geometric, rotating, cyclic) are not touched; one
shared-node geometry only (a beam end on a block node on a rigid-set face); `solve_static` and `modal_analysis` as wrappers run
the check before the dispatch and are covered by S2 of ADR 0426, not here. The contradiction on the modal path cannot be
reached (derivation above); the clause "not with the contradiction" is read there as "not with the error the constraint set
produces".

## Addendum 1: results (2026-11-14)

Run once in release (`cargo test -p rcfd-fem --release --test si_beam_entry_refusal`), library unchanged.

| Check | Limit | Measured | Met |
|---|---|---|---|
| E1 `solve_static_mpc`, shared, set A | BeamSolidNode naming the node | `BeamSolidNode { node: 2, beam: 2, solid: 1 }` | yes |
| E2 `modal_analysis_mpc`, shared, set A | same | same error | yes |
| E3 `modal_analysis_mpc`, shared, set B | same | same error | yes |
| E4 static control, set A | "contradicts" | "constraint 5 (Linear) contradicts constraint 4 (Linear): ... 0 = 1e-3" | yes |
| E5 modal control, set A | "takes zero values only" | "constraint 5 (Linear): value 1e-3; a modal analysis takes zero values only" | yes |
| E6 modal control, set B | "cancel to zero" | "constraint 4 (Linear): its coefficients cancel to zero" | yes |
| E7 static, shared, set B | BeamSolidNode naming the node | `BeamSolidNode { node: 2, beam: 2, solid: 1 }` | yes |
| E8 error fields equal `check_beam_model` | equal | (2, 2, 1) both | yes |

Mutations (scratch `git archive` copy of the test commit, one run each, results by check):

| Mutation | Missed checks (caught by) | Expected | Caught |
|---|---|---|---|
| MU7 (beam check out of `check_transformation_model`) | E1, E2, E3, E7, E8 | E1, E2, E3, E7, E8 | yes |
| MU7a (entry check out of `solve_static_mpc`) | E1, E7, E8 | E1, E7, E8 | yes |
| MU7b (entry check out of `modal_analysis_mpc`) | E2, E3 | E2, E3 | yes |
| MU7c (static entry check moved before `stiffness_with`) | E1, E7, E8 | E1, E7, E8 | yes |

The controls E4 to E6 passed on every mutant, as they should (no shared node). Deviations: the log of the first MU7 run
showed only the E8 line because of a filter error in my scratch script (the per-check status of E1 to E7 was not kept); the
mutation was run a second time with the script fixed, same mutant and same test, and the table above is from that run (the
first run also failed the test). No limit and no check was changed; the only edit after the test commit is rustfmt layout.
`cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --all-targets --release -- -D warnings` clean. No shared
solver file, hash table or golden file changed. Not run: the other `rcfd-fem` tests and the workspace suite (no library
line changed).

**Decision by the registered rule: done.** Every check met, every mutation caught by the checks named, no amendment.

### Remaining questions

1. On the modal path "contradictory" can only mean the refused non-zero value or a cancelling row (derivation): option (a)
   leave as registered; (b) none. Recommendation: (a).

## For the lead: roadmap entries

F-518: proposed status **done**. Note:

> Structural solver, intermediate tier. ADR 0429, 2026-11-14: a model with a beam node shared with a solid and a contradictory linear constraint is refused with BeamSolidNode naming the node, not with the contradiction, on solve_static_mpc (set of two rows u = 0 and u = 1e-3) and on modal_analysis_mpc (the same rows, which would give the zero-values refusal, and a cancelling row); controls without the shared node give the contradiction, the zero-values refusal and the cancel-to-zero error; the error fields equal those of check_beam_model; the mutation that leaves the beam check out of the transformation entry (ADR 0426 MU7) and three variants (out of either entry, moved after the transformation) are all caught. Library unchanged.

F-513: its note may add "MU7 caught by ADR 0429, 2026-11-14".
