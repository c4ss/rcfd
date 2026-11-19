# ADR 0741: F-271 registration: the field-store plan as the default of start_run

Status: **REGISTERED 2026-11-19, written before any code of F-271 and before any run of its checks.** Before this text no
library or test code of this registration was written and none of its checks was run. One thing was done before it: a
release build of the unchanged code of main (`cargo build --release -p rcfd-cli -p rcfd-runner -p rcfd-mcp`), which runs no
test. Every number below comes from reading the code and the ADRs, with the arithmetic done by hand. Results go in the
addenda at the end; nothing above them is changed after a run.

This is a further registration of roadmap item F-271 under rule 13: it adds no acceptance clause, it closes the first half
of the item's gap and reads the second. It answers the brief
[docs/adr/briefs/f271-start-run-default-plan.md](briefs/f271-start-run-default-plan.md) and follows
`docs/process/agent-rules.md`, version 3. Parent records:
[ADR 0186](0186-adr-0173-amendment-2-f271-registration-new-simulations-from-the-mcp-server.md) (the first registration of
F-271, its question Q2 and the maintainer's answer of 2026-11-18) and
[ADR 0173](0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md) (decision D7, check C7).
Read at main b52b882a.

## What is asked

The maintainer approved decision D7 of ADR 0173 and option (a) of ADR 0186 Q2 on 2026-11-18 ("Answers of the maintainer"
at the end of ADR 0186): the field-store plan is on by default for every `start_run` in project mode, also of an existing
case with no store setting. This is an approved change of a default, so rule 3's "defaults stay bit-identical" is lifted for
exactly the outputs listed under "What changes, case by case" and for nothing else.

The second half of the gap of F-271 is a reading task: whether the Electron Studio (`apps/studio-web`) uses the shared
start plan. The Studio is not changed here.

## What was read

- `docs/process/agent-rules.md` (version 3), the brief, ADR 0186 whole, ADR 0173 (context items 7 and 8, D7, the design of
  R1, check C7, the risks), `crates/rcfd-mcp/src/project.rs` whole.
- `crates/rcfd-workspace/src/plan.rs` whole and `runs.rs` lines 297 to 542 and 682 to 686 (`start_run`, `dry_start`,
  `store_json`).
- Every caller of `start_run` under `crates/`: `rcfd-mcp/tests/new_simulations_f271.rs`, `project_runs_w48f.rs`,
  `project_w48f.rs`, `case_facts_adr0737.rs`, `feed_f273.rs`, `run_records_adr0734.rs`, `runner_stdio_w48l.rs`,
  `sites_adr0739.rs`, `project_list_f270.rs`, `tests/common/mod.rs`, `tests/golden/project_shapes.json`, and
  `rcfd-workspace/tests/new_case_f271.rs`, `stop_f834.rs`.
- The Studio: `apps/studio-web/src/shared/actions/runs.ts` lines 137 to 215, `shared/actions/first.ts` lines 28 to 40,
  `main/project/control.ts` lines 290 to 310, and a search of `apps/studio-web/src` for `start_run` and `store`.
- `docs/mcp-tools.md` (the `start_run` row), `docs/run-control.md` (the paragraph on the start plan).

## Readings, registered

**R1. The default is one constant.** `rcfd_workspace::plan::PLAN_FOR_PROJECT_CASES` (`plan.rs` line 29) is `false`; its one
use is `runs.rs` line 351: `None => new_case.is_some() || PLAN_FOR_PROJECT_CASES`. ADR 0186 Q2 says "Reversible: one
constant". `Workspace::start_run` is called from one place (`lib.rs` line 285, the command `start_run`); the design tools
submit through `submit_tagged` and `submit_with`, `rcfd-runner start` and `rcfd run` do not call the workspace, and a rerun
(`from_run`) returns at `runs.rs` line 337, before the plan.

**R2. What the plan does with a case is already built and registered** (ADR 0186 G4 and G5, `Plan::with_set`): the case
is read as `rcfd run` reads it, the call's `set` entries are applied, no store is proposed when the case sets
`output.store_interval` or `output.store_time_interval` to a value other than its default, when it sets
`dynamic_mesh.enabled`, or when a `set` entry names one of the two interval keys; the plan's entries go before the caller's.
A case that does not parse is refused with 1003 before anything is queued.

**R3. The way back exists.** `store: "none"` is in `start_run`'s schema since ADR 0186 G5 and is ADR 0173 D7's "caller's
way back to today's behaviour". It skips the plan and the reading of the case.

**R4. The reply.** With the plan, the reply has `store` {`applied`, `set`, `snapshots`, `estimated_bytes`, `note`}
(`store_json`), also when no store is proposed (`applied` false with the note). ADR 0173 D7 (a) asks for it ("The tool result
reports the planned snapshot count and size"), and its risk list says the report "is in every `start_run` result".

**R5. The Electron Studio uses the shared start plan** (reading; nothing run). Every start of a case from the Studio is the
engine's `start_run` with the store argument written out, so the plan is `rcfd_workspace::plan` and the Studio has no copy
of the rule:

- Run the case: `apps/studio-web/src/shared/actions/runs.ts` line 147, `mcpArgs: (p) => ({ case: p.case, store: 'plan' })`.
- Queue the case: `runs.ts` line 175, `store: p.store ?? 'plan'`; its form offers `none` and `plan` (lines 166 to 171).
- Run the first example: `apps/studio-web/src/shared/actions/first.ts` line 36, `store: 'plan'`.
- A run on a site: `apps/studio-web/src/main/project/control.ts` line 304 asks `start_run` with `store: 'plan'` and
  `dry_run: true` for the plan's entries and passes them to `rcfd-runner start --site`.
- A rerun is `start_run {from_run}` (`runs.ts` line 210), which keeps the settings of the run's record.

Because the Studio always names `store`, the change of the default does not change any call the Studio makes. Two texts
describe the old default and become stale: the comment at `runs.ts` line 146 ("start_run's own default (none) and the
CLI's stay") and the sentence of `docs/run-control.md` that says Run, Queue and F5 of the desktop Studio store only what
the case asks for (stale already: they ask for the plan since ADR 0737). The first is a follow-up for the Studio stream
(proposed in the addendum); the second is in this item's topic page and is corrected here.

## What changes, case by case

A call `start_run` in project mode with `case` and without `store`. Before: the run got exactly the caller's `set`, and
the reply was `{run, case, state}` (plus `site`, `run_time` where asked). After:

| Case and call | Run record and stored output | Reply |
|---|---|---|
| The case sets no store interval, no `dynamic_mesh.enabled`; no `set` entry names an interval key | `settings.set` of `run.json` begins with the plan's entries (`output.store_interval=k` or `output.store_time_interval=dt`, then `output.store_precision=f32` unless the case sets the precision), so the launch has those `--set` entries and the run writes `runs/<id>/fields/store.rts` (at most about 100 snapshots, f32). **Changed.** | gains `store` with `applied` true. **Changed.** |
| The case sets `output.store_interval` or `output.store_time_interval` (not the default) | the same bytes as before: no entry is added, the case's own store is written | gains `store` with `applied` false and the note. **Changed (reply only).** |
| The case sets `dynamic_mesh.enabled` | as before: no entry, no store | gains `store`, `applied` false. **Changed (reply only).** |
| A `set` entry names `output.store_interval` or `output.store_time_interval` | as before: only the caller's entries | gains `store`, `applied` false. **Changed (reply only).** |
| The case file does not read as a case, or a `set` entry is refused by the case | before: the run was queued and the solver refused it. After: error 1003, nothing queued. **Changed.** | error 1003 |
| With `dry_run: true` | nothing is written, as before | `store` is the plan (it was null) and `set` holds the plan's entries. **Changed.** |
| With `run_time` | the plan's entries stand between the run-time entries and the caller's (the order of ADR 0737) | as the rows above |
| With `site` or `continue_from` | the same rule: the run on the site, or the continued run, gets the plan's entries | as the rows above |

Not changed: `start_run` with `store: "plan"` or `store: "none"`; with `new_case` (the plan was its default already); with
`from_run`; the design tools; `rcfd-runner start`, `rcfd-runner rerun` and `rcfd run`; every call of the Studio (R5); the
tool list (no descriptor changes, see D3); the session tools; anything without `--project`.

The solver's numbers do not change with the store: the store is written from the solution and
`output.store_time_interval` "does not change the time step" (`docs/run-control.md`, Field store). Row 6 below checks it.

## Design

**D1. The constant becomes `true`.** `PLAN_FOR_PROJECT_CASES = true` in `plan.rs`, with its comment rewritten to say that
the default is on by ADR 0741 on the maintainer's approval of 2026-11-18, and that `store: "none"` is the way back. No other
line of library code changes. The constant stays, so the old default is one line away and the committed mutation (rule 16)
is that line.

**D2. The way to ask for the old behaviour is `store: "none"`** (R3). It is tested for a case (row 4): the reply has
exactly `run`, `case`, `state`, the run record has no added entry and no store is written.

**D3. No descriptor is changed.** The description of `start_run`'s property `store` in `crates/rcfd-mcp/src/project.rs`
ends "Default plan with new_case, none with case". After D1 that sentence is wrong for `case`. It is part of the description
of an existing tool in the MCP tools list, so by the brief and rule 3 it is not an additive edit: it is left as it is and
reported as the step that needs approval (open question 1), with the one-line change.

**D4. Existing tests that assert the old default.**

- `crates/rcfd-mcp/tests/new_simulations_f271.rs`, test `c7_one_start_run_makes_a_case_from_an_example_and_stores_its_fields`,
  control (ii): `start_run {case: "cases/sod.toml"}` must return exactly `case`, `run`, `state`, with an empty
  `settings.set` and no store. It asserts the old default. The test is **retired by this registration as an ignored
  record**: it gets the attribute `#[ignore = "..."]` naming ADR 0741 and no other change. The new file below asserts all of
  C-7 again with control (ii) in its new form, so clause C7 of the acceptance keeps a running check.
- `crates/rcfd-mcp/tests/project_runs_w48f.rs`, test `f3_f11_project_tools_on_runner_made_runs`, compares the shape of the
  reply of `start_run {case: "cases/cavity.toml"}` with the golden `tests/golden/project_shapes.json`, whose entry is
  `{"case": "string", "run": "string", "state": "string"}`. The fixture sets `store_interval = 10`, so its runs do not
  change, but the reply gains `store`. The golden is not edited (rule 3). The change adds one key to one entry and removes
  or changes none, so it is reported as an **additive golden edit** with the diff, for the lead to re-record:
  `"start_run"` gains `"store": {"applied": "bool", "estimated_bytes": "null", "note": "string", "set": [], "snapshots":
  "null"}`. Until it is re-recorded that test fails at this one shape; row 9 checks that this is the only difference.
- No other test asserts the old default, by reading: the other callers (`project_w48f.rs`, `case_facts_adr0737.rs`,
  `feed_f273.rs`, `run_records_adr0734.rs`, `runner_stdio_w48l.rs`, `sites_adr0739.rs`, `stop_f834.rs`) use a case that
  sets its own store (the fixture `cavity`, the shipped cylinder) or name `store`, and read `run`, `site`, `run_time` or
  `rerun_of` from the reply, not its key set. Row 10 runs them unedited.

Other records of the old default, which are documents: the `start_run` row of `docs/mcp-tools.md` and the start-plan
paragraph of `docs/run-control.md` (both corrected here, rule 9), and the roadmap gap of F-271 (the lead's).

**D5. The new test file** `crates/rcfd-mcp/tests/start_run_default_plan_adr0741.rs` (header: ADR 0741, F-271), through
the `rcfd-mcp` binary with the real runner, as C-7 was. Two tests:

- `c7_restated`: ADR 0186 C-7 as registered there, with control (ii) replaced by the new default.
- `default_plan_case_by_case`: a project with `cases/plain.toml` (the 12 x 12 lid-driven cavity of the fixture, `simplec`,
  30 iterations, the `lid_drag` report, **no** `[output]`), `cases/cavity.toml` (the fixture, `store_interval = 10`) and
  `cases/bad.toml` (text that is not TOML).

Arithmetic for `plain.toml`, by the rule of `plan.rs`: steady, `max_iterations` 30, so `k = max(1, ceil(30 / 100)) = 1`,
entries `output.store_interval=1` and `output.store_precision=f32`; snapshots `ceil(30 / 1) = 30`; estimated bytes
`144 cells x 4 fields x 30 x 4 = 69 120`. With `set: ["solver.max_iterations=250"]`: `k = ceil(250 / 100) = 3`, entry
`output.store_interval=3`, snapshots `ceil(250 / 3) = 84`. For `sod.toml` the numbers are ADR 0186's:
`output.store_time_interval=0.002`, 100 snapshots.

## Checks

Limits are exact (equal values, equal key sets, equal lines), from the tolerance table's row "Bit-identity of default
output" or from the parent's acceptance; where the estimate equals the limit the margin is written 1.

| # | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| 1 | Acceptance clause "creates a case ... and starts a run of it ... in one call ... with the same start plan as the Studio, so the run's fields are stored" (`c7_restated`): `start_run {new_case: sod}` writes `cases/sod.toml` byte-equal to the example; `store.applied` true, `store.set` the two sod entries, `store.snapshots` 100; `run.json` `settings.set` equal to `store.set`; finished within 300 s; n snapshots; `field_summary` p finite; controls (i) `store: "none"` with `new_case`, (iii) the taken name refused 1004, and the `set` entry with an interval key | as ADR 0186 C-7: byte-equal; exact values; 50 <= n <= 101 | ADR 0186 C-7 (parent acceptance) | n = 100 (ADR 0186 Addendum 1) | 1 | code verification |
| 2 | The new default, C-7 control (ii) restated: `start_run {case: "cases/sod.toml"}` returns exactly the keys `case`, `run`, `state`, `store`; `store.set` and `settings.set` are the two sod entries; finished; n snapshots | exact; 50 <= n <= 101 | ADR 0186 Q2 (a) as approved; ADR 0186 C-7 for n | n = 100 | 1 | code verification |
| 3 | The new default on a steady case (`plain.toml`, no `store`): keys `case`, `run`, `state`, `store`; `store.applied` true; `store.set` = [`output.store_interval=1`, `output.store_precision=f32`]; `store.snapshots` 30; `store.estimated_bytes` 69 120; `settings.set` = `store.set`; finished at step 30; n snapshots in the store; `field_summary` p finite | exact; 30 <= n <= 31 | the arithmetic of D5 by `plan.rs` (ADR 0186 G4); one more snapshot allowed for an initial or final state, as C-7 allowed 101 for 100 | n = 30 | 1 | code verification |
| 4 | The way back (D2): `start_run {case: plain, store: "none"}` returns exactly `case`, `run`, `state`; `settings.set` empty; finished at step 30; no `fields/store.rts` | exact | tolerance table, "Bit-identity of default output"; ADR 0173 D7 | as the limit | 1 | unchanged default output |
| 5 | A case with its own store (`cavity.toml`, no `store`): `store.applied` false, `store.set` empty, the note names the case's own store; `settings.set` empty; the steps of the stored snapshots equal those of the same case run with `store: "none"` | exact | tolerance table, "Bit-identity of default output" | equal | 1 | unchanged default output |
| 6 | The store does not change the solution: the lines of `residuals.jsonl` of the run of row 3 equal those of the run of row 4, and `metrics.json` `reports.lid_drag` is equal bit for bit | exact | tolerance table, "Bit-identity of default output" | equal | 1 | unchanged default output |
| 7 | `set` entries: `start_run {case: plain, set: ["output.store_interval=5"]}` gives `store.applied` false and `settings.set` = that entry; `set: ["solver.max_iterations=250"]` gives `store.set` = [`output.store_interval=3`, `output.store_precision=f32`], `store.snapshots` 84 and `settings.set` = those two then the caller's | exact | D5 arithmetic; ADR 0186 G5 (order, the interval-key rule) | as the limit | 1 | code verification |
| 8 | `dry_run` and a file that is no case: `start_run {case: plain, dry_run: true}` has `store.applied` true and `set` = the two entries of row 3, and the run folders are the same before and after; `start_run {case: "cases/bad.toml"}` is refused with 1003 and adds no run folder; with `store: "none"` the same file is queued (a run id) and the run ends in a state other than `finished` | exact | ADR 0186 G5 ("refused with 1003 before anything is queued"); ADR 0737 (dry run) | as the limit | 1 | code verification |
| 9 | Additive golden edit (D4): `f3_f11_project_tools_on_runner_made_runs`, unedited, with the committed golden reports 18 of 19 shapes equal and names only `start_run`; with the one key of D4 added to a copy of the golden (not committed) it passes | 1 shape differs, `start_run`; then 0 | rule 3 (additive class) | as the limit | 1 | unchanged default output |
| 10 | Existing tests, unedited, pass: `rcfd-mcp` `project_w48f`, `case_facts_adr0737`, `feed_f273`, `run_records_adr0734`, `runner_stdio_w48l`, `sites_adr0739`, `project_list_f270`, `tool_list_f275`, `tool_list_adr0730`, `default_f1`, the second test of `new_simulations_f271`; `rcfd-workspace` `new_case_f271`, `store_plan_f834`, `store_runs_f834`, `stop_f834`; `--lib` of `rcfd-workspace` and `rcfd-mcp`; `rcfd-cli` `docs_site` | 0 failures | rule 7; tolerance table, "Bit-identity of default output" (the tool list is unchanged) | 0 | 1 | unchanged default output |
| 11 | The retired record: the ignored `c7_...` of `new_simulations_f271.rs`, run once with `--ignored`, fails at control (ii) (the key set) | fails there | D4 | fails there | 1 | code verification (negative control) |
| 12 | The reading R5 (the Studio uses the shared plan), with file and line | stated | the brief | stated above | - | reading, no run |

No row is of kind validation: the acceptance of F-271 claims no agreement with a measured case.

### Negative controls and planned mutations

Row 4 is the control of row 3 (the same case stores nothing when asked), row 11 the control of the retirement, and in row 8
the `store: "none"` call is the control of the refusal. Mutations, each on the code commit, run once against the new file
and restored:

| Mutation | Must fail |
|---|---|
| M1 (committed, rule 16): `PLAN_FOR_PROJECT_CASES` back to `false`. Patch `crates/rcfd-mcp/tests/start_run_default_plan_adr0741.mutation.patch` | both tests of the new file (rows 2 and 3) |
| M2: in `runs.rs`, `Some("none") => false` becomes `true` (the way back removed) | rows 1 (control (i)) and 4 |

## Size

Size (agent): done; 0; 0; 1; 103070

## Decision rule

- **done**: rows 1 to 11 are met, with nothing left out but the additive golden edit of row 9 and the description of D3.
- **partial**: a row of 1 to 8 or 10 is missed or not run, with the miss recorded by rule 2.
- **todo**: the default cannot be made without a golden edit that is not additive.

## What is not covered

- The tool description of `store` (D3) stays wrong for `case` until it is approved.
- A case with `dynamic_mesh.enabled` and the `site` and `continue_from` rows of the table are not run here: the rule is
  one code path (`Plan::with_set`), checked for the moving mesh by `store_plan_f834.rs`.
- The Studio is read, not run; `npm test` of `apps/studio-web` is not run because nothing there is touched.
- Disk use: every run Claude starts of a case without a store setting now writes a store. The size is in the reply; no
  budget is set here (ADR 0173, risks).
