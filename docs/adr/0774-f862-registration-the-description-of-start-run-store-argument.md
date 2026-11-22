# ADR 0774: F-862 registration: the description of start_run's store argument says what the default is

Status: **REGISTERED 2026-11-22, written before any code of F-862 and before any run of its checks.** Before this text no
library or test code of this registration was written, nothing was built and none of its checks was run. Every statement
below comes from reading the code and the records. Results go in the addenda at the end; nothing above them is changed
after a run.

It answers the brief [docs/adr/briefs/f862-start-run-store-description.md](briefs/f862-start-run-store-description.md)
and follows `docs/process/agent-rules.md`, version 5. Parent record:
[ADR 0741](0741-f271-registration-the-field-store-plan-as-the-default-of-start-run.md) (D3, open question 1 and the
maintainer's answer of 2026-11-21, option (a)). Read at main d6e44b27.

## What is asked

The acceptance of F-862 in `docs/roadmap.json`, read with a script: "The description of the store argument of start_run in
the project-mode tools list no longer says "none with case": it says that plan is the default and that none is the way
back to a run that stores only what its case asks for, shown by a test that reads the live tools list; no other text of the
tools list, no argument, no default and no golden changes, and the Studio's calls are unchanged."

This is an approved change of the text of an existing tool (the maintainer's answer in ADR 0741), so rule 3 is lifted for
exactly that one string and nothing else. No behaviour changes: ADR 0741 already made `plan` the default.

## What was read

- `docs/process/agent-rules.md` (version 5), the brief, ADR 0741 whole.
- `crates/rcfd-mcp/src/project.rs`, the `start_run` entry (lines 391 to 415); a search of the repository for the words
  "none with case" (found only in `project.rs` line 399 and in records: ADR 0741, the brief, the roadmap).
- `crates/rcfd-mcp/tests/project_list_f270.rs` whole, the head of `tool_list_f275.rs` and `tool_list_adr0730.rs`,
  `tests/common/mod.rs` (the `Mcp` helper), the golden `tests/golden/tools_list_project_f270.json` (its `start_run` has no
  `store` property: `store` came with F-271, and `project_list_f270.rs` takes the live `store` property into its
  reconstruction without comparing it to anything), `tests/golden/tools_list_default.json` (session tools only; no
  `start_run`).
- `docs/mcp-tools.md`, the `start_run` row.
- The Studio's calls of ADR 0741 R5: `apps/studio-web/src/shared/actions/runs.ts` (lines 147, 175, 210),
  `shared/actions/first.ts` (line 36), `main/project/control.ts` (line 304), and a search of `apps/studio-web/src` for the
  words "Default plan" and "none with case" (none).

## Design

**D1. The new text.** The description of the property `store` of `start_run` is today

    plan: the Studio's start plan stores the run's fields (about 100 snapshots, f32) unless the case or set gives the store; none: only what the case asks for. Default plan with new_case, none with case

Its last sentence is replaced by option (a) of ADR 0741 question 1, word for word as approved:

    plan: the Studio's start plan stores the run's fields (about 100 snapshots, f32) unless the case or set gives the store; none: only what the case asks for. Default plan; none is the way back to a run that stores only what its case asks for

Nothing else of `project.rs` changes: not the property's `type` or `enum`, not the order of the properties, not any other
description.

**D2. No golden changes.** No golden holds the `store` property (read above): `tools_list_project_f270.json` predates it and
`tools_list_default.json` has no `start_run`. `project_list_f270.rs` copies the live `store` property into the list it
compares, so it passes with either text. `project_shapes.json` holds reply shapes, not descriptions.

**D3. The test that reads the live tools list.** New file `crates/rcfd-mcp/tests/start_run_store_description_f862.rs`
(header: ADR 0774, F-862). One test, `store_description_names_plan_as_the_default`, through the `rcfd-mcp` binary with
`--project` (the `Mcp` helper of `tests/common`): `initialize`, `tools/list`, then on the tool `start_run`:

- the property `store` equals, as a JSON value, `{"type": "string", "enum": ["plan", "none"], "description": <D1 new text>}`
  exactly (so its type and enum are held too);
- its description does not contain "none with case" and contains "Default plan;" and "none is the way back to a run that
  stores only what its case asks for".

**D4. The comparison of the whole list, before and after, less the one description.** A one-off check, not a test of the
gate, because a pinned copy of the whole list would be a new golden and would fail at every later additive change (another
agent changes rcfd-mcp for F-876 at the same time). Its record is kept in the scratch folder; the ADR records the sizes and
hashes. Steps:

1. After this registration's commit and before the code commit, build `rcfd-mcp` from the unchanged code and write the
   `tools/list` response of the binary with `--project <empty project>` and without `--project` to `before_project.json`
   and `before_default.json`.
2. After the code commit, the same to `after_project.json` and `after_default.json`.
3. A script (`compare_f862.py` in the scratch folder): without `--project` the two texts are byte-equal; with `--project`,
   in the after text the `store` description of `start_run` is D1's new text, and with it replaced by D1's old text the after
   text is byte-equal to the before text. It also prints the number of tools and the SHA-256 of each text.

**D5. The Studio's calls.** No file under `apps/studio-web` is touched. The check is a reading: the five calls of ADR 0741
R5 still name `store` (`plan`, or the form's value) or `from_run`, and `git diff main --stat -- apps/` is empty. Because
every Studio start names `store`, the description does not decide the Studio's behaviour.

## Checks

Limits are exact. The source of every limit is the tolerance table's row "Bit-identity of default output"
(`docs/process/tolerances.md`) or the acceptance of F-862 itself; where the estimate equals the limit the margin is 1.

| # | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| 1 | Acceptance, the text: the live `store` property of `start_run` with `--project` equals the value of D3, with D1's new text; no "none with case" (`store_description_names_plan_as_the_default`) | exact value | F-862 acceptance; ADR 0741 answer, option (a) | equal | 1 | code verification |
| 2 | Acceptance, nothing else of the tools list: D4, without `--project` the before and after texts are equal bytes; with `--project` the after text with the old description put back equals the before text | equal bytes | F-862 acceptance; tolerance table, "Bit-identity of default output" | equal | 1 | unchanged default output |
| 3 | Acceptance, no golden changes: `git diff main --stat -- crates/rcfd-mcp/tests/golden` is empty, and the existing list tests pass unedited: `project_list_f270`, `tool_list_f275`, `tool_list_adr0730`, `default_f1` | empty; 0 failures | F-862 acceptance; rule 3 | 0 | 1 | unchanged default output |
| 4 | Acceptance, the Studio's calls unchanged: D5 (the five calls of ADR 0741 R5 read at their lines; `git diff main --stat -- apps/` empty) | stated; empty | F-862 acceptance | empty | 1 | unchanged default output |
| 5 | No behaviour change: the tests of `start_run`'s store, unedited, pass: `start_run_default_plan_adr0741` (2 tests), `new_simulations_f271` (1 test, 1 ignored record), and `--lib` of `rcfd-mcp` | 0 failures | rule 7; tolerance table, "Bit-identity of default output" | 0 | 1 | unchanged default output |
| 6 | The only source change is one line of `project.rs`: `git diff main --numstat -- crates/` shows `project.rs` with 1 line added and 1 removed, besides the new test file and its patch | as stated | the brief (files owned) | as stated | 1 | code verification |
| 7 | `cargo fmt -p rcfd-mcp --check`, `cargo clippy -p rcfd-mcp --all-targets -- -D warnings`, `cargo test -p rcfd-cli --release --test docs_site` | clean; 0 failures | rule 7 | clean | 1 | code verification |

No row is of kind validation: the acceptance claims no agreement with a measured case.

### Negative controls and planned mutations

Each in the working tree on the code commit, run once, restored, recorded in Addendum 1.

| Mutation | Must fail |
|---|---|
| M1 (committed, rule 16): the description back to the old text. Patch `crates/rcfd-mcp/tests/start_run_store_description_f862.mutation.patch`, first line `# expect-fail: cargo test --release -p rcfd-mcp --test start_run_store_description_f862` | row 1 |
| M2: the `enum` of `store` loses `"none"` (the text kept new) | row 1 (the exact value) and the D4 script (row 2) |
| M3: another text of the list changes, the description of `dry_run` gains one word | the D4 script (row 2); recorded whether `project_list_f270` catches it (by reading it does not: it removes `dry_run` after checking only its type) |

The negative control of the D4 script: given the after text with the description not put back, it must report exactly one
difference, at `start_run`'s `store` description.

## Size

Size (agent): done; 0; 0; 0; 110000

## Decision rule

- **done**: rows 1 to 7 are met and M1 is caught.
- **partial**: any row of 1 to 7 is missed or not run, recorded with what failed.
- **todo**: the text cannot be changed without changing a golden or another text of the list.

## What is not covered

- `docs/mcp-tools.md`, the `start_run` row, says "the tool's own description of `store` still names the earlier default
  until its change is approved". It is outside the files this brief gives (the description string and a new test file), so
  it is not edited here; it is named for the lead in the addendum.
- `npm test` of `apps/studio-web` is not run: nothing there is touched (rule 7).
- The tools list without `--project` has no `start_run`; row 2 checks it is unchanged.

## Addendum 1: results (2026-11-22)

Built as registered: the registration eaa78707, then the code commit 1c62c2bc (the one string of `project.rs` line 399 and
the new test file), then the committed mutation patch with this addendum. The before lists of D4 were written from the
binary of eaa78707 (the unchanged code) before the code commit. All runs in the release profile, each registered run made
once; the record (the four lists, the scripts and the logs) is in the scratch folder
`E:/projects/cfd/w49i-work/f862-start-run-store-description-scratch`. No amendment was needed.

| # | Check | Limit | Measured | Met |
|---|---|---|---|---|
| 1 | The live `store` property of `start_run` | exact value | `store_description_names_plan_as_the_default`: 1 passed; the property is `{"type": "string", "enum": ["plan", "none"], "description": <D1 new text>}`, no "none with case" | yes |
| 2 | The whole list before and after, less the one description | equal bytes | without `--project`: 34 tools, 28 104 bytes before and after, SHA-256 a68843af005758f5... both, equal; with `--project`: 53 tools, 53 944 bytes before (368ca07bc7018905...), 53 985 after (f1906f35903338a4...), the new text once, and with the old text put back equal bytes. The 41 bytes are the length of the new last sentence (83 characters) less the old (42) | yes |
| 3 | No golden changes; the list tests unedited | empty; 0 failures | no file under `crates/rcfd-mcp/tests/golden` changed; `project_list_f270` 1, `tool_list_f275` 8, `tool_list_adr0730` 2, `default_f1` 2 passed | yes |
| 4 | The Studio's calls | stated; empty | no file under `apps/` changed; read at their lines: `runs.ts` 147 `store: 'plan'`, 175 `store: p.store ?? 'plan'`, 211 `from_run`; `first.ts` 36 `store: 'plan'`; `control.ts` 304 `store: 'plan', dry_run: true` | yes |
| 5 | No behaviour change | 0 failures | `start_run_default_plan_adr0741` 2 passed, `new_simulations_f271` 1 passed (1 ignored record), `rcfd-mcp --lib` 18 passed (1 ignored, as before) | yes |
| 6 | The only source change | as stated | `crates/rcfd-mcp/src/project.rs` 1 added, 1 removed; `tests/start_run_store_description_f862.rs` 45 added; the patch | yes |
| 7 | fmt, clippy, docs_site | clean; 0 failures | `cargo fmt -p rcfd-mcp --check` clean; `cargo clippy -p rcfd-mcp --all-targets -- -D warnings` clean; `docs_site` 2 passed | yes |

### Mutations (each in the working tree on 1c62c2bc, run once, restored)

| Mutation | Registered to fail | Result |
|---|---|---|
| M1, committed as `crates/rcfd-mcp/tests/start_run_store_description_f862.mutation.patch` | row 1 | the new test failed at line 32 (the exact value) |
| M2, `enum` `["plan"]` | row 1 and the D4 script | the new test failed at line 32; the script: not met, one difference, `start_run.inputSchema.properties.store.enum` |
| M3, `dry_run`'s description gains "exactly" | the D4 script | the script: not met, one difference, `start_run.inputSchema.properties.dry_run.description`; `project_list_f270` and the new test passed, as read (neither holds that text) |

The negative control of the D4 script (the after list without the old text put back) reported exactly one difference,
`start_run.inputSchema.properties.store.description`.

### Deviations

- The registration names `git diff main` for rows 3, 4 and 6. The local branch `main` of this checkout is not the base of
  this work (it is at another line of history), so the diffs were taken against the base d6e44b27, the commit the brief
  was issued at.
- The first invocation of rows 3 and 5 stopped before any check of `start_run_default_plan_adr0741` ran: its two tests need
  `rcfd-runner.exe` next to `rcfd-mcp.exe`, which was not yet built in this target folder (the helper's message names it),
  and cargo stopped there, so `tool_list_*` and the new test were not reached in that invocation. After
  `cargo build --release -p rcfd-cli -p rcfd-runner` the whole set was run once with `--no-fail-fast`, with the results
  above. No code or test was changed between the two.
- The mutation runs M1 to M3 came before the run of rows 3 and 5; the binary was rebuilt from the clean code afterwards and
  its list is byte-equal to the after list of row 2.

### Not run, not done

- `docs/mcp-tools.md`, the `start_run` row, still ends its `store` clause with "the tool's own description of `store`
  still names the earlier default until its change is approved", now stale. Outside the files of the brief; for the lead.
- The comment at `apps/studio-web/src/shared/actions/runs.ts` line 146 ("start_run's own default (none)") is the Studio
  follow-up ADR 0741 proposed; not touched.
- `npm test` of `apps/studio-web`: not run, nothing there is touched.

### Size

Size (agent): done; 0; 0; 0; about 120000

### Decision

By the registered rule: **done**. Rows 1 to 7 are met and M1 is caught; every clause of the acceptance of F-862 has a
passing check (rows 1 to 4).

## Open questions for the maintainer

None.

## For the lead: roadmap entries

Status proposed for F-862: done.

> ADR 0774, 2026-11-22, done: the description of start_run's store argument now ends "Default plan; none is the way back to a run that stores only what its case asks for" (ADR 0741 question 1, option (a)), held by a test that reads the live tools list; the whole list before and after is byte-equal with the old text put back, no golden, argument or default changed and no Studio call changed.
