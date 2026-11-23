# Brief: F-888, the end-to-end test of the quick run by keyboard where the Studio window lacks the system's focus

Branch: f888-studio-quick-run-focus
Items: F-888
ADR: 0786
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-23
Size (lead): done; 0; 0; 0; 70341

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f888-studio-quick-run-focus`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f888-studio-quick-run-focus-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/studio-guardrails.md 14395 (the rules of Studio work)
- apps/studio-web/tests/e2e/run-control.spec.ts 26458 (the test at line 192 and its tab-order checks)
- apps/studio-web/tests/support/studio.ts 6188 (launchStudio, how the Electron window is started)
- apps/studio-web/tests/support/rcfd.ts 4571 (alive and killPid, how the tests end the processes they start)
- docs/adr/briefs/f852-studio-close-session-timeout.md 5738 (a brief of the same kind, a Studio end-to-end test that fails on one machine)

Also in the repository, as needed: `apps/studio-web/playwright.config.ts`, the Electron main process under `apps/studio-web/electron/` or `src/main/` (find it from `package.json`), the other end-to-end tests that call `toBeFocused` (`grep -rn toBeFocused apps/studio-web/tests/e2e`), and the lead's logs of the failure, `E:/projects/cfd/w49i-work/push-f861c.log` (lines 1700 to 1900 and 2700 to 2910), read only.

## What the maintainer decided (2026-11-23)

The lead's push runs of 2026-11-23 failed `npm test` in each of three runs on the lead's Windows machine on `run-control.spec.ts:192`: `expect(run).toBeFocused()` after a Tab receives "inactive" on the Run button, while the same test passes on CI's windows-latest. Playwright reports "inactive" when the element is the document's active element but the document does not have the system's focus, which points at the window rather than the tab order; that is the lead's reading and must be shown, not assumed. The same runs left `rcfd serve` processes running after `npm test`, which held `target/release/rcfd.exe`. The maintainer allowed on 2026-11-23 ("Yes, one over") this one agent beyond the limit of 4 at once, for this item alone. The sentence this work must be able to write: the test fails on that machine because of a named cause, shown before the fix; after the fix it passes the registered number of consecutive runs whatever window has the focus, still checking the tab order and the focus ring, and the test leaves no `rcfd serve` process behind.

## Task

One registration, `docs/adr/0786-f888-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before the change** the hypotheses of the cause (at least: the window lacks the system's
   focus; the order of Tab stops differs; a timing of the select's close), the run or test that tells them apart and
   fails without the fix, the number of consecutive runs of `npm test` after it (at least 5) with another window holding
   the focus during them, the count of `rcfd serve` processes before and after `npm test`, and a committed mutation that a
   registered test must catch.
2. **Fix the test or the Studio**, whichever the cause names. The test is not skipped, retried or given a longer limit,
   and what it checks (the tab order of the run bar and the visible focus ring) is kept; if the fix is in the test
   harness (for example the window given the focus before keyboard checks), every end-to-end test that checks focus is
   looked at and the ones with the same exposure are listed. If the processes left behind have a cause in the
   harness, fix it in the same change; otherwise report it.
3. Run the registered checks, the Studio's type checks and `npm test` in `apps/studio-web`.

Decision rule to register: done if the cause is shown by a run that fails without the fix, the test then passes the registered number of consecutive runs with the focus elsewhere and keeps its checks, and the processes are counted; partial otherwise.

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: `apps/studio-web/tests/e2e/` (the test and the shared helpers), `apps/studio-web/playwright.config.ts`, and the Studio's source only where the cause is in it. No golden.

## Common to this work

The acceptance of F-888 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. Change no solver source; if a defect of the solver appears, stop and report it. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0786-F-888.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
