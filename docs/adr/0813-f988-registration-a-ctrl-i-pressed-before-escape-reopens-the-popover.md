# ADR 0813: F-988 registration, a Ctrl+I pressed just before Escape opens the provenance popover again

REGISTERED 2026-11-26, written before any code of F-988 and before any run of its checks.

Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead. It answers the brief
[briefs/f988-provenance-popover-escape.md](briefs/f988-provenance-popover-escape.md) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f988-provenance-popover-escape`, at the
brief's commit e7d40a2c (equal to `origin/main`). Scratch folder `E:/projects/cfd/w49i-work/f988-provenance-popover-escape-scratch`.

## The item

F-988's acceptance (read from `docs/roadmap.json` by a script): "Every limit not stated here is registered in the ADR
before the run. The cause of the KR KK failure (provenance-f828.spec.ts, the popover left open after Escape, 2 of 5 runs
on 2327d4f1 in ADR 0808 Addendum 8) is diagnosed and remedied without loosening the case; the spec run alone passes in
every one of a registered number of runs (at least 10) and npm test passes."

## What was read

AGENTS.md; the rules (version 5); the brief; `apps/studio-web/tests/e2e/provenance-f828.spec.ts` (its header and helpers
to line 146, `byHover`, `byKeys`, `read`, `shut`, KR KK at lines 351 to 402); ADR 0808 Addenda 7 and 8
([0808](0808-f981-registration-retire-the-g15-coverage-map-and-the-first-path.md)) and the control K logs of its
scratch folder (`k.sh`, `k.out`, `kb-run1.log`: the failure at `shut`, line 398 of the base's file, `.pv-pop` 1, expected
0, after "14 × locator resolved to 1 element" over 5 s); `src/renderer/provenance/Provenance.tsx` and `registry.ts` in
full; `src/renderer/commands/PaletteHost.tsx` (lines 1 to 50), `CommandPalette.tsx` (lines 80 to 170), `run.ts` (the two
`provenance.show` cases); `src/shared/commands/shortcuts.ts` (`globalCommandFor`, the Ctrl+I row); React Aria 1.22.1 as
installed: `react-aria/dist/private/dialog/useDialog.cjs` (the dialog focuses itself on mount, and again after 500 ms if
it or the body has the focus) and `react-aria/dist/private/overlays/useOverlay.cjs` (Escape closes the overlay through
the overlay element's own `onKeyDown`, so only when the focus is inside it). About 90 000 bytes.

## The steps of KR KK that fail, traced in the code

The failing block (lines 391 to 398): `cell.focus()` on the residual-of-u cell of the RE100 row; twice
`Control+i` then `read(page)`; `expect(.pv-pop).toHaveCount(1)`; `shut` (Escape, then `.pv-pop` count 0 within 5 s).

How Ctrl+I acts (the code at e7d40a2c): `PaletteHost` handles the key on the window in the capture phase and only sets
a request (`setRequest`). The lazily loaded `CommandPalette` takes the request in an effect, `execute` awaits
`executeInner`, whose `provenance.show` case returns `close(() => h.run(values))`; `closeThen` then defers the run by
`afterFocusReturns`: two animation frames and a `setTimeout(0)`. Only then does `showNow` of `registry.ts` run. So the
effect of a Ctrl+I comes at least two frames after the key.

- The first Ctrl+I: nothing is open; `showNow` takes the focused cell's one number (u) and opens it. The dialog focuses
  itself (useDialog), so the focus is inside `.pv-host`. `read` waits for the popover and its facts.
- The second Ctrl+I: `read` finds the popover already open and read, and returns at once; `toHaveCount(1)` holds at once;
  the Escape follows within a few Playwright round trips, which can be less than the two frames and a task of the
  second key's deferral.
- Escape: useOverlay's `onHide` calls the trigger's `close` (Provenance.tsx `useTrigger`): the popover root is emptied at
  once (`flushSync`), `setOpen(null)`, and the focus goes back to the cell. The trigger's effect cleanup then calls
  `clearOpened`, so `opened` is null.

## Hypotheses, each with its control

- **H1 (the late key reopens it).** The second Ctrl+I's `showNow` runs after the Escape. With `opened` already null, it
  takes the focused cell's number again (`targets()`) and opens it: `.pv-pop` goes to 0 at the Escape and back to 1 a few
  frames later, and stays. The case's own steps do not wait for the second key's effect; the app acts on a key pressed
  while the popover was open as though it had been pressed after the popover closed. Cause in the app (the Show
  provenance command does not know that the popover the key was about has closed since); the case is right to expect
  the popover closed.
- **H2 (the Escape does not reach the popover).** At the Escape the focus is not inside `.pv-host` (the dialog's
  focus not yet given, lost in its 500 ms re-focus, or taken by a second key's handling), so useOverlay never sees the
  key: `.pv-pop` is never removed.

**K, the control**, on main's tree: a scratch copy `k` of e7d40a2c (`git archive`, its own `npm ci`, built and packaged),
whose `tests/e2e/provenance-f828.spec.ts` is changed only by a trace, no step of the case and no assertion changed:

1. In KR KK, before `cell.focus()` of the failing block, a recorder is installed in the page: a `keydown` listener on
   the window in the capture phase, added before the key reaches the app's, that records for each key its time
   (`performance.now()`), the key, whether `document.activeElement` is inside `.pv-host`, a short name of the active
   element (tag, role, class), and the number of `.pv-pop`; and a `MutationObserver` on `document.body` (subtree,
   child list) that records the time of each `.pv-pop` added or removed with its dialog's `aria-label`. This is the
   state before each key; the state after each key is the next key's record and the observer's log. No call of the test
   is put between the steps, so their timing is the case's.
2. The steps of the block run as they are; in a `finally`, 1500 ms after the block ends or fails, the recorder adds a
   last record (the active element, `.pv-pop` count) and the log is printed as one line `KKTRACE <json>`.
3. **Kf**, a case of the copy after KR KK: the same recorder, then on the RE100 row's residual-of-u cell: focus,
   Ctrl+I, the popover read (`read`), then Ctrl+I and Escape pressed back to back (no call between), then ten animation
   frames and 200 ms, then the last record; printed as `KFTRACE <json>`; Kf asserts nothing (it is a control, its result
   is the trace). The same for the row of RE100 (focus, Ctrl+I, read, Ctrl+I and Escape back to back).

`provenance-f828.spec.ts` of the copy alone, **10 runs**, the machine's load (CPU load in per cent, free memory, the
number of `cargo`, `rustc`, `node` and `electron` processes) recorded before each run.

Classification of a run whose KR KK fails at the block's `shut`:

- **KA** (H1): the Escape's record has the focus inside `.pv-host`, a `.pv-pop` removal follows it, and a `.pv-pop`
  addition follows that removal.
- **KB** (H2): the Escape's record has the focus outside `.pv-host`, or no `.pv-pop` removal follows it.
- **KC**: a failure of KR KK elsewhere, or a trace that is neither: recorded, not classified.

Kf in each run: **reopened** if a `.pv-pop` is present at its last record (cell or row), else **closed**.

Rule, fixed now:

- **K-A**: at least one run is KA and none is KB; or no run fails at the block's `shut` and Kf is "reopened" in at
  least one run. H1 holds; the remedy is V (below), and nothing else.
- **K-B**: at least one run is KB. H2 holds (with H1 too if some run is KA): work stops after the control; an amendment
  registers H2's remedy from the trace before any code of it, and V is written only if K-A's condition holds as well.
- **K-0**: no run fails at the block's `shut` and Kf is "closed" in every run: not diagnosed; no remedy; stop and
  report.

K is a control, not a row; none of its runs is made again.

## The remedy V, if K-A

The cause by H1 is in the app: a Show provenance key that was pressed while a popover was open is about that popover;
if that popover closes before the key's request runs, the key is spent. The remedy is in `src/renderer/provenance/
registry.ts` only:

1. In `listen()`, a `keydown` listener on the window in the capture phase (the phase and target of `PaletteHost`'s, so
   that its `stopPropagation` does not hide the key from it): for a key that is not a repeat or composing and for which
   `globalCommandFor(e)` is `provenance.show`, it records the open popover at the press (`pressedOn = { at: opened }`,
   `opened` being a new object at each opening).
2. In `showNow()`, first: take and clear `pressedOn`; if it was recorded with a popover open (`at !== null`) and that
   popover is no longer the one open (`at !== opened`), return true and open nothing.

A Ctrl+I pressed with nothing open, a second Ctrl+I stepping through the row's numbers while one is open (`at === opened`
when its request runs), and the palette's Show provenance (no key, `pressedOn` null) act as before. `Provenance.tsx`,
the Runs grid (`runs/`) and `CommandPalette.tsx` are not changed. No default output of the engine is touched; no golden.

**The new check, KK2**, in a new spec `apps/studio-web/tests/e2e/provenance-escape-f988.spec.ts` (header: F-988, ADR
0813), its own Studio window on a copy of the `quick-start` example as `provenance-f828.spec.ts` sets it up: in Runs,
(a) the RE100 row's residual-of-u cell focused, Ctrl+I, the popover read (name "residual of u"), then Ctrl+I and Escape
back to back; then ten animation frames and 200 ms; then, read once without retry, `.pv-pop` count 0, and the cell has
the focus; (b) the same on the RE100 row (Ctrl+I opens "iterations"), with the row focused at the end. The wait is
the check's set-up, not a wait added to an existing step: the deferral it outlasts is two frames and a task
(`afterFocusReturns` of `CommandPalette.tsx`), so ten frames and 200 ms is a margin of five in frames. The stem
`provenance-f828.spec` already holds F-828's committed mutation, hence a spec of its own.

`provenance-f828.spec.ts` is not changed: KR KK stays as it is, and is the acceptance's check.

## Checks

| Row | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| R1 | `provenance-f828.spec.ts` alone after V, 10 runs, load recorded with each | 10 of 10 runs with 0 failed (6 passed each, KR KK among them) | F-988 acceptance ("passes in every one of a registered number of runs (at least 10)") | 10 of 10 | pass or fail | code verification |
| R2 | `provenance-escape-f988.spec.ts` (KK2) alone after V, 5 runs | 5 of 5 runs with 0 failed | F-988 title and acceptance (the popover closes on Escape every time) | 5 of 5 | pass or fail | code verification |
| R3 | `npm test` in `apps/studio-web`, once, after R1 and R2, against the scratch engine and `f447_bench` | 0 failed | F-988 acceptance ("npm test passes") | 0 failed | pass or fail | code verification |
| R4 | `npm run typecheck` in `apps/studio-web` | exit 0 | rule 7 of the rules, version 5; the brief, task 4 | exit 0 | pass or fail | code verification |
| R5 | `cargo test -p rcfd-cli --release --test docs_site` | 0 failed | rule 7; the brief, task 4 | 0 failed | pass or fail | code verification |
| R6 | `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0813-F-988.json` | exit 0 | rule 9 | exit 0 | pass or fail | code verification |
| R7 | `python scripts/process_report.py --check`, last | no failure but a generated file not current | rule 7 | as limit | pass or fail | code verification |

No limit changes after a run; each registered run is made once; no test is skipped, removed, ignored or loosened. A
failed row is recorded as a miss with its number.

## Negative control and mutations

Kf of K is the negative control of KK2: on the tree without V the same steps are expected to leave the popover open.

Mutations, on one scratch copy `m` of V's commit (own `npm ci`), each applied alone, built and packaged, then
`provenance-escape-f988.spec.ts` alone **3 runs**; caught means KK2 failed in that run:

- **M1** (committed as `apps/studio-web/tests/e2e/provenance-escape-f988.spec.mutation.patch`, made with `git diff`,
  first line `# expect-fail: cd apps/studio-web && npm run build && npm run package:dir && npx playwright test
  tests/e2e/provenance-escape-f988.spec.ts --project=electron`): V's test in `showNow` removed (the `if` and its return).
  Expected: caught in 3 of 3, at (a).
- **M2**: V's listener added on `document` instead of `window` (the key then never reaches it, `PaletteHost` having
  stopped it at the window). Expected: caught in 3 of 3, at (a).

## Size

Size (agent): done; 0; 4 (K's 10 runs, R1 and R2, the mutations' 6 runs, R3); 0; 110000

## Decision rule

**Done** if K gives K-A, R1 to R7 hold, and M1 and M2 are each caught in 3 of 3 runs. **Partial** otherwise (K-B or
K-0 included).

## Not covered

- The case where the trigger's effect cleanup has not run when the late key's request runs (`opened` not yet null):
  React flushes that discrete update before the next frame, and the request runs two frames later, so V does not clear
  `opened` itself; a row whose late key would then step to its next number is not tested.
- Other e2e files that press Ctrl+I and Escape back to back are not searched for; only the provenance specs are run
  besides npm test.
- No change to the palette's deferral (`afterFocusReturns`), which other commands need.

## Addendum 1: amendment 1, the control's test of the focus (2026-11-26)

REGISTERED 2026-11-26, written before any run of K and before any code of V, committed alone. Written by Claude Opus 5.5
(`claude-opus-5-5`).

A defect of K's set-up, found before its run: React Aria's `Overlay` (`react-aria/dist/private/overlays/Overlay.cjs`,
lines 42 and 64, 1.22.1 as installed) portals the popover to `document.body`, so `.pv-pop` and the dialog that has the
focus are not inside `.pv-host` (the root's host is empty but for the portal's place). The test "the focus inside
`.pv-host`" of K, of KA and of KB would call the focus outside the popover in every run. It is replaced, wherever K
says "inside `.pv-host`", by "inside the popover": `document.activeElement.closest('.pv-host, .pv-pop')` is not null,
the selector `POPOVER` of `src/renderer/provenance/registry.ts`. Nothing else of K, of its rule, of V, of the rows or of
the mutations changes.

## Addendum 2: amendment 2, the form of M1 (2026-11-26)

REGISTERED 2026-11-26, written after V's commit ea5153aa and before any run of a mutation, of R1 or of R2, committed
alone. Written by Claude Opus 5.5 (`claude-opus-5-5`).

A defect of M1's set-up: removing V's `if` line from `showNow` leaves the constant `pressed` unread, and
`tsconfig.json` has `noUnusedLocals`, so `npm run build` (its `typecheck`) would refuse the mutant and the expect-fail
command would fail at the build, not at KK2. M1 is instead: the `return true` of V's test replaced by `void 0` (the
test stays, its outcome does nothing), which builds and withholds V's effect in the same way. M2, the rows, the rule and
the expected catches are unchanged. The committed patch is M1 in this form.
