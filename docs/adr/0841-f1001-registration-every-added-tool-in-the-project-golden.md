# ADR 0841: F-1001 registration: the project tool-list golden holds every tool added since 7c960bb

Status: **REGISTERED 2026-11-27, written before any code of F-1001 and before any run of its checks.** Before this text
nothing was changed, built or run but reading and searching. Results go in addenda at the end; nothing above them is
changed afterwards. Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead.

It answers the brief `docs/adr/briefs/f1001-golden-every-tool.md` ([the file](briefs/f1001-golden-every-tool.md)) and
follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f1001-golden-every-tool` from
d99e9283 (origin/main at the start of this work, also the merge base); scratch `w49i-work/f1001-golden-every-tool-scratch`.
The base of every diff below is d99e9283.

## What was read

`AGENTS.md`, the rules (version 5) and the brief, whole. F-1001's acceptance by a script.
[ADR 0839](0839-f999-registration-canvas-tools-in-the-project-golden.md) whole, with its scratch scripts `make_golden.py`,
`g1.py`, `mut.sh` and `run.sh`. `crates/rcfd-mcp/tests/project_list_f270.rs` whole. `tools()` of
`crates/rcfd-mcp/src/project.rs` (lines 89 to 560, the name and description lines), `all_tools` of
`crates/rcfd-mcp/src/lib.rs` (lines 699 to 712), the mutation-patch discovery of `scripts/push_main.py` (lines 380 to
410), the header of `start_run_store_description_f862.rs`, the list of `crates/rcfd-mcp/tests/`, and `git log -S` of each
tool's name in `crates/rcfd-mcp/src`. Bytes read: about 120 000.

Found by reading, before any code:

1. **The tools the test takes off the live list.** Besides the three canvas tools of ADR 0839, `project_list_f270`
   removes or filters out of the live list 19 tools that the golden (the 52 entries of 7c960bb and the 3 of F-999) does
   not hold. Each with the commit that added it to `crates/rcfd-mcp/src/project.rs` (the first commit whose diff adds
   `name: "<tool>"`):

   | Group | Tools | Commit |
   |---|---|---|
   | F-271 (ADR 0186) | `new_case` | 85440cab |
   | F-272 (ADR 0187) | `check_case_mesh`, `preview_mesh`, `import_mesh`, `list_meshes` | 08ece42c |
   | F-273 (ADR 0188) | `read_feed` | 1897fc24 |
   | ADR 0733 | `runner_config`, `set_runner_config` | ebe02f62 |
   | ADR 0734 | `hash_project_file` | df3e808a |
   | ADR 0736 | `list_trash`, `trash_run`, `trash_results`, `trash_case`, `restore_trash`, `empty_trash` | d79e11dd |
   | ADR 0738 | `derive_picture` | 8bba78f2 |
   | ADR 0737 | `case_facts`, `refine_case_mesh` | 95d3cb7d |
   | ADR 0739 | `list_sites` | e275ee6b |

   `new_case`, the four mesh tools and `read_feed` are not removed by name but filtered out of the restated list, and
   the whole-text comparison at the end inserts the live entries of `new_case` and the mesh tools into the wanted list,
   so their text is compared with itself; none of the 19 is compared with a recorded text.
2. **None must stay off.** `project::tools()` lists all of them unconditionally, and `all_tools` appends that list
   whenever a project is open (`lib.rs` lines 699 to 712); the test starts the binary with `--project` on a fresh
   project and asserts each is listed. The 16 session tools of ADR 0195 P1 are not taken off the live list (they are
   absent from it and put back from `tools_list_default.json`), so they are outside this item.
3. **Live order.** Project tools follow the session tools in the live list, in the order of `project::tools()`:
   `derive_picture`, `read_feed`, `hash_project_file`, `new_case`, `check_case_mesh`, `preview_mesh`, `import_mesh`,
   `list_meshes`, `runner_config`, `set_runner_config`, `case_facts`, `refine_case_mesh`, `list_sites`, `list_trash`,
   `trash_run`, `trash_results`, `trash_case`, `restore_trash`, `empty_trash`. This is the order they are appended in.
4. **What is not a tool.** Properties added to tools the golden already holds stay outside a byte comparison: `start_run`
   gains `new_case`, `store`, `continue_from`, `max_steps`, `from_run`, `run_time`, `dry_run` and `site`; six held tools
   gain `reason`. Their golden entries are among the first 55, which stay byte-identical, and a second entry of the same
   name would make the golden no longer a tool list. `reason` is held transitively once `new_case` and `import_mesh` are
   in the golden: the test asserts the eight `reason` properties are equal. `store`'s description is held by
   `start_run_store_description_f862.rs`. The other seven properties of `start_run` are Q1.
5. **The committed mutation patch.** `project_list_f270.mutation.patch` exists (ADR 0839's M2), and `push_main.py`
   finds one patch per test file (`<stem>.mutation.patch`). This work does not change that file; its committed patch
   goes beside a new test file (D3).
6. The golden at d99e9283 is 48 063 bytes, 55 entries, the serialisation of its value with no trailing newline, ending
   in `}]}`; only `project_list_f270.rs` reads it.

## Design

### D1. The golden (19 entries appended)

`crates/rcfd-mcp/tests/golden/tools_list_project_f270.json` at HEAD is d99e9283's text with its last two bytes `]}`
replaced by `,` + E + `]}`, where E is the 19 entries of finding 1, in the live order of finding 3, joined by `,`, each
exactly as the live list gives it. E is made by `make_golden.py` in the scratch folder from the `tools/list` response of
the branch's release `rcfd-mcp --project` on a minimal project made in the scratch folder: the response's `result` is
parsed keeping key order, re-serialised compactly and must equal the response text byte for byte, the live list must
hold 58 tools and its names in the order of finding 3, else the script stops and writes nothing. No entry is typed by
hand. No other byte changes; the first 48 061 bytes (the 55 entries) stay as they are.

### D2. The test change (`crates/rcfd-mcp/tests/project_list_f270.rs`)

A constant `ADDED: [&str; 19]` with the names of finding 3. One block is inserted right after the two serialisation
assertions and before the export_view block: it asserts the golden's entry 55 (index 54) is `export_view` and the names
of the entries after it are `ADDED`; for each of them it finds the live entry of the same name in the live list as the
binary gave it (before any block changes it), asserts it is listed and at a later place than the one before, and
compares the two as text byte for byte with the message `<name> differs from the golden`; then it truncates the golden
to its 55 entries. The live list is not changed there: the existing blocks take each tool off the live list as they do
now. From the export_view block on, every line of the test is unchanged (the golden they see is the 55 entries they saw
before). A paragraph naming ADR 0841 and F-1001 is added to the leading comment (rule 12). No tool's text changes; no
file under `crates/*/src` changes.

### D3. A new test, `crates/rcfd-mcp/tests/golden_every_tool_f1001.rs`

One test, header `ADR 0841` and `F-1001`: it starts `rcfd-mcp --project` on a fresh project as `project_list_f270` does
and asserts (1) every tool of the live list has its name in the golden; (2) every entry of the golden from index 52 on
(the 22 added after 7c960bb) is equal as text, byte for byte, to the live entry of its name, with the message
`<name> differs from the golden`. It is the check that the golden holds every tool, and the file beside which the
committed patch lies (finding 5).

## Checks (rule 14)

Release profile, `CARGO_TARGET_DIR` under the scratch folder, `CARGO_BUILD_JOBS=8`.

| Id | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| A1 | `project_list_f270` with the new golden and the test change | passes | F-1001 acceptance ("compared by the test") | pass | exact | code verification |
| A2 | `golden_every_tool_f1001` | passes | F-1001 acceptance ("every tool ... is in tools_list_project_f270.json as the live list gives it") | pass | exact | code verification |
| G1 | `g1.py`: HEAD's golden minus its last 2 bytes starts with d99e9283's golden minus its last 2 bytes; the remainder is `,` + E' + `]}` with E' 19 objects named as finding 3 in that order; 74 entries | 48 061 bytes identical, 19 entries added, 0 changed | F-1001 acceptance ("every earlier entry byte-identical"); the brief (first 55) | holds | exact | unchanged default output |
| G2 | `git diff --name-only d99e9283 HEAD` | only ADR 0841, the golden, `project_list_f270.rs`, `golden_every_tool_f1001.rs`, `golden_every_tool_f1001.mutation.patch`, `docs/roadmap/proposals/0841-F-1001.json`; nothing under `crates/*/src` | the brief's owned files | as stated | exact | unchanged default output |
| G3 | `git diff -U0 d99e9283 HEAD -- crates/rcfd-mcp/tests/project_list_f270.rs` | only added lines: the comment paragraph, `ADDED`, the D2 block; no line removed | D2; the brief ("every other assertion unchanged") | as stated | exact | unchanged default output |
| E1 | `tool_list_f275`, `tool_list_adr0730`, `default_f1`, `show_view_description_f987`, `canvas_view_f976`, `start_run_store_description_f862` | every test passes | the brief's decision rule ("every tool-list test passes") | pass | exact | unchanged default output |
| L1 | `cargo test -p rcfd-mcp --release --lib` | passes | rule 7 | pass | exact | code verification |
| L2 | `cargo fmt -p rcfd-mcp --check` (or `rustfmt --check` of the touched files on os error 206); `cargo clippy -p rcfd-mcp --release --all-targets -- -D warnings` | clean | rule 7 | clean | exact | unchanged default output |
| L3 | `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0841-F-1001.json`; `python scripts/process_report.py --check` | pass (the last: only a generated file not current may fail) | rules 7, 9 | pass | exact | unchanged default output |

No row is of kind validation: the acceptance claims no agreement with a measured case.

### Negative controls and mutations (rule 16)

Each on a scratch copy of the tree made by `git archive` into the scratch folder, with one target folder for these copies
under the scratch folder. Each mutation replaces one string of `crates/rcfd-mcp/src/project.rs` that occurs there once
(checked before this text was written), and both `project_list_f270` and `golden_every_tool_f1001` are run; "caught"
means the named test fails with `<name> differs from the golden` for the mutated tool.

| Id | Tree | Change | Expected |
|---|---|---|---|
| N1 | the code commit, with d99e9283's golden (55 entries) | none | both tests fail |
| N2 | d99e9283 (the old test and golden) | M9's change | `project_list_f270` passes: the gap this item closes |
| M1 | the code commit | `new_case`: `Create a new case file cases/<name>.toml` to `Create the new case file cases/<name>.toml` | caught by both |
| M2 | the code commit | `preview_mesh`: `PNG image of a project case's mesh` to `PNG picture of a project case's mesh` | caught by both |
| M3 | the code commit | `read_feed`: `who changed what and why.` (in `The project's change feed: who changed what and why.`) to `who changed what, and why.` | caught by both |
| M4 | the code commit | `set_runner_config`: `Applies at the next runner start` to `Applies at the next start of the runner` | caught by both |
| M5 | the code commit | `hash_project_file`: `The hash of a project file or folder` to `The hash of a file or folder` | caught by both |
| M6 | the code commit | `empty_trash`: `Remove items of the project trash for good` to `Remove items of the project trash forever` | caught by both |
| M7 | the code commit | `derive_picture`: `A derived picture of a stored snapshot` to `A derived image of a stored snapshot` | caught by both |
| M8 | the code commit | `refine_case_mesh`: `Refine x2 (ADR 0737): double every cell count` to `Refine x2 (ADR 0737): double each cell count` | caught by both |
| M9 | the code commit | `list_sites`: `The run sites of the user's sites file` to `The run sites of the sites file` | caught by both |

One mutation per added group of finding 1. M9 is committed as `crates/rcfd-mcp/tests/golden_every_tool_f1001.mutation.patch`
(made with `git diff`), first line
`# expect-fail: cargo test -p rcfd-mcp --release --target-dir target/mutation --test golden_every_tool_f1001`.

## Size

Size (agent): done; 0; 0; 1; 120000

(Expected outcome done; no further registration; no heavy run; one question for the maintainer, Q1; bytes to read.)

## Decision rule

Done for F-1001 if A1, A2, G1, G2, G3, E1, L1, L2 and L3 hold, N1 fails and N2 passes as stated, and M1 to M9 are each
caught by `project_list_f270` and by `golden_every_tool_f1001`. Partial otherwise.

## Not covered

- The seven properties of `start_run` other than `store` (finding 4) stay caught by no recorded text; Q1.
- The class change of `design_run_study`, `design_evaluate` and `design_run_operating_map` under `--project` (ADR 0196 B,
  ADR 0200 S9) is asserted by the test as now and is not a text in the golden.
- `tool_list_f275`, `project_w48f`, `project_runs_w48f` and `canvas_view_f976`, which pin counts or names, are not
  changed: the live list does not change.
- The Studio and its capability inventory are not touched.

## Open questions for the maintainer

- **Q1. The properties added to `start_run` (scope).** `new_case`, `continue_from`, `max_steps`, `from_run`, `run_time`,
  `dry_run` and `site` are compared by no recorded text; the golden's `start_run` is 7c960bb's and stays so. (a) leave
  them; (b) a follow-up item records the live `start_run` entry in a golden file of its own and compares it byte for
  byte, as F-862 does for `store`. Recommendation: (b), as a new item for the lead to decide; not taken here, as F-1001's
  acceptance names tools, not properties.
