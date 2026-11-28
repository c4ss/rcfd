# ADR 0843: F-1000 registration, KK2 of provenance-escape-f988.spec.ts under load

REGISTERED 2026-11-28, written before any code of F-1000 and before any run of its checks.

Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead. It answers the brief
`docs/adr/briefs/f1000-kk2-under-load.md` ([the brief](briefs/f1000-kk2-under-load.md)) and follows
`docs/process/agent-rules.md` ([the rules](../process/agent-rules.md)), version 5. Branch `f1000-kk2-under-load`, at the
brief's commit 8e5e7bb0 (equal to `origin/main`). Scratch folder `E:/projects/cfd/w49i-work/f1000-kk2-under-load-scratch`.

## The item

F-1000's acceptance (read from `docs/roadmap.json` by a script): "Every limit not stated here is registered in the ADR
before the run. The cause of KK2's failure in the first npm test of F-273's push (2026-11-27, "two keys in flight: a
popover open after Escape", the machine at about 100 % CPU) is diagnosed and remedied without loosening the case; KK2
passes in every one of a registered number of whole npm test runs under a registered load (at least 5), and alone in at
least 10."

## What was read

AGENTS.md; the rules (version 5); the brief; [ADR 0813](0813-f988-registration-a-ctrl-i-pressed-before-escape-reopens-the-popover.md)
in full (V, V2, the control K and its trace, "Not covered"); `apps/studio-web/tests/e2e/provenance-escape-f988.spec.ts`
in full and its mutation patch; `src/renderer/provenance/registry.ts` in full; `Provenance.tsx` lines 80 to 185
(`showPopover`, `useTrigger`, `close`); `src/renderer/commands/PaletteHost.tsx` in full; `CommandPalette.tsx` lines 70 to
170 (`afterFocusReturns`, `closeThen`, the request effect); `run.ts` (`execute`, the `provenance.show` cases);
`playwright.config.ts` (one worker, timeout 60 s, KK2's describe 120 s); the push log of F-273
(`w49i-work/push-f273.log`, lines 2792 to 2843: the failure at line 96 of the spec, `.pv-pop` 1 expected 0, in run 1 of
3, 900 passed, 14.3 min; its error context is no longer on disk). About 105 000 bytes.

## The path of a Ctrl+I, traced in the code

A Ctrl+I keydown reaches two capture listeners on the window: the registry's, which pushes a record `{ at: opened, t }`
on `pressedOn`, and `PaletteHost`'s, which sets a numbered request. The palette's effect takes the request, `execute`
returns `close(() => h.run())` after microtasks only, and `closeThen` defers the run by `afterFocusReturns`: two
animation frames and a `setTimeout(0)`. Then `showNow` drops records older than `PRESS_LIFE_MS` (2000 ms), takes the
oldest, and spends the key (opens nothing) if it was pressed with a popover open that is no longer open. Escape closes
the popover through React Aria's `onKeyDown` in the popover's own root; the trigger's `close` empties that root at once
(`flushSync`) and calls `setOpen(null)`; `opened` is set to null by `clearOpened` in the trigger's effect cleanup.

In KK2 (c) two keys are in flight with the popover "iterations" open, then Escape. A popover open after `settle` (ten
frames and 200 ms) means a request opened a number after the Escape, or the Escape closed nothing.

## Hypotheses, each with what it predicts

- **HA (a record outlived by its request).** Under load a request runs more than 2000 ms after its key (frames late), so
  `showNow` drops that key's record by age; the request finds no record, or the next key's, and with `opened` null opens
  the focused row's first number. Predicts: a late `.pv-pop` named "Provenance of iterations", and a `showNow` after the
  Escape that dropped at least one record by age.
- **HB (`opened` not yet cleared).** The Escape's close is in the DOM but the trigger's effect cleanup has not run when a
  late request runs (ADR 0813 "Not covered"); the request takes its record with `at === opened`, steps to the row's next
  number and opens it. Predicts: a `showNow` after the Escape that saw `opened` not null and stepped; the late popover is
  not "iterations".
- **HC (the Escape met no popover).** A request ran before the Escape (keys slower than the deferral under load) and
  stepped; the Escape fell between the old popover's removal and the new one's mount, or with the focus outside the
  popover, so it closed nothing. Predicts: at the Escape's keydown `.pv-pop` count 0 or the focus outside `.pv-host,
  .pv-pop`.
- **HD (a request without its own record).** A key's request was lost or a record left by an earlier key was taken, so
  a late request opens a number although no record was dropped by age. Predicts: a `showNow` after the Escape that dropped
  none, took no record or one taken with nothing open, and opened a number.

## T, the control (on the tree of 8e5e7bb0, before any remedy)

A scratch copy `t` of 8e5e7bb0 (`git archive`, its own `npm ci`, built and packaged). Changed only by a trace:

1. In the copy's `provenance-escape-f988.spec.ts`, before each block's first `focus`, a recorder as K of ADR 0813: a
   `keydown` listener on the window in the capture phase recording for each key its time, the key, whether the focus is
   inside `.pv-host, .pv-pop` and the `.pv-pop` count; a `MutationObserver` on `document.body` recording each `.pv-pop`
   added or removed with its dialog's `aria-label` and time. In a `finally`, 1500 ms after the test ends or fails, the
   log is printed as one line `KKTRACE <json>`. No step or assertion of KK2 changes.
2. In the copy's `registry.ts`, one statement at the end of each branch of `showNow` pushing to `window.__kkt` (read by
   the recorder): its time, the number of records dropped by age, the age of the record taken (or none), whether it had
   a popover (`at`), whether `opened` was null, and the outcome (spent, stepped, opened first, run opener, nothing).

Load **L**, the registered load: `load.mjs` of the scratch folder, `os.availableParallelism()` (16 on this machine)
worker threads in one node process, each a busy loop, at normal priority; the run script starts it before each run and
ends its own pid after; it also ends itself after 40 minutes. Beside each run the script records the CPU load in per cent
(`Win32_Processor.LoadPercentage`, with L running), free memory, and the count of cargo, rustc, node and electron
processes.

Arms: **T1**, the copy's KK2 spec alone under L, 10 runs; **T2**, the copy's whole `npm test` under L, 2 runs; **T3**
(the negative control of KK3, below), the copy with KK3's spec added unchanged, KK3 alone without L, 3 runs. Every run
of T1 and T2 is classified by its trace, a failing block (any "a popover open after Escape") as **TA**, **TB**, **TC**
or **TD** by the predictions above, else **TX**; every run's largest key-to-request time (from `__kkt` ages) is recorded.

Rule, fixed now:

- **T-A**: at least one TA and no TB, TC, TD: HA holds; the remedy is RA, and nothing else.
- **T-B**: at least one TB and no TC, TD (with TA too, both hold): an amendment registers RB's check and mutation, then
  RB (and RA if TA) is written.
- **T-C**: any TC, TD or TX: work stops after T; an amendment registers the remedy from the trace before any code.
- **T-0**: no failing block in T1 and T2. If T3 fails in 3 of 3 (HA's mechanism shown by a stall), RA is written as the
  remedy of a mechanism the load can make but was not seen to make; F-1000 is then at most partial (the cause was not
  observed under load). If T3 does not fail in 3 of 3, stop and report.

T is a control, not a row; none of its runs is made again.

## The remedies

**RA** (for HA), in `src/renderer/provenance/registry.ts` only. A record's life counts the frames drawn since its press as
well as the time: each record gets `frames`, counted by its own chain of animation frames that stops at
`PRESS_LIFE_FRAMES` = 10; `showNow` drops a record only when it is older than 2000 ms **and** has seen 10 frames. A
request runs two frames and a task after its key, so a record that has seen fewer than 10 frames cannot have lost its
request to the deferral, however slow the frames are; 10 is five times the frames of the deferral (the margin of
KK2's `settle`). A record whose request never ran is still dropped once 2000 ms and 10 frames have passed. Nothing else
of V2 changes; `Provenance.tsx`, `CommandPalette.tsx` and `PaletteHost.tsx` are not changed.

**RB** (for HB), in `Provenance.tsx` only: the trigger's `close` calls `clearOpened(entry)` at once, before
`hidePopover`. Written only by T-B, after its amendment.

**KK3, the new check of RA**, in a new spec `apps/studio-web/tests/e2e/provenance-stall-f1000.spec.ts` (header: F-1000,
ADR 0843), its own Studio on a copy of `quick-start` set up as KK2's: in Runs, the RE100 row focused, Ctrl+I, the popover
read ("iterations"); then a one-shot `keydown` listener is installed on the window in the capture phase that, at the next
Ctrl+I, holds the renderer's thread for 2500 ms (a loop on `performance.now()`), as a loaded machine holds its frames;
then Ctrl+I, Ctrl+I and Escape back to back, then ten frames and 200 ms (KK2's `settle`); read once without retry:
`.pv-pop` count 0, and the row has the focus. The stall is installed after the registry's listener, so the first key's
record is taken before it, and is 2500 ms against the 2000 ms of `PRESS_LIFE_MS` (margin 1.25 in time; in frames the
request still runs two frames after the stall). On the tree without RA, the first record is dropped by age, the first
request takes the second key's record and is spent, and the second request finds none and opens "iterations": KK3 fails.
The spec ends its Studio and MCP processes in `afterAll` as KK2's does; it starts no runner.

## Checks

| Row | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| R1 | `provenance-escape-f988.spec.ts` (KK2) alone after the remedy, under L, 10 runs, CPU recorded | 10 of 10 runs, 0 failed | F-1000 acceptance ("alone in at least 10") | 10 of 10 | pass or fail | code verification |
| R2 | whole `npm test` in `apps/studio-web` after the remedy, under L, 5 runs, CPU recorded | KK2 passes in 5 of 5 | F-1000 acceptance ("every one of ... whole npm test runs under a registered load (at least 5)") | 5 of 5 | pass or fail | code verification |
| R3 | the same 5 runs: the provenance specs (`provenance-f828`, `provenance-escape-f988`, `provenance-stall-f1000`) | 0 failed in each of 5 | F-1000 acceptance (not loosened); rule 7 (npm test) | 0 failed | pass or fail | code verification |
| R4 | `provenance-stall-f1000.spec.ts` (KK3) alone after RA, without L, 5 runs | 5 of 5, 0 failed | F-1000 acceptance (remedied without loosening the case) | 5 of 5 | pass or fail | code verification |
| R5 | `npm run typecheck` in `apps/studio-web` | exit 0 | rule 7 | exit 0 | pass or fail | code verification |
| R6 | `cargo test -p rcfd-cli --release --test docs_site` | 0 failed | rule 7; the brief | 0 failed | pass or fail | code verification |
| R7 | `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0843-F-1000.json` | exit 0 | rule 9 | exit 0 | pass or fail | code verification |
| R8 | `python scripts/process_report.py --check`, last | no failure but a generated file not current | rule 7 | as limit | pass or fail | code verification |

Other failures of the 5 whole runs (timing specs such as `run-control.spec.ts:268`'s P2, 20 ms, under L) are recorded
with their names and numbers as they come; they are in code this work does not touch and do not decide R2 or R3. No
limit changes after a run; each registered run is made once; no test is skipped, removed, ignored or loosened, and no
wait of KK2 is lengthened. No crate is touched, so there is no `cargo fmt`, `clippy` or `--lib` run (rule 7 asks them of
crates touched).

## Negative controls and mutations

T3 is KK3's negative control: on the tree without RA it is expected to fail in 3 of 3. T1 and T2 are KK2's under L.

Mutations of RA, on a scratch copy `m` of RA's commit (own `npm ci`), each applied alone, built and packaged, then
`provenance-stall-f1000.spec.ts` alone, 3 runs; caught means KK3 failed in that run:

- **MA1** (committed as `apps/studio-web/tests/e2e/provenance-stall-f1000.spec.mutation.patch`, made with `git diff`,
  first line `# expect-fail: cd apps/studio-web && npm run build && npm run package:dir && npx playwright test
  tests/e2e/provenance-stall-f1000.spec.ts --project=electron`): the drop test by time alone (the frame condition
  removed, V2's form). Expected: caught 3 of 3.
- **MA2**: `PRESS_LIFE_FRAMES` 1 in place of 10. Expected: caught 3 of 3 (the record has seen one frame when its request
  runs).

If RB is written, its mutation is registered by the T-B amendment.

## Size

Size (agent): partial; 1; 5 (T1 and T2, T3 and the mutations, R1, R2's five whole runs, R4); 0; 130000

## Decision rule

**Done** if T gives T-A (or T-B with its amendment), R1 to R8 hold, and every registered mutation is caught in 3 of 3.
**Partial** otherwise: T-0 (the cause not observed under load), T-C, a missed row, or a mutation not caught.

## Not covered

- Other specs that press Ctrl+I and Escape back to back are not searched; only the provenance specs are rows.
- The palette's deferral (`afterFocusReturns`) is not changed; other commands need it.
- The run-control timing specs under L (F-991's P2) are recorded, not diagnosed.
- Load L is one form of load (CPU threads in one process); a load of disk or memory is not registered.
