# ADR 0689: F-770 registration: a fast check of the window condition of the repetition trigger

Status: **REGISTERED 2026-11-17, written before any code of F-770 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-770 was written that remains in the tree; one probe was run to find the
moved window point (disclosed below). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-770 (dep F-766), the follow-up proposed by [ADR 0686](0686-f766-registration-a-fast-check-of-the-plain-pick-condition-of-the-repetition-trigger.md)
(remaining question 1, option b: mutation M4, the window condition dropped, survived because N1 and P1 both have the window
inside the basin). Number 0689 given by the lead. Branch `f770-work` from ae4d2af7, worktree `w49i-work/f770`, scratch
`w49i-work/f770-scratch`. Test-only item unless a defect is found.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here, not changed after a run; each registered run made once; defaults
bit-identical; no edit of an existing test or hash table (the new test is a new file); no new dependency, no unsafe code;
clean-room. `docs/ROADMAP.md` and `docs/roadmap.json` are not edited. Choices are taken on the maintainer's standing instruction
of 2026-11-11 (take the recommendation).

## What F-770 asks

The acceptance, binding: a fast test on an observation sequence whose plain pick lies in the incumbent's basin and whose last
d + 1 evaluated points do not all lie in it shows that the repetition trigger makes no restart pick, the survivor of ADR 0686
(mutation M4, the window condition dropped) is caught by it, a twin with the window inside gets a restart pick, and the O1 points
hash of ADR 0673 is unchanged. Every limit not stated in the acceptance is registered here.

## What was read

`crates/rcfd-api/src/design/bayes.rs` (the repetition branch of `propose` takes the last `window = d + 1` observations that are
not `Pending`, in the order given; `basin_restart` returns `None` when the plain pick is not inside the basin or when a window
point is not inside), `tests/f766_plain_pick_condition.rs` (observations, helpers, N1 and P1), ADR 0686, ADR 0680, ADRs 0572 and
0573 (format).

## Design

Same set-up as ADR 0686 (d = 2, unit cube, seed 1, `n0 = Some(10)`, the 14 Halton points, the incumbent (0.5, 0.5) of value 0,
then a window of three points, each next to the incumbent with value `1e-3 (1 + x0)`), with the default exclusion, so that the
plain pick lies in the basin (ADR 0686 P1: delta 0.70). The test file `crates/rcfd-api/tests/f770_window_condition.rs` has its
own copies of the helpers (the existing file is not edited).

* **W1 (the acceptance test)**: the last window point `(0.48, 0.48)` is replaced by the far point `(0.9, 0.9)` (value from the
  rough field `g`, as the 14 far points). The window is then `(0.52, 0.5)`, `(0.5, 0.52)`, `(0.9, 0.9)`: two points inside, the
  last outside. The plain pick lies inside the basin; the repetition restart makes no restart pick (`kind == Ei`, `x` bit-equal
  to the plain pick). Preconditions asserted from the library's own plain pick (incumbent 0, its length scales): the plain pick
  `delta < 1`, the first two window points `delta < 1`, the last `delta >= 3`, at least 4 evaluated points outside the basin (so
  that a dropped window condition would reach a restart pick rather than decline for lack of points).
* **W2 (the twin)**: the same observations with the last point `(0.48, 0.48)` (the window inside): a restart pick
  (`kind == Restart`) with `delta >= 2`, preconditions as W1 with the three window deltas `< 1`. This is ADR 0686 P1 restated in
  the new file so that the pair differs in the one point.

The window is the last d + 1 = 3 evaluated observations; the moved point is the last of them (the most recent). Which position
of the window is outside is not varied (see Limits).

## Probe run before this text (disclosed)

A throwaway probe (deleted, not committed) printed, for the last point in (0.8, 0.2), (0.7, 0.7), (0.9, 0.9), (0.3, 0.8) and the
default exclusion: plain pick delta 0.62, 0.14, 0.15, 0.15 (all inside); last-window delta 5.97, 3.98, 8.21, 4.97; evaluated points
outside the basin 15, 14, 15, 15; repetition pick kind `Ei` bit-equal to the plain pick in all four (the library already declines).
(0.9, 0.9) was taken as the one with the largest margin; the limits below are fixed from these numbers, not tuned.

## Checks

| id | clause | quantity | limit | estimate |
| --- | --- | --- | --- | --- |
| W1a | acceptance | plain-pick `delta` (default exclusion) | `< 1` (rho / 2) | 0.15 |
| W1b | acceptance | first two window points `delta` | both `< 1` | 0.3, 0.26 |
| W1c | acceptance | last window point `delta` | `>= 3` (rho + 1) | 8.2 |
| W1d | acceptance | evaluated points outside the basin | `>= 4` | 15 |
| W1e | acceptance | repetition restart: pick kind and bits | `Ei` and `x` bit-equal to the plain pick | by the window condition of ADR 0680 |
| W2a | twin | plain-pick `delta`; the three window deltas | `< 1`; all `< 1` | 0.70; 0.22 to 0.32 |
| W2b | twin | repetition restart pick | `kind == Restart`, its `delta >= 2` | 3.27 |
| M4 | acceptance | mutation M4 on a scratch copy | W1 fails, W2 passes | see mutations |
| H1 | acceptance | O1 points hash (`h1_default_o1_points_hash_unchanged` of f758) | `== 0xe2cf141320774cfc` | unchanged, no library line changed |
| B1 | regression | `f766_plain_pick_condition`, `f758_repetition_trigger`, `f715_robust_ei` (non-ignored), `--lib` | pass | no library line changed |

## Mutations (scratch copy `f770-scratch/copy`, library file only)

| id | mutation of `basin_restart` | expected |
| --- | --- | --- |
| M4 | the window condition dropped: the `all(inside)` term over the window removed | W1 fails (a restart pick with a window point outside); W2 passes; N1 and P1 of ADR 0686 survive (as recorded there) |
| M2 | the plain-pick condition dropped (cross-check) | W1 passes (the plain pick is inside, so the term is not exercised); N1 of ADR 0686 fails |

## Decision rule

* **done**: W1a to W1e, W2a, W2b, H1, B1 met, M4 caught by W1 and not by W2, M2 not caught by W1 (it is caught by ADR 0686 N1).
* **partial**: all met but M4 not caught, or a limit missed with the limit intact and the measured number recorded.
* **todo**: the preconditions cannot be met (the test would be vacuous).

## Limits

One function, one seed, d = 2, batch 1; a test of one step, not of a run. The outside window point is the most recent; the other
positions (first, middle) and windows containing a failed or pending observation are not tested. `window: Some(w)` other than
the default d + 1 is not tested.
