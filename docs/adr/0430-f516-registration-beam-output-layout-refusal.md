# ADR 0430: F-516 registration: beam output refuses a result whose count of unknowns per node differs from the model's, naming the node and both counts

Status: **REGISTERED 2026-11-14, written before any code of F-516 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-516 was written, compiled or run: the files listed under "What was read" were read;
no arithmetic beyond counting the unknowns of the fixture of ADR 0423 was done. Results go in the addendum at the end; nothing
above the addendum is changed after a run.

Parent: [ADR 0423](0423-f512-registration-beam-results-in-solid-output.md) (F-512: `beam_fields`, check N2b, mutation MU7 not
caught, remaining question 2 on the refusal text). Roadmap item F-516 (deps F-512). Size XS. Number 0430 given by the lead.
Branch `f516-beam-output-layout-refusal`, worktree `w49i-work/f516`, scratch `w49i-work/f516-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; each registered run made once;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the hash tables or of a golden file; no new dependency, no unsafe
code; clean-room.

## What F-516 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. rcfd_post::solid_output refuses a result of a beam model whose DofMap has the model's number of unknowns but another count at some node, with a message naming that node and both counts; the N2b test of ADR 0423 passes unedited; the mutation removing the DofMap equality check is caught by a check registered before its run.

Clauses: (c1) the refusal, with a message naming the node and both counts; (c2) the N2b test of ADR 0423 passes unedited; (c3)
the mutation removing the `DofMap` equality check is caught by a check registered here, before its run; (c4) every limit not
stated in the acceptance is registered before the run (this document). Added by the brief and ADR 0423: (c5) the output of a
model without beams stays byte-identical (the F-512 B1 test, unedited).

## What was read

- `docs/adr/0423-*` (D2 the order of refusals in `beam_fields`; N2, N2b, MU7; Addendum 1; remaining question 2: the refusal of
  a result with another layout reads `length 27, expected 27`, because the `FieldLength` error is reused).
- `crates/rcfd-post/src/solid_output.rs`: `beam_fields` refuses with `length_error("displacement", ...)` when
  `*result.dofs() != dofs || result.displacement_values().len() != dofs.n_equations()`. The `||` mixes two conditions: a wrong
  length (the message is right: two different numbers) and a wrong layout with the same length (the message prints the same
  number twice). `solid_fields` (the solid path) is not changed.
- `crates/rcfd-fem/src/model.rs`: `DofMap::n_nodes`, `n_equations`, `components(node)` (0 for a node that does not exist),
  `PartialEq`.
- `crates/rcfd-post/tests/si_beam_output.rs`: fixture `frame_model` (5 nodes, model counts `[6, 6, 6, 6, 3]`, 27 unknowns), N2
  (`DofMap::uniform(5, 3)`), N2b (`DofMap::from_counts([6, 6, 6, 3, 6])`, 27 unknowns). These tests are not edited.

## Design

### D1. Order of refusals in `beam_fields` after the element-kind refusals

Let `model_dofs = model.dof_map()` and `r = result.dofs()`.

1. If `result.displacement_values().len() != model_dofs.n_equations()`: the existing `length_error("displacement", ...)`
   (unchanged text: a wrong number of unknowns, two different numbers in the message). This keeps N2 as it is.
2. Else, if `r.n_nodes() != model_dofs.n_nodes()`: `InvalidValue { path: "displacement", reason }` with
   `reason = "the result numbers {r_nodes} nodes and the model {m_nodes}; ..."`.
3. Else, the first node `k` (smallest index) with `r.components(k) != model_dofs.components(k)`, if any:
   `InvalidValue { path: "displacement", reason }` with
   `reason = "node {k} has {r_count} unknowns in the result and {m_count} in the model; ..."` (the rest of the sentence says
   the result has the model's number of unknowns in another layout).
4. Else the layouts are equal (offsets are determined by the counts, so equal counts mean `*r == model_dofs`): proceed as before.

Only the beam path changes; `solid_fields`, `solid_vtu_string` for a model without Beam2 and every other function are not
touched, so their bytes do not change (c5). The check of step 3 is the `DofMap` equality check of ADR 0423 D2 written per node;
the mutation MU7 removes it.

## Checks (limits fixed now)

Fixture F: `frame_model()` of `si_beam_output.rs` (copied into the new test file `crates/rcfd-post/tests/si_beam_layout.rs`;
the existing test file is not edited). Model counts `[6, 6, 6, 6, 3]`, 27 unknowns. A result is
`SolidResult::new(dofs, vec![0.0; n_equations])`. A refusal is read from `solid_vtu_string(&model, &r)` and from
`beam_fields(&model, &r)`; both must refuse with the same text.

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| C1 | c1 | F with counts `[6, 6, 6, 3, 6]` (the N2b result): the message contains exactly `node 3 has 3 unknowns in the result and 6 in the model` and `displacement`, and does not contain `expected 27` | exact (substring) | D1 step 3: nodes 3 and 4 differ, the first is 3, result count 3, model count 6 |
| C2 | c1 | F with counts `[3, 6, 6, 6, 6]` (27 unknowns): the message contains exactly `node 0 has 3 unknowns in the result and 6 in the model` | exact | the first differing node is 0 |
| C3 | c1 | F with counts `[6, 6, 6, 6, 3]` (the model's own layout): `solid_vtu_string` and `beam_fields` return `Ok` | exact | D1 step 4 |
| C4 | c1 | F with the result `DofMap::uniform(9, 3)` (27 unknowns, 9 nodes): the message contains exactly `the result numbers 9 nodes and the model 5` and `displacement` | exact | D1 step 2 |
| C5 | c1 | F with the result `DofMap::uniform(5, 3)` (N2, 15 unknowns): the message contains `length 15` and `expected 27` and does not contain `unknowns in the result and` | exact | D1 step 1 unchanged |
| C6 | c2 | `cargo test -p rcfd-post --release --test si_beam_output` (A1 to A6, N1, N2, N2b, N3) with the file unedited: its git blob equal to that at the base commit `c8ccbecf` | pass; blob equal | D1 steps 1 and 4 keep N2, A1 to A6; step 3 refuses N2b with the path `displacement` |
| C7 | c5 | `cargo test -p rcfd-post --release --test si_beam_output_bytes` (B0, B1: the six no-beam fixtures give the recorded FNV-1a 64 hashes and lengths), `sb_solid_output`, `si_scaffold_output` | pass | no change of the no-beam code |

Negative controls: N2 and N3 of ADR 0423 (unedited, in C6), C3 (a matching layout is not refused), C5 (a wrong length is not
given the layout text).

## Planned mutations (on a scratch git-archive copy, one at a time, each run once, after the unmutated checks passed)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU7 | the `DofMap` equality check removed: step 3 deleted, so a result with the model's number of unknowns and another layout is not refused | C1, C2 (an `Ok` where a refusal is required) and N2b of ADR 0423 |
| MU8 | step 3 names the last differing node instead of the first | C1 (node 4 instead of 3) |
| MU9 | the two counts swapped in the message | C1, C2 |
| MU10 | the loop of step 3 starts at node 1 | C2 |
| MU11 | step 2 (the node-count test) removed | C4 |

A mutation is caught if at least one listed check fails under it, not caught if all pass.

## Decision rule

- **done:** C1 to C7 met as registered, MU7 to MU11 caught by a check registered here, no amendment after a run.
- **partial:** a registered check missed, a mutation not caught, or a check met only after an amendment made after its run.
- **todo:** the change does not build.

## Limits (not covered)

- L1. Only the beam path (`beam_fields`, and `solid_vtu_string` for a model with Beam2 blocks). The solid path keeps its F-377
  refusals (`node k has n components; the solid output writes 3`, which already names the node and one count).
- L2. Only the first differing node is named; the message does not list all of them.
- L3. A model mixing Beam2 with other kinds stays refused (ADR 0423 L1).

## Open questions taken

1. Wording of the node-count refusal (step 2) and that only the first differing node is named: taken on the maintainer's
   standing instruction of 2026-11-11 (take the recommendation).

## Addendum 1: results, 2026-11-14

Code: `crates/rcfd-post/src/solid_output.rs`, `beam_fields` (lines 198 to 232 of the new file: the length refusal kept, then the
node-count refusal and the first-differing-node refusal of D1); new test file `crates/rcfd-post/tests/si_beam_layout.rs`
(C1 to C5). `si_beam_output.rs` is not edited (git blob `30790d28` at the base and at the tip). No shared solver file changed
(`rcfd-fem` untouched), so `baseline_hashes` was not run. Runs: `cargo test -p rcfd-post --release --test si_beam_layout
--test si_beam_output --test si_beam_output_bytes --test sb_solid_output --test si_scaffold_output`; each registered check was
run once on the final code (the first compile of the new test file ran the same tests before two cosmetic fixes, below).

| id | check | limit | measured | met |
| --- | --- | --- | --- | --- |
| C1 | N2b layout: message names node 3, counts 3 and 6, `displacement`, no `expected 27` | exact | `node 3 has 3 unknowns in the result and 6 in the model; ...` | yes |
| C2 | layout `[3, 6, 6, 6, 6]` names node 0 | exact | `node 0 has 3 unknowns in the result and 6 in the model` | yes |
| C3 | the model's own layout accepted by both entry points | exact | both `Ok` | yes |
| C4 | 9 nodes, 27 unknowns: node counts named | exact | `the result numbers 9 nodes and the model 5` | yes |
| C5 | wrong length keeps `length 15, expected 27`, no layout text | exact | as stated | yes |
| C6 | `si_beam_output` (A1 to A6, N1, N2, N2b, N3) passes, file unedited | pass; blob equal | 3 passed; blob equal | yes |
| C7 | `si_beam_output_bytes` (B0, B1), `sb_solid_output`, `si_scaffold_output` | pass | 2, 2, 4 passed... see note | yes |

Note on C7: `si_beam_output_bytes` 2 passed, `sb_solid_output` 2 passed, `si_scaffold_output` 4 passed as printed in the run (the
runner prints one line per binary; all `ok`, 0 failed).

Mutations (scratch git-archive copy `f516-scratch/mut` of the committed tip, one at a time, each run once):

| id | mutation | caught by |
| --- | --- | --- |
| MU7 | layout check removed | C1 and C2 failed; N2b of ADR 0423 also failed (run separately) |
| MU8 | last differing node named | C1 and C2 failed |
| MU9 | counts swapped | C1 and C2 failed |
| MU10 | loop starts at node 1 | C2 failed |
| MU11 | node-count refusal removed | C4 failed |

Deviations:

- The first compile of the new test file produced a warning (an unused constant copied from the fixture), the message of the
  refusal had a run of spaces from a lost string continuation, and `cargo fmt` reformatted two assertions. All three were fixed
  before the committed code; the checks are substring tests that the unfixed message also satisfied, and no limit was changed.
- MU7 was run with `cargo test` on both test binaries; the runner stops after the first failing binary, so N2b was run as a
  separate command for MU7 only (it failed as ADR 0423 expects of the checked code).
- fmt: `cargo fmt -p rcfd-post --check` clean after `cargo fmt -p rcfd-post`. clippy: `cargo clippy -p rcfd-post --all-targets
  -- -D warnings` clean (run before the final fmt, which changed only whitespace in the test file).

Not run: the whole workspace (the lead's gate); `baseline_hashes` (no shared solver file touched).

**Decision by the registered rule: done.** C1 to C7 met, MU7 to MU11 caught by checks registered before their run, no
amendment after a run.

### Remaining questions

1. Name all differing nodes instead of the first (L2)? Options: (a) leave; (b) list up to a few. Recommendation: (a).

## For the lead: roadmap entries

F-516: proposed status **done**. Note:

> Structural solver, intermediate tier. ADR 0430, 2026-11-14: rcfd_post::solid_output beam path refuses a result with the model's number of unknowns in another layout with a message naming the first differing node and both counts (node 3 has 3 unknowns in the result and 6 in the model), a result with another number of nodes with both node counts, and keeps the former length text for a wrong number of unknowns; the N2b test of ADR 0423 passes unedited; the mutation removing the layout check (and four more) is caught by checks registered before their run; output of models without beams byte-identical (B1 unedited).
