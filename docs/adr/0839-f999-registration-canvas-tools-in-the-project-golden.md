# ADR 0839: F-999 registration: the project tool-list golden holds show_view, read_view and export_view

Status: **REGISTERED 2026-11-27, written before any code of F-999 and before any run of its checks.** Before this text
nothing was changed, built or run but reading and searching. Results go in addenda at the end; nothing above them is
changed afterwards. Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead.

It answers the brief `docs/adr/briefs/f999-canvas-tools-in-golden.md` ([the file](briefs/f999-canvas-tools-in-golden.md))
and follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f999-canvas-tools-in-golden`
from f5480237 (origin/main at the start of this work, also the merge base), scratch
`w49i-work/f999-canvas-tools-in-golden-scratch`. The base of every diff below is f5480237.

## What was read

`AGENTS.md`, the rules (version 5) and the brief, whole. F-999's acceptance by a script. ADR 0838 whole
([the file](0838-f987-registration-show-view-description-names-every-form.md)), `crates/rcfd-mcp/tests/project_list_f270.rs`
whole, `crates/rcfd-mcp/src/project.rs` lines 505 to 560 (the three tools' entries), the `--canvas` handling of
`crates/rcfd-mcp/src/main.rs` lines 78 to 90, the history of the golden and of the test (`git log`, `git show 5eb2be45`
and `git show 43f126e0` for the test), A2 of [ADR 0185](0185-adr-0173-amendment-1-f270-registration-the-rcfd-workspace-crate.md)
(lines 177 to 179), D2 and finding 4 of [ADR 0805](0805-f976-registration-a-view-specification-and-show-view.md) (lines
67 to 69, 115 to 118), finding 5 and amendment 3 of
[ADR 0823](0823-f980-registration-a-view-exported-as-one-self-contained-html-page.md) (lines 80 to 87, 331 to 345), the
search of `crates/` for the golden's name. Bytes read: about 110 000.

Found by reading, before any code:

1. **Why the three were taken off the list.** The golden is the list of the binary of main 7c960bb, recorded before the
   project tools moved onto `rcfd-workspace`, and ADR 0185 A2 calls it "never re-recorded". Every change of the project
   list since (F-271 to F-848) was registered as a restatement of the test: the live list is taken back to the golden by
   removing what was added, and the golden is not edited. ADR 0805 D2 put show_view and read_view last "so the lead's
   restatement of `project_list_f270.rs` takes them off the end", and the lead's re-record of F-976 (5eb2be45) and of
   F-980 (43f126e0, "The JSON goldens are not edited") did exactly that, checking only their names, classes and place. So
   the reason is the rule that the golden is the 7c960bb list and is not edited, not a property of the three tools.
2. **What that reason asks of the change.** The three tools are listed whenever `--project` is given: `tools()` of
   `project.rs` lists them unconditionally and `--canvas` only registers the process as a canvas (`main.rs` lines 78 to
   90); the test starts the binary with `--project` alone and finds them. Nothing asks for a tool shown only with the
   canvas. What the change cannot keep is ADR 0185 A2's "never re-recorded"; that is the lead's decision of 2026-11-27 in
   the brief (on the maintainer's standing instruction of 2026-11-26), recorded here as such and not as my choice. To keep
   as much of A2 as the change allows, the golden's first 52 entries stay byte for byte the 7c960bb list (G1), and the
   test, after comparing the three, takes them off the golden as well, so every later comparison of the test runs against
   exactly the 52 entries it ran against before.
3. **Where the golden's order puts them.** In the live list they are the last three, after `empty_trash`, in the order
   `show_view`, `read_view`, `export_view`. The golden holds none of the tools that the live list has after `wait_run`
   (the trash tools are taken off the live list, not held in the golden), so the three go at the golden's end, after
   `wait_run`, in the live order. The golden becomes 55 entries.
4. Only `project_list_f270.rs` reads `tools_list_project_f270.json` (a search of `crates/` for the name finds that file
   only), so no other test sees the edit.
5. The golden's text is the serialisation of its value (the test asserts `serde_json::to_string(&gold) == GOLDEN`), with
   no trailing newline; it ends with the bytes `}]}` at 43 969 bytes.

## Design

### D1. The golden (the three entries only)

`crates/rcfd-mcp/tests/golden/tools_list_project_f270.json` at HEAD is f5480237's text with its last two bytes `]}`
replaced by `,` + E + `]}`, where E is the text of the three entries `show_view`, `read_view`, `export_view` joined by
`,`, each exactly as the live list gives it. E is made by a script in the scratch folder from the `tools/list` response of
the branch's release `rcfd-mcp --project` on a fresh project: the response line is parsed keeping key order, the parse is
re-serialised compactly and must equal the response's `result` text byte for byte (else the script stops and nothing is
written), and the last three entries are then serialised the same way. No entry is typed by hand. No other byte changes.

### D2. The test change (`crates/rcfd-mcp/tests/project_list_f270.rs`)

`gold` becomes mutable. In the two blocks that take the canvas tools off the live list (the export_view block of ADR 0823
and the show_view and read_view block of ADR 0805), each tool is now taken off the end of the golden too, its name
asserted there, and the live entry compared with the golden's entry as text, byte for byte, with the message
`<name> differs from the golden`. Every assertion those blocks make now (export_view last; show_view and read_view the
last two after it, their classes `write_files` and `read`; `empty_trash` before them) stays. After the two blocks the
golden holds its 52 entries of 7c960bb and every later line of the test is unchanged; the comparisons of the whole text
(`serde_json::to_string(&gold) == GOLDEN`, of the full 55 entries) and `gold.len() == 52` stay as they are. A paragraph
naming ADR 0839 and F-999 is added to the leading comment (rule 12); no other line of the comment changes. No tool's
text changes; no file under `crates/*/src` changes.

## Checks (rule 14)

Release profile, `CARGO_TARGET_DIR` under the scratch folder, `CARGO_BUILD_JOBS=8`.

| Id | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| A1 | `project_list_f270` (its one test) with the new golden and the test change | passes | F-999 acceptance ("project_list_f270 compares them") | pass | exact | code verification |
| G1 | a script: HEAD's golden minus its last 2 bytes starts with f5480237's golden minus its last 2 bytes; the remainder is `,` + E' + `]}` where E' parses to exactly 3 objects named `show_view`, `read_view`, `export_view`; HEAD's golden has 55 entries | as stated (43 967 bytes identical, 3 entries added, 0 changed) | F-999 acceptance ("every other entry byte-identical") | holds | exact | unchanged default output |
| G2 | `git diff --name-only f5480237 HEAD` | only ADR 0839, the golden, `project_list_f270.rs`, `project_list_f270.mutation.patch` and `docs/roadmap/proposals/0839-F-999.json`; nothing under `crates/*/src` | the brief's owned files; "no tool's text changes" | as stated | exact | unchanged default output |
| G3 | `git diff -U0 f5480237 HEAD -- crates/rcfd-mcp/tests/project_list_f270.rs` | only the comment paragraph, `let mut gold`, and lines inside the two canvas blocks | D2 ("every other comparison unchanged") | as stated | exact | unchanged default output |
| E1 | `tool_list_f275`, `tool_list_adr0730`, `default_f1`, `show_view_description_f987`, `canvas_view_f976` | every test passes | the brief's decision rule ("every tool-list test passes") | pass | exact | unchanged default output |
| L1 | `cargo test -p rcfd-mcp --release --lib` | passes | rule 7 | pass | exact | code verification |
| L2 | `cargo fmt -p rcfd-mcp --check` (or `rustfmt --check` of the touched file on os error 206); `cargo clippy -p rcfd-mcp --release --all-targets -- -D warnings` | clean | rule 7 | clean | exact | unchanged default output |
| L3 | `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0839-F-999.json`; `python scripts/process_report.py --check` | pass (the last: only a generated file not current may fail) | rules 7, 9 | pass | exact | unchanged default output |

No row is of kind validation: the acceptance claims no agreement with a measured case.

### Negative controls and mutations (rule 16)

Each on a scratch copy of the tree made by `git archive` into the scratch folder, with one target folder for these copies
under the scratch folder, running `project_list_f270` unless said.

| Id | Change | Expected |
|---|---|---|
| N1 | the new test with f5480237's golden (52 entries) | fails (the golden's last entry is `wait_run`) |
| N2 | f5480237's tree, unchanged test and golden, with M2 applied | passes: the gap of ADR 0838's M6 that this item closes |
| M1 | show_view's description: `Show a view in the Studio` becomes `Show the view in the Studio` | fails, `show_view differs from the golden` |
| M2 | read_view's description: `exists false` becomes `exists: false` (ADR 0838 M6) | fails, `read_view differs from the golden` |
| M3 | export_view's description: `Write a view as one` becomes `Write the view as one` | fails, `export_view differs from the golden` |
| M4 | export_view's property `out`: `exports/<name>.html"` becomes `exports/<name>.htm"` | fails, `export_view differs from the golden` |

M2 is committed as `crates/rcfd-mcp/tests/project_list_f270.mutation.patch` (made with `git diff`), first line
`# expect-fail: cargo test -p rcfd-mcp --release --target-dir target/mutation --test project_list_f270`.

## Size

Size (agent): done; 0; 0; 2; 110000

(Expected outcome done; no further registration; no heavy run; two questions for the maintainer, Q1 and Q2; bytes to
read.)

## Decision rule

Done for F-999 if A1, G1, G2, G3, E1, L1, L2 and L3 hold, N1 fails and N2 passes as stated, and M1 to M4 are each caught
by `project_list_f270`. Partial otherwise.

## Not covered

- The descriptions of the other tools that the test takes off the live list (the trash tools, `list_sites`,
  `case_facts`, `refine_case_mesh`, `derive_picture`, `hash_project_file`, `runner_config`, `set_runner_config`, and the
  properties added to `start_run`) stay caught by no golden; Q2.
- `tool_list_f275`, `project_w48f`, `project_runs_w48f` and `canvas_view_f976`, which pin counts or names, are not
  changed: the live list does not change.
- The Studio and its capability inventory are not touched.

## Open questions for the maintainer

- **Q1. ADR 0185 A2's "never re-recorded" (a reason the change cannot keep).** The golden was the list of main 7c960bb,
  never re-recorded; with the three entries it is that list plus three. (a) A2 is read from now as "the first 52 entries
  are the 7c960bb list, byte for byte", which G1 checks and the test keeps by taking the three off the golden after
  comparing them; (b) the three go in a golden file of their own and `tools_list_project_f270.json` stays as recorded.
  Recommendation: (a), as the acceptance names that file and the lead has taken it so in the brief.
- **Q2. The other tools added since 7c960bb (scope).** Their descriptions are caught by no test either, as the canvas
  tools' were. (a) leave them; (b) a follow-up item puts them in the golden in the same way (additive, first entries
  byte-identical, the test comparing each instead of taking it out). Recommendation: (b), as a new item for the lead to
  decide; not taken here, as it is outside F-999's acceptance.

## Addendum 1: results (2026-11-27)

Code commit 26961daf (the golden's three entries and the test change), mutation patch ff37f78c; the run was made once, in
the release profile, by `run.sh` in the scratch folder (`CARGO_BUILD_JOBS=8`, the scratch target folder). The three
entries were made by `make_golden.py` (D1) from the branch's release binary: the live list (58 tools, 63 036 bytes)
re-serialised equal to the response text, E is 4 093 bytes, the golden went from 43 969 to 48 063 bytes and from 52 to 55
entries. Logs: `build.log`, `build2.log`, `a1.log`, `e1.log`, `lib.log`, `fmt.log`, `clippy.log`, `mut_N1.log`,
`mut_N2.log`, `mut_M1.log` to `mut_M4.log`, `run.log` in the scratch folder.

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| A1 | passes | `project_list_f270` 1 passed, 0 failed | yes |
| G1 | 43 967 bytes identical, 3 entries added, 0 changed | prefix of 43 967 bytes identical, tail `,` + E' + `]}`, added `show_view`, `read_view`, `export_view`, 52 to 55 entries | yes |
| G2 | only the owned files, nothing under `crates/*/src` | the golden, the mutation patch, `project_list_f270.rs`, ADR 0839 (and the proposal file, committed with this addendum) | yes |
| G3 | the comment paragraph, `let mut gold`, lines inside the two canvas blocks | 4 comment lines added, the `let mut gold` line, `tools.pop()` replaced by 5 lines in the export_view block, 9 lines added in the show_view and read_view block; nothing else | yes |
| E1 | every test passes | `canvas_view_f976` 4, `default_f1` 2, `show_view_description_f987` 3, `tool_list_adr0730` 2, `tool_list_f275` 8: 19 passed, 0 failed | yes |
| L1 | passes | `rcfd-mcp --lib` 18 passed, 0 failed, 1 ignored (an existing record, not touched) | yes |
| L2 | clean | `cargo fmt -p rcfd-mcp --check` exit 0, no output; clippy `--release --all-targets -D warnings` exit 0 | yes |
| L3 | pass | in Addendum 2, run after this addendum is committed | Addendum 2 |

### Negative controls and mutations

| Id | Expected | Measured | Met |
|---|---|---|---|
| N1 | fails | exit 101 at line 139: the golden's last entry is `wait_run`, not `export_view` | yes |
| N2 | passes | exit 0, 1 passed: f5480237's test does not catch M2 | yes |
| M1 | caught by `project_list_f270` | exit 101, `"show_view" differs from the golden` | yes |
| M2 | caught by `project_list_f270` | exit 101, `"read_view" differs from the golden` | yes |
| M3 | caught by `project_list_f270` | exit 101, `export_view differs from the golden` | yes |
| M4 | caught by `project_list_f270` | exit 101, `export_view differs from the golden` | yes |

### Deviations and what was not run

One of form: in the show_view and read_view block the message prints the name as a JSON value, so it reads
`"show_view" differs from the golden` with quotes, not `show_view differs ...` as D2 wrote; what fails and why is as
registered. To start the binary, `make_golden.py` wrote a minimal project (the six folders and `rcfd-project.json` as
`Project::create` writes them) in the system temporary folder, which is left there. Not run: the whole `rcfd-mcp`
integration suite, `project_w48f` and `project_runs_w48f` (they count the tools; the live list does not change),
`baseline_hashes` (no solver file touched), `npm test` (no Studio file touched).

Size (actual): done; 0; 0; 2; 115000

### Decision

A1, G1, G2, G3, E1, L1 and L2 hold, N1 fails and N2 passes, and M1 to M4 are each caught by `project_list_f270`; with L3
passing (Addendum 2), **done** by the registered rule. Q1 and Q2 above stay open for the lead.

### For the lead: roadmap entries

> Status: done.

> Note: ADR 0839, 2026-11-27, done: tools_list_project_f270.json holds show_view, read_view and export_view after wait_run as the live list gives them (55 entries, the first 43 967 bytes as before), project_list_f270 compares each byte for byte, and a change to each description is caught (4 of 4 mutations).

The same entries are `docs/roadmap/proposals/0839-F-999.json`. The golden edit is additive (rule 3): the diff is the
three entries appended after `wait_run`, made by the lead's taking of the maintainer's word as the brief records it.
