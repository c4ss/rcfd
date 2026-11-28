# ADR 0842: F-1002 registration: start_run's live project-mode entry in a golden of its own

Status: **REGISTERED 2026-11-28, written before any code of F-1002 and before any run of its checks.** Before this text
nothing was changed, built or run but reading and searching. Results go in addenda at the end; nothing above them is
changed afterwards. Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead.

It answers the brief `docs/adr/briefs/f1002-start-run-golden.md` ([the file](briefs/f1002-start-run-golden.md)) and
follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f1002-start-run-golden` from
b7d5cdc6 (origin/main at the start of this work, also the merge base); scratch `w49i-work/f1002-start-run-golden-scratch`.
The base of every diff below is b7d5cdc6.

## What was read

`AGENTS.md`, the rules (version 5) and the brief, whole. F-1002's acceptance by a script.
[ADR 0841](0841-f1001-registration-every-added-tool-in-the-project-golden.md) whole, with its scratch script
`make_golden.py`. `golden_every_tool_f1001.rs` and its mutation patch, `start_run_store_description_f862.rs` and the head
of its patch, the head of `start_run_default_plan_adr0741.rs`, the helpers of `tests/common/mod.rs` (`project`, `Mcp`),
lines 30 to 90 and 385 to 470 of `crates/rcfd-mcp/src/project.rs` (the shared schemas and `start_run`'s entry), the list
of `crates/rcfd-mcp/tests/` and its `golden/` folder, the property names of `start_run` in the project golden (by a
script), and `git log -S` of each property in `crates/rcfd-mcp/src/project.rs`. Bytes read: about 75 000.

Found by reading, before any code:

1. **The eight properties and the commits that added them** to `start_run`'s schema (the first commit whose diff adds the
   property key in that entry, by `git log --reverse -S`):

   | Property | Commit | Record |
   |---|---|---|
   | `new_case` | 85440cab | F-271, ADR 0186 |
   | `store` | 85440cab; its text of now by F-862 (ADR 0774), default plan by ADR 0741 | F-271, ADR 0186 |
   | `continue_from` | df3e808a | ADR 0734 |
   | `max_steps` | df3e808a | ADR 0734 |
   | `from_run` | df3e808a | ADR 0734 |
   | `run_time` | 95d3cb7d | ADR 0737 |
   | `dry_run` | 95d3cb7d | ADR 0737 (the hit of 0d8b5ba6 is another tool's `dry_run`) |
   | `site` | e275ee6b | ADR 0739, F-848 |

   `start_run` in the project golden (`tools_list_project_f270.json`) holds only `case`, `mem_mb`, `priority`, `set` and
   `threads`, as at 7c960bb; `reason` (ADR 0188) is the shared `reason_schema()`, held transitively by ADR 0841.
2. **What the entry draws from elsewhere.** `run_time` is `run_time_schema()`, shared with `case_facts`; `new_case`'s
   `properties` are `new_case_schema()`'s, shared with the tool `new_case`, and hold the enums of the shipped examples and
   templates. So the new golden changes whenever those do, as the project golden's `new_case` and `case_facts` entries
   already do (Q1).
3. **Unique strings.** Each of the eight mutation strings below occurs once in `crates/rcfd-mcp/src/project.rs`
   (`grep -c -F`, each 1).
4. **Serialisation.** The live `tools/list` result re-serialises compactly to the response text byte for byte (ADR 0841,
   asserted by `golden_every_tool_f1001`), so an entry's compact serialisation is the text the binary gives.

## Design

### D1. The golden

`crates/rcfd-mcp/tests/golden/start_run_project_f1002.json`: `start_run`'s entry of the live project-mode `tools/list`
result, compactly serialised in the binary's key order, as it stands in the response text, with no trailing newline. It
is written by `make_golden.py` in the scratch folder from the `tools/list` response of the branch's release
`rcfd-mcp --project` on a minimal project made in the scratch folder: the result is parsed keeping key order, must
re-serialise to the response text byte for byte, must hold exactly one `start_run` whose `inputSchema.properties` holds
the eight properties of finding 1, each with a `description`, and the entry's text must occur in the response text;
else the script stops and writes nothing. No byte is typed by hand. No other golden changes.

### D2. The test, `crates/rcfd-mcp/tests/start_run_golden_f1002.rs`

One test, header `ADR 0842` and `F-1002` (rule 12): it starts `rcfd-mcp --project` on a fresh project (the `common`
helpers, as F-862's test), takes `tools/list`, and asserts (1) the golden parses and re-serialises to itself; (2) its
`name` is `start_run` and its properties hold the eight names, each with a `description`; (3) for each of the eight, the
live property equals the golden's as text, with the message `<property> differs from the golden`; (4) the live entry's
compact text equals the golden byte for byte, with the message `start_run differs from the golden`. The committed patch
lies beside it (D3). No tool's text changes; no file under `crates/*/src` changes.

### D3. The committed mutation

M4 below, made with `git diff`, as `crates/rcfd-mcp/tests/start_run_golden_f1002.mutation.patch`, first line
`# expect-fail: cargo test -p rcfd-mcp --release --target-dir target/mutation --test start_run_golden_f1002`.

## Checks (rule 14)

Release profile, `CARGO_TARGET_DIR` under the scratch folder, `CARGO_BUILD_JOBS=8`.

| Id | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| A1 | `start_run_golden_f1002` | passes | F-1002 acceptance ("recorded in a golden file of its own and compared byte for byte by a test") | pass | exact | code verification |
| A2 | `make_golden.py`'s assertions (D1) when it writes the golden | all hold, the golden written | F-1002 acceptance ("the live project-mode entry ... with the properties ...") | hold | exact | code verification |
| G1 | `git diff --name-status b7d5cdc6 HEAD` | only `A` of ADR 0842, the golden, the test, its mutation patch and `docs/roadmap/proposals/0842-F-1002.json`; no `M` or `D`; nothing under `crates/*/src`; so every existing golden byte-identical | the brief (owned files; "every existing golden is byte-identical") | as stated | exact | unchanged default output |
| E1 | `project_list_f270`, `golden_every_tool_f1001`, `tool_list_f275`, `tool_list_adr0730`, `default_f1`, `show_view_description_f987`, `canvas_view_f976`, `start_run_store_description_f862` | every test passes | the brief's decision rule ("every tool-list test passes") | pass | exact | unchanged default output |
| L1 | `cargo test -p rcfd-mcp --release --lib` | passes | rule 7 | pass | exact | code verification |
| L2 | `cargo fmt -p rcfd-mcp --check` (or `rustfmt --check` of the touched file on os error 206); `cargo clippy -p rcfd-mcp --release --all-targets -- -D warnings` | clean | rule 7 | clean | exact | unchanged default output |
| L3 | `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0842-F-1002.json`; `python scripts/process_report.py --check` | pass (the last: only a generated file not current may fail) | rules 7, 9 | pass | exact | unchanged default output |

No row is of kind validation: the acceptance claims no agreement with a measured case. Every limit is exact (pass or
fail), from the acceptance, the brief or a rule; no number is estimated.

### Negative controls and mutations (rule 16)

On scratch copies made by `git archive` into the scratch folder (one copy of b7d5cdc6, one of the code commit), with one
target folder for these copies under the scratch folder; each change is applied to the copy, the tests named run, and the
changed file restored from a saved copy before the next. Each mutation replaces one string of
`crates/rcfd-mcp/src/project.rs` (finding 3). "Caught" means `start_run_golden_f1002` fails with
`"<property>" differs from the golden` (the name printed as a JSON value or plain) for the mutated property.

| Id | Tree | Change | Expected |
|---|---|---|---|
| N1 | b7d5cdc6 | M4's change | `project_list_f270`, `golden_every_tool_f1001` and `start_run_store_description_f862` each pass: the gap this item closes |
| N2 | the code commit, the golden replaced by the `start_run` entry of `tools_list_project_f270.json` (7c960bb's) | none | `start_run_golden_f1002` fails |
| M1 | the code commit | `new_case`: `a new case made first` to `a new case created first` | caught |
| M2 | the code commit | `store`: `about 100 snapshots, f32` to `about 100 snapshots in f32` | caught |
| M3 | the code commit | `continue_from`: `starts from that ended run` to `starts from that finished run` | caught |
| M4 | the code commit | `max_steps`: `at most this many steps` to `at most this number of steps` | caught |
| M5 | the code commit | `from_run`: `rerun that run id from its case.snapshot` to `rerun that run from its case.snapshot` | caught |
| M6 | the code commit | `run_time`: `set the run's time step` to `set the run's time steps` | caught |
| M7 | the code commit | `dry_run`: `and write nothing (ADR 0737)` to `and writes nothing (ADR 0737)` | caught |
| M8 | the code commit | `site`: `its folder here is kept up to date from the site` to `its folder here is kept current from the site` | caught |

M4 is the committed patch (D3).

## Size

Size (agent): done; 0; 0; 1; 75000

(Expected outcome done; no further registration; no heavy run; one question for the maintainer, Q1; bytes to read.)

## Decision rule

Done for F-1002 if A1, A2, G1, E1, L1, L2 and L3 hold, N1 passes and N2 fails as stated, and M1 to M8 are each caught by
`start_run_golden_f1002`. Partial otherwise.

## Not covered

- The project golden's own `start_run` entry stays 7c960bb's; the new golden is beside it, not in place of it.
- Properties' texts drawn from shared schemas (finding 2) are held here as part of `start_run`; their other uses are held
  by ADR 0841's tests.
- The behaviour of each property (what a run does with it) is held by the tests of its own records, not by this golden.
- The Studio and its capability inventory are not touched.

## Open questions for the maintainer

- **Q1. The golden's reach (scope).** The live entry holds, through `new_case_schema()`, the enums of the shipped
  examples and templates, and `run_time_schema()`'s text; a new example or a change to `case_facts`'s run-time text
  changes this golden too, so that edit needs the golden re-recorded by the lead. (a) keep the whole live entry, as the
  acceptance says ("the live project-mode entry"); (b) record only the eight properties' descriptions. Recommendation:
  (a); this work builds (a), which is the acceptance's wording; the question stays open only for whether the coupling is
  wanted.

## Addendum 1: results (2026-11-28)

Code commit ee9fa8d8 (the golden and the test), mutation patch c7c0b881. The golden was written by `make_golden.py` (D1)
from the branch's release binary: 58 live tools, the `start_run` entry 4 125 bytes with 14 properties (`case`,
`continue_from`, `dry_run`, `from_run`, `max_steps`, `mem_mb`, `new_case`, `priority`, `reason`, `run_time`, `set`,
`site`, `store`, `threads`, the binary's key order). `cargo fmt -p rcfd-mcp --check` was run once on the new code before
the commit, as the lead's brief asked (exit 0). The registered run was made once, in the release profile, by `run.sh` in
the scratch folder (`CARGO_BUILD_JOBS=8`, the scratch target folders), with `mut.sh` at its end; its whole output is
`run.log` there, with `a1.log`, `e1.log`, `lib.log`, `fmt.log`, `clippy.log` and `mut_<id>.log`.

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| A1 | passes | `start_run_golden_f1002` 1 passed, 0 failed | yes |
| A2 | all hold, the golden written | every assertion held; 4 125 bytes written, no trailing newline | yes |
| G1 | only additions of the owned files | `A` of the golden, the test, its patch and ADR 0842 (the proposal file is added with this addendum); no `M` or `D`, nothing under `crates/*/src` | yes |
| E1 | every test passes | `canvas_view_f976` 4, `default_f1` 2, `golden_every_tool_f1001` 1, `project_list_f270` 1, `show_view_description_f987` 3, `start_run_store_description_f862` 1, `tool_list_adr0730` 2, `tool_list_f275` 8: 22 passed, 0 failed | yes |
| L1 | passes | `rcfd-mcp --lib` 18 passed, 0 failed, 1 ignored (an existing record, not touched) | yes |
| L2 | clean | `cargo fmt -p rcfd-mcp --check` exit 0; clippy `--release --all-targets -D warnings` exit 0 | yes |
| L3 | pass | in Addendum 2, run after this addendum is committed | Addendum 2 |

### Negative controls and mutations

One `git archive` copy per tree (`copy-base` of b7d5cdc6, `copy-code` of c7c0b881); each change applied, the test run,
the file restored from a saved copy before the next (the same form as ADR 0841's, one copy per tree).

| Id | Expected | Measured | Met |
|---|---|---|---|
| N1 | the three tests pass | `project_list_f270`, `golden_every_tool_f1001`, `start_run_store_description_f862` each exit 0, 1 passed: b7d5cdc6 does not catch M4 | yes |
| N2 | fails | exit 101 at line 32 (`new_case has no description in the golden`: 7c960bb's entry lacks the eight) | yes |
| M1 `new_case` | caught | exit 101, `new_case differs from the golden` | yes |
| M2 `store` | caught | exit 101, `store differs from the golden` | yes |
| M3 `continue_from` | caught | exit 101, `continue_from differs from the golden` | yes |
| M4 `max_steps` | caught | exit 101, `max_steps differs from the golden` | yes |
| M5 `from_run` | caught | exit 101, `from_run differs from the golden` | yes |
| M6 `run_time` | caught | exit 101, `run_time differs from the golden` | yes |
| M7 `dry_run` | caught | exit 101, `dry_run differs from the golden` | yes |
| M8 `site` | caught | exit 101, `site differs from the golden` | yes |

### Deviations and what was not run

The form of the mutation copies above. Not run: the whole `rcfd-mcp` integration suite, `baseline_hashes` (no solver
file touched), `npm test` (no Studio file touched).

Size (actual): done; 0; 0; 1; 80000

### Decision

A1, A2, G1, E1, L1 and L2 hold, N1 passes, N2 fails, and M1 to M8 are each caught by `start_run_golden_f1002`; L3 is in
Addendum 2. By the registered rule, **done** if L3 holds there. The new golden is an added file: no existing golden
byte changes; it was written by this work's script from the live list, on the lead's taking of the maintainer's word as
the brief records it. Q1 stays open for the lead.

### For the lead: roadmap entries

> Status: done.

> Note: ADR 0842, 2026-11-28, done: golden/start_run_project_f1002.json holds start_run's live project-mode entry (4 125 bytes, 14 properties, written by script from the live list), start_run_golden_f1002 compares the eight properties added since 7c960bb one by one and the entry byte for byte, and 8 of 8 description mutations are caught; every existing golden is unchanged.

The same entries are `docs/roadmap/proposals/0842-F-1002.json`.

## Addendum 2: L3 (2026-11-28)

Run once at 6f779053, after Addendum 1's commit, by one command in the scratch folder (`run2.log`, `docs_site.log`).

| Id | Limit | Measured | Met |
|---|---|---|---|
| L3 | pass (process report: only a generated file not current may fail) | `docs_site` 2 passed, 0 failed; `roadmap_proposals.py check docs/roadmap/proposals/0842-F-1002.json` exit 0 ("F-1002: todo -> done; note + 368 characters"); `process_report.py --check` 1 failure, only "generated file is not current: docs/adr/index.md", left to the lead (rule 9) | yes |

Every registered check holds, N1 passes, N2 fails and M1 to M8 are caught: by the registered rule F-1002 is **done**.
