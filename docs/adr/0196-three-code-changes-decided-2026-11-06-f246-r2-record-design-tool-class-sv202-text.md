# ADR 0196: three code changes decided on 2026-11-06: the F-246 R2 record, the class of two design tools in project mode, SV202's text per solver

Status: **REGISTERED 2026-11-06**, written before any code of the three changes. The changes were decided by the maintainer
on 2026-11-06; this text fixes how each is built and checked. Results go in the addendum at the end.

Parents: [ADR 0193](0193-f246-registration-pressure-interpolation-variants.md) (F-246, addendum 1 difference 1),
[ADR 0192](0192-f253-registration-particle-wall-film.md) (the record and corrected tests of case K, the pattern used in A),
[ADR 0195](0195-adr-0173-amendment-6-f275-registration-project-mode-tool-list-and-plugin-edit-hook.md) (P1, P8, Q6, C5-L),
[ADR 0189](0189-adr-0173-amendment-5-f274-registration-design-study-evaluations-on-the-runner.md) (Q7),
[ADR 0185](0185-adr-0173-amendment-1-f270-registration-the-rcfd-workspace-crate.md) (A2, the project golden),
[ADR 0135](0135-wave48f-plugin-and-project-tools.md) (F1, the default golden),
[ADR 0031](0031-wave19c-restart-chain.md) (D6, the set-up rules and their grammar),
[ADR 0191](0191-f259-registration-case-tree-and-cli-wiring-of-layering-and-compressible-ale.md) ("Observation, not fixed").

Read at main 2f60baf. Number: 0195 is the highest on main; no branch holds a 0196.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md), ADR 0192): a defective set-up gets a new corrected
test and the old one stays as an ignored record; a missed limit becomes an ignored `*_known_miss` test with its numbers, never
tuned; defaults stay bit-identical (no solver code, no case key, no case hash changes); `crates/rcfd-api/tests/baseline_hashes.rs`
and `case_hashes_w48u.rs` are not edited and nothing is re-recorded; a golden is never re-recorded silently, and an edit of an
existing test is a registered edit named here with its before and after; no new third-party crate; `Cargo.lock` unchanged; no
commit trailers.

## A. The F-246 R2 cases: the registered test as a record, the corrected test beside it

**What happened.** ADR 0193 registered T1 in `crates/rcfd-api/tests/pressure_interp_f246.rs` with its R2 cases for
`algorithm = "pimple"` and `algorithm = "coupled"` on C1, and C1 carries `physics.body_force`. The first run of T1 failed at the
`pimple` case: it is refused at `physics.body_force`, not at `solver.pressure_interpolation`.
Addendum 1 difference 1 records that both R2 cases then dropped the body force, in place, in the one test `t1_located_errors`,
and that the test gained the control "pimple without the key". The house rule asks for the registered test to stay as an ignored
record and the corrected test to be new, as ADR 0192 amendment 1 did for case K.

**A0. The change.** In `crates/rcfd-api/tests/pressure_interp_f246.rs` only:

- `t1_located_errors_setup_defect`, new, `#[ignore = "ADR 0193 addendum 1 difference 1 (ADR 0196 A): ..."]` naming the defect and
  the corrected test: T1 as registered. Its cases are R1 (`"momentum_weighted"`, parse), R2 V1 `pimple` on C1, R2 V1 `coupled` on
  C1, R2 V1 on the frozen-flow energy case, R3 V2 on C1 without `physics.body_force`, R3 the same with a `porous` source only, R4
  V2 on C1 with an `acceleration` source, and the registered controls (accepted): V1 on C1 without a body force, V2 on C1, V2 on
  the buoyant case. "On C1" means `cavity_text(0, true, ..)`, with `physics.body_force = [0, -10, 0]`. Ten cases.
- `t1b_located_errors`, the corrected test: the eleven cases of main's `t1_located_errors`, unchanged (the two R2 cases without
  `physics.body_force`, the control "pimple without the key"). The name `t1_located_errors` is no longer used.
- Both build their cases with one function (`t1_cases(registered: bool)`) and check them with one function that prints every
  case's outcome and asserts, after the last case, that each was refused at its expected path or accepted as expected. Main's T1
  stopped at the first mismatch; the pass rule is the same, and the record then shows every case's outcome, not only the first
  failing one.
- No other test of the file changes, and no file outside it.

**Checks.**

- **A1 (the corrected test).** `t1b_located_errors` is not ignored and passes in the gate; its eleven cases give the outcomes of
  ADR 0193's addendum row T1.
- **A2 (the record).** `t1_located_errors_setup_defect` is ignored in the gate. Run by name in release it is expected to fail,
  because the `pimple` case on C1 is refused at `physics.body_force` (ADR 0193 addendum 1). The addendum records, for each of the
  ten cases, the path it was refused at (or "accepted"); the `coupled` case on C1 was never run before and is recorded as it comes.
  No limit is involved.
- **A3 (nothing else).** The diff of the file against main touches only T1's lines and the new helper; every other test function
  is byte-unchanged.

## B. `design_run_study` and `design_evaluate`: class `execute` under `--project`

**The decision (maintainer, 2026-11-06).** In project mode `design_run_study` and `design_evaluate` start runs on the project's
runner (ADR 0189), and `start_run`, which starts runs, is `execute`. So both are reclassified from `write_files` to `execute`. This
overturns ADR 0195 P8 and Q6 (a) for these two tools (decision 9 there). The other half of ADR 0195 Q6 (b), checking `start_run`
with `new_case` also as `write_files`, was not decided and is not done.

**What the code read shows.** The class is the field `access` of each `Tool` in `tools()` (`crates/rcfd-mcp/src/lib.rs`), with the
names of `policy::Access::name`. `McpServer::all_tools()` gives both `tool_list()` and `call_tool()` their tools: the class goes
into `tools/list` (`_meta."rcfd/access"`, and through read or not read into `readOnlyHint`, `idempotentHint` and
`_meta."rcfd/needsApproval"`) and into the `ToolCall` the policy sees (read-only mode refuses every class but `read`; a hook gets
the class; a denial's `data.access` names it). `rcfd-assistant` reads `_meta."rcfd/access"` from a server without a project.

**Pinned bytes.**

- `crates/rcfd-mcp/tests/golden/tools_list_default.json`, the list without `--project` (ADR 0135 F1, `default_f1.rs`, never
  re-recorded), holds `"rcfd/access":"write_files"` for both tools. The class is part of pinned bytes, so the list without a
  project must not change: **only project mode changes**, and this golden is not edited.
- `crates/rcfd-mcp/tests/golden/tools_list_project_f270.json` (ADR 0185 A2, `project_list_f270.rs`, never re-recorded), the
  project list of 7c960bb, holds `write_files` for both. It is not edited; the test that restates it is (E2).
- So no golden changes. Two existing tests change by registered edits (E1, E2).

**B0. The change.** `crates/rcfd-mcp/src/lib.rs`: a constant `PROJECT_EXECUTE: [&str; 2] = ["design_run_study",
"design_evaluate"]`, and in `all_tools()`, when the server has a project, those two tools get `Access::Execute`. Nothing else:
their name, description, schema and dispatch stay; `readOnlyHint` (false), `idempotentHint` (false) and `needsApproval` (true)
do not change, since both classes are not `read`. Without a project every byte of `tools/list` and every answer stay as before.

**Registered edits of existing tests.**

- **E1. `crates/rcfd-mcp/tests/tool_list_f275.rs`, C5-L** (`c5l_project_list_has_no_session_only_tool`). Before: each of the
  17 kept session tools under `--project` is byte-identical to its entry in `tools_list_default.json`. After: the same, except
  that for `design_run_study` and `design_evaluate` the live entry's `_meta."rcfd/access"` must be `"execute"` and the golden's
  `"write_files"`; with the live value replaced by the golden's, the entry is byte-identical to the golden's. The module comment
  and the test's comment name this ADR.
- **E2. `crates/rcfd-mcp/tests/project_list_f270.rs`, A2** (`a2_tools_list_with_project_is_the_golden_with_the_f271_changes`).
  Before: the live list with the 16 hidden tools put back from the default golden and `read_feed`'s `from_seq` removed is the
  F-273 list. After: in that rebuilt list, the two tools' `_meta."rcfd/access"` must be `"execute"` and is set back to
  `"write_files"` before the remaining checks, which are unchanged; the module comment names this ADR.
- **Not edited:** `default_f1.rs` and both goldens; `project_w48f.rs` (its class table has no design tool);
  `src/design_tests.rs` T5 and `src/tests.rs` (servers without a project); the plugin's skills and `hooks/hooks.json` (no design
  tool is named there); `rcfd_claude::allow::PROJECT_READ_TOOLS` (read tools only; both tools are not `read` in either class).

**Checks.**

- **B1 (the class, new test `b1_design_tools_are_execute_under_project_adr0196` in `tool_list_f275.rs`).** Through the library
  (`McpServer::new(..).with_project(..)` on a project folder, and a server without one), with a hook policy that records each
  `(tool, access)` it is asked about and refuses:
  (i) `tools/list` under a project: both tools `"execute"`, `needsApproval` true, `readOnlyHint` false; without a project both
  `"write_files"`;
  (ii) a call of each tool (arguments its schema accepts) under a project: the hook is asked once with `Access::Execute`, the
  result is an error result with code 1100 and `data.access` `"execute"`, and the project folder's files and bytes are the same
  before and after; without a project the hook is asked with `Access::WriteFiles`.
- **B2 (the binary).** E1 and E2 pass; `default_f1.rs` passes unedited.
- **B3 (can fail).** One mutation, restored afterwards and compared byte for byte with the kept copy: the line in `all_tools()`
  that sets `Access::Execute` removed. Then B1 and E1's C5-L must fail (and E2's A2). Recorded in the addendum.

## C. SV202's text names the default of the solver

**The decision (maintainer, 2026-11-06).** SV202 (info, `boundaries`) says a patch without a boundary condition takes "a fixed
no-slip wall"; for the compressible solver it is a slip wall (ADR 0191, "Observation, not fixed"). The message becomes
solver-aware: no-slip for incompressible, slip for compressible. Its id, severity, category, path and condition stay.

**What the code read shows (main 2f60baf, nothing run).**

1. SV202 is data, `crates/rcfd-api/rules/setup_rules.json`: `when` `count(boundaries) == 0`, path `boundaries`, message
   `no boundary conditions are set: every patch takes the default (a fixed no-slip wall)`. A rule has one string `message`; ids are
   unique, so the rule cannot be split into two with the same id. The engine is `crates/rcfd-api/src/rules.rs`.
2. The compressible solver is selected by `solver.algorithm = "compressible"` (`comp::enabled`). `rcfd_fv::compressible`
   starts every patch as `CompBc::Slip`, except a patch of kind outlet (`CompBc::Extrapolate`), and `CompState::new` replaces only
   the patches named under `boundaries`.
3. The other solvers: `Simulation::new` refuses a case in which a mesh patch that is neither empty nor cyclic has no
   `boundaries.<patch>` entry ("no boundary condition for mesh patch '<patch>'", located at `boundaries.<patch>`). `rcfd check`,
   `--check-only`, `rcfd validate` and `rcfd run` evaluate the rules only after the simulation is built, so for those solvers a
   case without boundaries is refused before SV202 can show; the no-slip text is reached only where the rules are evaluated on
   such a case directly (`rules::check_case`, as `rules_w19c.rs` E1 does with `Case::new()`). See Q1.
4. No test pins the wording: the text appears only in `setup_rules.json`; `rules_w19c.rs` E1 checks SV202's id, severity,
   category and path and that its message is not empty. No document quotes it.

**C0. The change.**

- Grammar (ADR 0031 D6; the rule file stays format `rcfd-setup-rules` version 1): a rule's `message` is a string, as before, or
  `{"first_of": [{"when": C, "message": S}, ..., {"message": S}]}`, read like `path`'s `first_of`: the first entry whose `when`
  holds, or that has none, gives the message; `{name}` is substituted as before. Parsing refuses a `first_of` that is not a
  non-empty list, an entry with a key other than `when` and `message`, an entry without a string `message`, an invalid `when`, and
  a last entry with a `when` (so a rule that fires always has a message). A string `message` works as before.
- Data: SV202's `message` becomes `first_of` with two entries: `{"when": {"path": "solver.algorithm", "op": "eq", "value":
  "compressible"}, "message": "no boundary conditions are set: every patch takes the default of the compressible solver (a slip
  wall)"}`, then `{"message": "no boundary conditions are set: every patch takes the default (a fixed no-slip wall)"}`. The second
  text is the old one, byte for byte. No other rule changes.
- The module comment of `rules.rs` names the message's `first_of`.

**Checks.**

- **C1 (both wordings, new test `e5_sv202_names_the_default_of_the_solver_adr0196` in `crates/rcfd-api/tests/rules_w19c.rs`).**
  `Case::new()` (the default algorithm) gives exactly one SV202 diagnostic with the no-slip text, byte for byte; `Case::new()` with
  `solver.algorithm = "compressible"` gives exactly one with the slip text, byte for byte; each other value of
  `solver.algorithm`'s enum gives the no-slip text; in every case id `SV202`, severity info, category `boundaries`, path
  `boundaries`. A case with one boundary entry gives no SV202 with either algorithm.
- **C2 (the grammar, new test `e4b_message_first_of_is_validated_strictly_adr0196` in the same file).** Each refused shape of C0
  is refused with a message naming the problem; a valid two-entry `first_of` gives the first entry's text when its `when` holds and
  the last entry's otherwise, with `{name}` substituted under `each`.
- **C3 (unchanged).** `rules_w19c.rs` E1 to E4 pass unedited (21 rules, the same ids).
- **C4 (can fail).** One mutation, restored and compared byte for byte: SV202's compressible entry's `value` changed from
  `"compressible"` to a value no case has. Then C1 must fail. Recorded in the addendum.

## Gate and documents

- **Gate.** `cargo fmt -p <crate> -- --check` for every package, `cargo clippy --workspace --all-targets -- -D warnings`, then
  every test in release on a `git archive` copy of the gated commit, one process per test, three jobs
  (`scripts/gate_test.py`): no failure. A failure of a known flake (`runner_w47b` s1, s4a, s4b, s9; `case_path_wfix` r9;
  `journal_f273` c3s; `chat_e` e1; `driver_fake` d5; `preview_gpu_g1` g10) is rerun alone and reported with its name.
  `baseline_hashes.rs` and `case_hashes_w48u.rs` pass unedited.
- **Documents.** `docs/mcp-tools.md` (the two rows and the project-mode text), `docs/design-loop.md` (its access sentence and
  the two rows), `docs/case-schema.md` (the set-up rules paragraph: a message may depend on the case). `docs/ROADMAP.md` is not
  edited: the record of these changes is this ADR.

## Questions for the maintainer

**Q1. SV202's text where it does not match the code (C, items 2 and 3).** (a) As decided: "a fixed no-slip wall" for every
solver but the compressible one, "a slip wall" for the compressible one. (b) Also say what the code does in the two cases the
decided texts leave out: for the incompressible solvers a case without boundaries is refused, every mesh patch needs a
`boundaries.<patch>` entry, and an entry without keys is a fixed no-slip wall; for the compressible solver a patch of kind outlet
is extrapolated, not a slip wall. Recommendation (b): the message then says what happens. Proceeding on (a), the decision; (b) is
a text change only.
