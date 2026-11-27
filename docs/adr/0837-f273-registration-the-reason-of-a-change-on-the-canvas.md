# ADR 0837: F-273 registration: the reason of a change shown with who and what, in the process view's rows and the activity line

Status: **REGISTERED 2026-11-27, written before any code of F-273 and before any run of its checks.** Before this text
nothing was changed, built or run but reading. Results go in addenda at the end; nothing above them is changed
afterwards. Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead.

It answers the brief `docs/adr/briefs/f273-reason-on-canvas.md` ([the file](briefs/f273-reason-on-canvas.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f273-reason-on-canvas` from 476de79b.
Scratch `E:/projects/cfd/w49i-work/f273-reason-on-canvas-scratch`.

## What was read

`AGENTS.md`, the rules (version 5) and the brief whole; F-273's acceptance and gap (by a script); of
[ADR 0188](0188-adr-0173-amendment-4-f273-registration-project-journal-and-change-feed.md) J4 to J7, C3, C3-St and the
lead's reading; `apps/studio-web/src/renderer/project/activity.ts` whole; of `crates/rcfd-workspace/src/feed.rs`
`journal_text` and `journal_entry`; of `journal.rs` `MAX_REASON` and `reason_arg`; of `crates/rcfd-workspace/src/view.rs`
`journal_step`, `run_steps` and the test `p1_f978_...`; of `Canvas.tsx` the header, `ProcessPanel`, `stepFacts` and
`ChangeLine`; of `claudeCode.tsx` `ClaudeActivity`; `ChangeEntry` and `ProcessStep` of `src/shared/project.ts`; the heads
and the activity test of `tests/e2e/claude-code.spec.ts`, the head, `listed` and E1/E3 of `tests/e2e/canvas-f978.spec.ts`
(with [ADR 0826](0826-f978-registration-the-process-view-of-a-case.md)'s limit and
[ADR 0828](0828-f978-registration-process-view-rows-and-runner-teardowns.md)'s `endProjectRunner`). Bytes read: about
95 000.

What is there now. The journal keeps `reason` (J4, at most 500 characters, a string, blank not recorded). The feed's
entry carries it (`Change.reason`, strings only) and its `text` ends `; why: <reason>`. The process view's journal step
copies the line's `reason` as it is, of any JSON type. The canvas's process row prints `by <client>` and ` : <reason>`
only when one of them is there, with no form for a missing reason and no cut; a non-string `reason` in a hand-written line
would reach React as an object. The header's activity line (`ClaudeActivity`) shows the verb and the thing, and the
reason only inside its `title` attribute: no view text shows why. F-979's `ChangeLine` of the live panels shows the reason
and is not changed here.

## Design

* **D1 (rows, engine).** `journal_step` takes `reason` from the feed's entry of the line (`feed::journal_entry(line).reason`,
  a string or nothing) instead of the raw line. Every line the tools write has a string reason or none (J4), so the rows
  of every journal the commands make are byte for byte what they were; only a hand-written non-string `reason` is now
  left out, as the feed leaves it out. Additive only, no new store: the rows and the activity line read the feed's
  fields. No other engine change, no solver file, no golden.
* **D2 (the words, one function).** `whyOf(reason: unknown)` in `activity.ts`: given when `reason` is a string with a
  character other than white space; then the text is the reason, cut after 500 Unicode code points (ADR 0188 J4's
  `MAX_REASON`, counted as Rust's `chars()` counts) with `…` added, `cut` true. Absent (null, undefined, not a string,
  blank): the text `not given`, `given` false. The words shown are `why: <text>`.
* **D3 (escape).** The reason is put in the page only as a React text child (no `innerHTML`), so markup in it shows as
  its characters and makes no element.
* **D4 (the process row).** Each journal step (`source` `journal`) of `ProcessPanel` gets one line `p.cv-change`:
  `by <client> (pid N)` when the step has an actor (pid when it has one), then `; why: <text>` always; hooks
  `data-actor`, `data-pid`, `data-why` (`given` or `none`), `data-reason-cut` when cut. Run steps keep what they have.
  The step's button (the engine's `text`) is unchanged (F-978's E1 compares it).
* **D5 (the activity line).** After the thing, a span `.ph-activity-why` with the text `; why: <text>`, hooks `data-why`
  and `data-reason-cut`; the line gains `data-actor` and `data-pid` from the entry's actor. Who is "Claude Code" (the
  client `mcp`, as before), what is the verb and the thing (as before), why is D2's text.

Files: `crates/rcfd-workspace/src/view.rs` (`journal_step`, one unit test), `apps/studio-web/src/renderer/project/activity.ts`
(`whyOf`), `apps/studio-web/src/renderer/project/claudeCode.tsx` (the activity line, `ClaudeActivity` only: it is the
activity line's renderer, which the brief names by its role), `apps/studio-web/src/renderer/canvas/Canvas.tsx`
(`ProcessPanel`), new tests `tests/unit/reason-f273.spec.ts`, `tests/e2e/reason-f273.spec.ts` with its mutation patch,
this ADR and `docs/roadmap/proposals/0837-F-273.json`.

## Checks

The e2e test (packaged Studio, quick-start copy open, the test is the MCP client: it spawns `rcfd-mcp --project`): S1
`new_case` with a reason; S2 `edit_case` without one; S3 `edit_case` with the reason `<b>bold</b> & "q" <i>`; S4
`start_run` (max_steps 5) with a reason; then `show_view` of the case's process; then lines appended by the test to
`.rcfd/edits.jsonl` with actor `mcp` and the server's pid (what no tool writes): S5 a reason of 600 code points, S6 a
`reason` 42. The runner S4 starts is ended in the teardown by `endProjectRunner`. Time is from the tool's result (or the
append) to the page showing it. L = 4 × `FEED_POLL_MS` = 1.6 s, ADR 0188 C3-St (4 Studio periods) as ADR 0826 registers it.

| # | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| A1 | Acceptance "the Studio ... shows who changed what and why", activity line: after each of S1 to S4 the line shows "Claude Code", `data-actor` `mcp`, `data-pid` the server's pid, the verb and thing of `activityOf`, and `why: <reason>` (S2: `why: not given`, `data-why` `none`) | all 4 hold, each within L | F-273 acceptance; ADR 0188 C3-St | 0.5 s | 3.2 | code verification |
| A2 | Same acceptance, process view: within L of `show_view`'s result every journal step of the case shows `by mcp (pid <server pid>)` and `why:` its line's reason, or `not given` for S2's line; as many such rows as the case's journal lines | all rows, within L | F-273 acceptance; ADR 0826 E1 | 0.5 s | 3.2 | code verification |
| A3 | Escape: S3's reason is the exact text of the why in the activity line and its row; no `b` or `i` element in either | exact; 0 elements | F-273 acceptance (the reason as the caller said) | holds | n/a | code verification |
| A4 | Cut: S5 in the activity line and in its new row (within L of the append): the why is the first 500 code points then `…`, `data-reason-cut` set | exact, within L | ADR 0188 J4 (500 characters) | 0.5 s | 3.2 | code verification |
| A5 | Negative control: S6 shows `why: not given` in the line and its row, and the rows are one more than before (no crash) | exact, within L | ADR 0188 J4 (a reason is a string) | 0.5 s | 3.2 | code verification |
| U1 | `whyOf` (unit): 500 code points not cut; 501 cut to 500 + `…`; 499 + one astral character (500 code points) not cut; `''`, `'  '`, null, undefined, 42 give `not given` | all | ADR 0188 J4 | holds | n/a | code verification |
| R1 | `rcfd-workspace` lib: a journal with a string reason and a numeric reason for the case: the rows have the string, and no `reason` key for the number; each row's `text` is `journal_entry`'s | all | ADR 0188 J6; D1 | holds | n/a | unchanged default output |
| G1 | `npm test` in `apps/studio-web` passes, the 6 tests main skips skipped | 0 failed | rule 7 | holds | n/a | code verification |
| G2 | `--lib` of `rcfd-workspace`, `canvas_process_f978.rs` and `design_journal_f274.rs` (the named files of the process view), `docs_site`, fmt, clippy `-D warnings` | 0 failed, clean | rule 7 | holds | n/a | code verification |

Negative controls: S2 and S6 (above). Mutations (rule 16), run on a scratch copy:

* M1 the activity line without its why span: A1 must fail. **Committed** as
  `apps/studio-web/tests/e2e/reason-f273.spec.mutation.patch`, its expect-fail command building and packaging the app
  first (`npm run build && npm run package:dir && npx playwright test tests/e2e/reason-f273.spec.ts --project=electron`).
* M2 `whyOf` with no cut: U1 (and A4).
* M3 `whyOf`'s absent form empty: U1 (and A1's S2).
* M4 D1 reverted (the raw line's `reason`): R1.
* M5 the row's reason put in as HTML (`dangerouslySetInnerHTML`): A3.

Size (agent): done; 0; 1; 0; 120000

## Decision rule

Done for F-273 if the Studio shows who, what and why of every change in the e2e test (A1, A2, A3, A4, A5 hold), U1 and
R1 hold, `npm test` passes (G1), G2 holds and each mutation is caught. Partial otherwise.

## Not covered

F-979's `ChangeLine` of the live panels (it shows the reason already, without D2's form or cut); the exported page's
process panel (F-996's); linking an `mcp` actor to a Claude Code session (ADR 0188 Q4 (c)); the reason of the Studio's
own writes (it gives none); run steps' event reasons (shown as before).
