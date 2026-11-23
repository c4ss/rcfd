# ADR 0780: F-842 registration: the inertia and the retry of the nonlinear driver in the solid case's output (what the acceptance presupposes, what the solid case file and the CLI have today, the output registered for the opt-in, and one question of scope that is not taken)

Status: **REGISTERED 2026-11-23, written before any code of F-842 and before any run of its checks.** Before this text no
library, CLI or test code of F-842 was written and no file of the repository was changed. What was done: reading (below)
and searches of the source tree. Results go in addenda at the end; nothing above them is changed afterwards.

It answers the brief `docs/adr/briefs/f842-solid-inertia-in-output.md`
([the file](briefs/f842-solid-inertia-in-output.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f842-solid-inertia-in-output` from main
19debd2d, scratch `w49i-work/f842-solid-inertia-in-output-scratch`.

## What is asked

Roadmap item F-842 (deps F-780), the binding acceptance:

> Every limit not stated here is registered in the ADR before the run. With the solid case's opt-in for NewtonOptions::indefinite_retry, the inertia and the retry flag of every accepted increment written where the case reports its increments and shown by the CLI summary, with a warning when an accepted state has nu > 0; the 16^3 block of ADR 0694 run through the case file reports nu = 1 at full load; off by default, the default bits, the schema golden and the hash tables unchanged.

Clauses: **c1** an opt-in of the solid case for `NewtonOptions::indefinite_retry`; **c2** the inertia and the retry flag of
every accepted increment written where the case reports its increments; **c3** the same shown by the CLI summary; **c4** a
warning when an accepted state has `nu > 0`; **c5** the `16^3` block of ADR 0694 run through the case file reports `nu = 1`
at full load; **c6** off by default, with the default bits, the schema golden and the hash tables unchanged.

## What was read

`docs/process/agent-rules.md` (version 5, whole); the brief (whole); `crates/rcfd-api/src/solid_case.rs` (whole);
[docs/solid-case-file.md](../solid-case-file.md) (whole); the items F-842, F-386, F-403, F-405, F-435 and F-621 of
`docs/roadmap.json` (by a script: acceptance, deps, status, note);
[ADR 0719](0719-f780-registration-driver-retry-with-the-indefinite-tangent-after-a-refused-factor.md) (the acceptance,
D4 on the `16^3` block, the open questions, the remaining questions of addendum 1, the roadmap entries and the
maintainer's answer on F-842);
[ADR 0694](0694-f774-registration-the-corner-element-of-rule-l-at-the-punch-corner-and-the-equilibrium-past-the-predictor.md)
(the lines on the `16^3` block, its paths and its roadmap entry); `crates/rcfd-fem/src/nonlinear.rs` (`LoadStep`,
`NonlinearResult`, the documentation of `NewtonOptions::indefinite_retry`); `crates/rcfd-fem/tests/si_eas_hg18.rs` (how
the block is built: lines 95 to 180); `crates/rcfd-cli/src/main.rs` (the table of subcommands);
`scripts/roadmap_proposals.py` (the form of a proposal).

## Findings of the reading (before any code)

1. **The inertia is already returned to the caller.** `LoadStep::inertia: Option<usize>` and `LoadStep::retried: bool`
   are public fields of every converged increment in `NonlinearResult::steps` (ADR 0719 D1, D2). No change of
   `crates/rcfd-fem/src/nonlinear.rs` is needed and none is registered.
2. **The solid case file runs no nonlinear analysis and reports no increments.** `[analysis] type` has two values,
   `static` and `modal` (`solid_case.rs`, the `analysis` object of `schema()`); `run` calls `solve_static_report` (the
   linear static solve) or `modal_analysis` and nothing else. No file under `crates/rcfd-api/src`, `crates/rcfd-cli/src`
   or `crates/rcfd-mcp/src` names `NewtonOptions`, `LoadStep` or `rcfd_fem::nonlinear` (searched). The report
   `rcfd-solid-report/1` has no list of increments. ADR 0719 says the same in its open question 1: "the CLI and the case
   file do not show `LoadStep` yet".
3. **The case file cannot describe the `16^3` block.** The block (ADR 0694; `si_eas_hg18.rs` lines 95 to 180) needs a
   hyperelastic material (`Material::hyperelastic` with the neo-Hookean energy), the incompatible-mode formulation of the
   block of elements with the stabilisation of rule L (`BlockProperty::solid`, `set_incompatible_stabilisation`), the
   supernodal factor and the load-stepping driver with ten steps. The case file has isotropic linear elastic materials
   only, no formulation key and no load steps. Its node sets, constraints and the box generator do cover the block's mesh,
   its die constraints and the punch set (a box with `x, y <= 0.5` on `zmax`). The hyperelastic key is the todo item
   F-621 (with the formulation `bbar`, not the incompatible modes); no roadmap item names a nonlinear static analysis
   of the solid case file (F-403 and F-435 list their analyses and it is not among them).
4. **There is no CLI summary of a solid case.** `rcfd` has no subcommand that runs a solid case file (`main.rs`, the
   table of subcommands; `docs/solid-case-file.md`: "There is no CLI subcommand or MCP tool for it yet"). The subcommand
   is the todo item F-405 (deps F-403, F-386), whose acceptance says it needs the maintainer's approval of two golden
   edits before it starts and which also asks for two MCP tools.
5. **Goldens.** The solid case's schema is separate from the flow case's and is in no golden; the report is in no golden
   or hash table. A new subcommand would add lines to the CLI help golden (the additive class of rule 3).

So c1, c2, c4 and c5 presuppose a nonlinear static analysis in the solid case file, and c3 presupposes a CLI subcommand
for it. Neither exists; each is a line of work of its own size, and the second is a roadmap item that is blocked on the
maintainer. Building them inside F-842 would change the scope of the item. By rule 6 a question that changes the scope
of the item is not taken: it is question 1 below, and **no code of F-842 is written before its answer.**

## Design registered for the output (for the day the case file has the analysis)

What F-842 itself adds is small and does not depend on which option of question 1 is chosen. It is registered here so
that it is fixed before any code; the names of the analysis table it attaches to are fixed by an amendment when that
table exists.

- **D1. The opt-in.** A boolean key `indefinite_retry` (default `false`) in the table that holds the options of the
  nonlinear analysis, mapped to `NewtonOptions::indefinite_retry` and to nothing else. Given with an analysis that is
  not the load-stepping one it is refused with its key and line, as the other keys of another kind are.
- **D2. Where it is written.** In the JSON report, in the list that has one object per accepted increment
  (`LoadStep`): with the opt-in each object gains `inertia` (the integer of `LoadStep::inertia`, `null` for `None`) and
  `retried` (boolean). Without the opt-in neither key is written, so a report without the opt-in has the bytes it has
  without this item. The report gains a top-level list `warnings` (strings) only when it is not empty.
- **D3. The warning.** One per accepted increment whose `inertia` is `Some(nu)` with `nu > 0`, in order of the
  increments, with the text `increment <k> (load factor <lambda>): the accepted state has inertia nu = <nu> > 0, the
  equilibrium is not shown to be stable (ADR 0719 D2)`, `<k>` counted from 1 over the accepted increments and `<lambda>`
  the `load_factor` in Rust's shortest round-trip form. An increment with `inertia` `None` under the opt-in gets
  `increment <k> (load factor <lambda>): the inertia of the accepted state could not be determined (ADR 0719 D2)`.
  Source of the sign convention: Sylvester's law of inertia as used in ADR 0719 D2 (the negative pivots of the
  `L D L^T` of the reduced tangent count its negative eigenvalues); nothing new is derived here.
- **D4. The CLI summary.** One line per accepted increment with `nu` and `retried` when the opt-in is on, and each
  warning of D3 on standard error prefixed `warning: `; without the opt-in the summary's bytes are those without this
  item.
- No numerics of the driver change; `nonlinear.rs` is not touched (finding 1).

## Checks

None can be run before the answer to question 1. They are registered so that the limits stand before any code.

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| K1 | c1 | the key off or absent: the options passed to the driver | `indefinite_retry` false | acceptance of F-842 | false | exact | code verification |
| K2 | c1 | the key with another analysis | refused with key and line | acceptance of F-386 (located errors) | refused | exact | code verification |
| R1 | c2 | opt-in on a fast block: `inertia`, `retried` of every increment in the report | equal to the `LoadStep` records of the same model by library calls | acceptance of F-842 | equal | exact | code verification |
| R2 | c6 | opt-in off: the report and the `.vtu` | no `inertia`, `retried` or `warnings` key; bytes equal to the library run | acceptance of F-842 | equal | exact | unchanged default output |
| W1 | c4 | the warnings of a list of records with `inertia` 0, 1, 21 and `None` | none for 0; the two texts of D3 for 1 and 21; the second text of D3 for `None` | acceptance of F-842 | as D3 | exact | code verification |
| C1 | c3 | the CLI summary with the opt-in | `nu` and `retried` of every increment; the warnings of D3 on standard error | acceptance of F-842 | equal | exact | code verification |
| H16 | c5 | the `16^3` block through the case file with the opt-in (heavy, ignored, run once) | `inertia` 1 at load factor 1 and a warning for it; 0 at 0.1 to 0.9 and no warning | acceptance of F-842; ADR 0719 addendum 1 (RT16) | 1 | exact | solution verification |
| B1 | c6 | `baseline_hashes`, the schema golden test, the two existing solid case test files | pass unedited | rule 3 | pass | exact | unchanged default output |

Planned mutations (rule 16), for the day the code exists: M1 the key not passed to the options (caught by R1, H16);
M2 the warning given for `nu >= 0` (W1); M3 `inertia` written without the opt-in (R2); M4 the warning dropped (W1).
The committed one: M2. Negative control: the record with `inertia` 0 in W1 gives no warning.

Size (agent): partial; 1; 0; 1; 90000

## Decision rule

**done** if the opt-in reports `nu = 1` for the `16^3` block run through the case file, the warning is given as
registered and the default is unchanged in bits (the brief's rule). **todo** if no clause of the acceptance can be built
because question 1 is open; this is the outcome expected of this registration. **partial** otherwise. The brief's rule
has two outcomes (done, partial otherwise); the third is added because an item of which nothing was built is not
partly done.

## What is not covered

The nonlinear static analysis of the solid case file and the CLI subcommand themselves (question 1); MCP tools; whether
the retry becomes a default (ADR 0719, question 1, open there).

## Open questions for the maintainer

1. **The acceptance of F-842 presupposes a nonlinear static analysis in the solid case file and a CLI subcommand for
   solid cases; neither exists (findings 2 to 4). Where are they built?** Not taken (rule 6: it changes the scope of the
   item). Options:
   (a) F-842 waits. A new item adds the nonlinear static analysis to the solid case file (load steps and the tolerances
   of `NewtonOptions`, a hyperelastic material, the incompatible-mode formulation and its stabilisation rule, the factor,
   a list of increments in the report; file runs bit-equal to library calls); F-842 gains that item and F-405 (the
   subcommand) as deps and is issued again with this registration's D1 to D4.
   (b) F-842 is widened to build both: a minimal `type = "nonlinear"` analysis, the material and formulation keys the
   block needs, and an `rcfd solid` subcommand with its summary. It overlaps F-621 and F-405, adds lines to the CLI help
   golden (additive class), and makes F-842 an item of size M or more.
   (c) As (a) for the analysis, and c3 (the CLI summary) is taken out of F-842's acceptance and given to F-405, so that
   F-842 depends on the new item only and is done at the library entry and the report.
   Recommendation: **(c)**. The analysis is the larger part and deserves its own acceptance (bit-equality with library
   calls, located errors); the subcommand is already an item with its own approval pending, and its summary can show
   what the report holds once both exist.

## Addendum 1: results (2026-11-23)

No code of F-842 was written and no registered check was run: every check of the table needs the nonlinear static
analysis of the solid case file or the CLI subcommand, and where those are built is question 1, which rule 6 does not
let the agent take.

| Id | Limit | Measured | Met |
|---|---|---|---|
| K1, K2, R1, R2, W1, C1, H16 | as registered | not run (nothing to run them on) | not decided |
| B1 | pass unedited | not run: no source file was changed, so the default bits, the schema golden and the hash tables are those of main 19debd2d | not decided |

What was run: `cargo test -p rcfd-cli --release --test docs_site` (2 passed: the links of this ADR) and
`python scripts/process_report.py --check` (one failure: the generated `docs/adr/index.md` is not current, left to the
lead by rule 7). No mutation was run or committed: there is no test of F-842 for a patch to fail (rule 16 is not met and
cannot be before the code). No golden, schema golden or hash table needs an edit by this work; no additive diff to
report. No amendment. Deviations: the decision rule has a third outcome (registered above); the registration stops
before code.

Size (agent), actual: todo; 1; 0; 1; 95000

### Decision

**todo** by the registered rule: no clause of the acceptance could be built because question 1 is open. The findings
(the solid case file has no nonlinear analysis and reports no increments; the block's material and formulation have no
key; there is no CLI summary of a solid case) and the output design D1 to D4 stand for the next issue of the item.

### Open questions for the maintainer

Question 1 above, unchanged: options (a), (b), (c); recommendation (c).

## For the lead: roadmap entries

Status proposed: todo (unchanged).

> ADR 0780, 2026-11-23, todo: registered and stopped before code, because the acceptance presupposes a nonlinear static analysis in the solid case file (the file has static and modal only, reports no increments and has no key for the hyperelastic material or the incompatible-mode formulation of the 16^3 block) and a CLI subcommand for solid cases (F-405, todo); the inertia is already returned by the library (LoadStep::inertia, LoadStep::retried); the opt-in key, the report keys, the warning's text and the checks are registered for the next issue; where the analysis and the subcommand are built is open question 1 of the ADR (recommended: a new item for the analysis, the CLI summary moved to F-405).

No follow-up item is proposed in the file: which items are needed depends on the answer to question 1 (the item of
options (a) and (c) is described there).

## The lead's reading (2026-11-23), at the merge

Merged at c557e935 and pushed at 857bcff1 (the report read at 03:31 UTC, 5 minutes of reading, proposed todo, written
todo). Checked by the lead before the merge: the branch adds the registration, alone in its commit, the addendum and
the proposal file, and changes no source. The lead confirmed the finding on main: the analysis type of the solid case
file is `static` or `modal` (`crates/rcfd-api/src/solid_case.rs`), and no crate of the API, the CLI or the MCP server
names `NewtonOptions`. The brief was issued against an acceptance that presupposes what does not exist; the agent was
right to stop before code. The third outcome of the decision rule (todo) is accepted as registered.

Status: **todo**, as proposed; the brief is closed. Open question 1 is put to the maintainer with the agent's
recommendation (c), which the lead shares. The outputs registered here (the opt-in key, the report keys, the warnings
and the checks) stand for the next issue.
