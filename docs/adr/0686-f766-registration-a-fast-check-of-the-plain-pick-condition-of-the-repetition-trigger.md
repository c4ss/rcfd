# ADR 0686: F-766 registration: a fast check of the plain-pick condition of the repetition trigger

Status: **REGISTERED 2026-11-17, written before any code of F-766 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-766 was written. Probes were run to find an observation sequence
(disclosed below). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-766 (dep F-758), the follow-up proposed by [ADR 0680](0680-f758-registration-a-repetition-trigger-for-the-basin-restart.md)
(remaining question 1: mutation M2, the plain-pick condition dropped, survived, because on seed 1 of the fast test T1 every fire
had its plain pick already in the basin). Number 0686 given by the lead. Branch `f766-work` from 5b852ad5, worktree
`w49i-work/f766`, scratch `w49i-work/f766-scratch`. Test-only item unless a defect is found.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here, not changed after a run; each registered run made once; defaults
bit-identical; no edit of an existing test or hash table; no new dependency, no unsafe code; clean-room. `docs/ROADMAP.md` and
`docs/roadmap.json` are not edited. Choices below are taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-766 asks

The acceptance, binding: a fast test with an observation sequence whose last d + 1 points lie in the incumbent's basin while the
plain pick of the step lies outside it shows that the repetition trigger makes no restart pick, the surviving ADR 0680 mutation M2
(the plain-pick condition dropped) is caught by it, and the O1 points hash of ADR 0673 is unchanged. Every limit not stated in the
acceptance is registered here.

## What was read

`crates/rcfd-api/src/design/bayes.rs` (`Trigger::Repetition`; the repetition branch of `basin_restart` returns `None` when fewer
than `window` evaluated observations exist, when the plain pick is not inside the basin, or when a window point is not inside;
afterwards it also returns `None` when fewer than d + 2 evaluated points lie outside the basin), `tests/f758_repetition_trigger.rs`
(helpers `unit`, `delta`, the O1 hash test H1), ADR 0680, ADRs 0572 and 0573 (format).

## Design

The basin of ADR 0651 is `delta(u) = sqrt(sum ((u_i - u*_i) / l_i)^2) < rho` with `rho = 2` and `l` the fitted length scales of
the objective process (the `length_scales` of the pick). The plain pick of a step is the pick of `propose` with `restart: None`
on the same observations and options; the repetition trigger may fire only when it and each of the last `d + 1` evaluated
observations have `delta < 2`. A test of the plain-pick condition needs a step where the window is inside and the plain pick is
outside, and at least `d + 2 = 4` evaluated points outside the basin (otherwise the restart declines for lack of points and a
dropped plain-pick condition is invisible). The reverse step (the twin) has the same observations with the plain pick inside.

The observation sequence, by hand (d = 2, unit-cube bounds, seed 1, `n0 = Some(10)`, `BayesOptions` otherwise default):

* 14 points `(halton_2(i), halton_3(i))`, `i = 1..14` (radical inverse in bases 2 and 3), value
  `g(x) = 0.05 + 0.04 sin(37 x0 + 5 x1) cos(41 x1)`;
* the incumbent `(0.5, 0.5)`, value 0;
* then, in this order, the window `(0.52, 0.5)`, `(0.5, 0.52)`, `(0.48, 0.48)`, each with value `1e-3 (1 + x0)`.

18 observations, all `Ok`, no constraints. The incumbent is the lowest value; the window is the last 3 observations (`d + 1`).
The 14 far points are the rough field that makes the fitted length scales short (the basin small) and keeps the process
uncertain away from the cluster, so that the plain pick of expected improvement leaves the cluster once the exclusion radius
pushes it off the evaluated points.

* **N1 (the acceptance test)**: `exclusion = 0.25` (unit cube; `BayesOptions::exclusion`, default 1e-6). The plain pick lies outside
  the basin; the restart with `Trigger::Repetition { window: None }` makes no restart pick: its pick has `kind == PickKind::Ei`
  and the same bits as the plain pick.
* **P1 (the positive twin)**: the same observations with the default exclusion. The plain pick lies inside the basin and the
  repetition restart makes a restart pick (`kind == PickKind::Restart`), outside the basin.

Both tests assert their preconditions from the library's own pick (the incumbent, the length scales of the plain pick, the
window being the last d + 1 observations, the number of points outside the basin) before the assertion about the restart, so a
changed sequence cannot pass vacuously.

## Probes run before this text (disclosed)

A scratch probe (`f766-scratch/zz_probe.rs`, not committed) tried five functions before this one (smooth sine sums, a Gaussian
well, symmetric neighbours); none gave a plain pick outside the basin with at least 4 points outside, because plain expected
improvement refines next to the incumbent (plain `delta` between 0.05 and 2.4). The final sequence above was then probed three
times: exclusion 0.15 (plain `delta` 1.98), exclusion 0.25 (plain `delta` 6.97, window deltas 0.24, 0.22, 0.32, length scales
0.084 and 0.091), and the default exclusion (plain `delta` 0.70, restart pick `delta` 3.27, same window deltas). The limits below
are fixed from these numbers; the exclusion 0.25 was chosen from the 0.15 probe (1.98, just inside the basin) by a margin, not
tuned further.

## Checks

| id | clause | quantity | limit | estimate |
| --- | --- | --- | --- | --- |
| N1a | acceptance | the window is the last 3 observations and each has `delta < 1` (rho / 2) | all three | 0.22 to 0.32 |
| N1b | acceptance | plain-pick `delta` (exclusion 0.25) | `>= 3` (rho + 1) | 6.97 |
| N1c | acceptance | evaluated points outside the basin | `>= 4` | 14 or more of 18 |
| N1d | acceptance | repetition restart: pick kind and bits | `Ei` and `x` bit-equal to the plain pick | by D1 of ADR 0680 |
| P1a | twin | plain-pick `delta` (default exclusion), the window deltas | `< 1`, all `< 1` | 0.70; 0.22 to 0.32 |
| P1b | twin | repetition restart pick | `kind == Restart` and its `delta >= 2` | 3.27 |
| M2 | acceptance | the mutation M2 on a scratch copy | N1 fails, P1 passes | see mutations |
| H1 | acceptance | O1 points hash (`h1_default_o1_points_hash_unchanged` of f758) | `== 0xe2cf141320774cfc` | unchanged, no library line changed |
| B1 | regression | `f758_repetition_trigger`, `f733_restart_second_function`, `f715_robust_ei` (non-ignored), `cargo test -p rcfd-api --release --lib` | pass | no library line changed |

## Mutations (scratch copy `f766-scratch/copy`, library file only)

| id | mutation of `basin_restart` | expected |
| --- | --- | --- |
| M2 | the plain-pick condition dropped: `!inside(rs.plain) \|\|` removed | N1 fails (a restart pick at a step whose plain pick is outside); P1 passes |
| M3 | the plain-pick condition inverted: `!inside(rs.plain)` read as `inside(rs.plain)` | N1 fails and P1 fails |
| M4 | the window condition dropped: the `all(inside)` term removed | not an acceptance item; N1 and P1 both have the window inside, so expected to survive (recorded as not covered) |

## Decision rule

* **done**: N1a to N1d, P1a, P1b, H1, B1 met, M2 caught by N1 and not by P1, M3 caught.
* **partial**: all met but M2 or M3 not caught, or a limit missed with the limit intact and the measured number recorded.
* **todo**: no sequence satisfies the preconditions (the tests would be vacuous).

## Limits

One function, one seed, d = 2, batch 1; a test of one step, not of a run. The window condition (M4) is not tested here.

## Addendum 1: results

Run once on 2026-11-17 (`cargo test -p rcfd-api --release --test f766_plain_pick_condition`; the tests are
`crates/rcfd-api/tests/f766_plain_pick_condition.rs`, 0.01 s). No library line changed.

| check | limit | measured | met |
| --- | --- | --- | --- |
| N1a window deltas (the last 3 observations) | all `< 1` | 0.239, 0.219, 0.324 | yes |
| N1b plain-pick delta, exclusion 0.25 | `>= 3` | 6.975 | yes |
| N1c evaluated points outside the basin | `>= 4` | 13 | yes |
| N1d repetition restart pick | `Ei`, bits equal to the plain pick | `Ei`, bit-equal | yes |
| P1a plain-pick delta (default exclusion); window deltas | `< 1`; all `< 1` | 0.696; as above | yes |
| P1b repetition restart pick | `Restart`, delta `>= 2` | `Restart`, delta 3.270 | yes |
| M2 on the scratch copy | N1 fails, P1 passes | N1 failed at "N1d no restart pick", P1 passed | yes |
| M3 (plain-pick condition inverted) | N1 and P1 fail | both failed | yes |
| M4 (window condition dropped), not an acceptance item | expected to survive | survived (both passed) | as expected |
| H1 O1 points hash (`h1_default_o1_points_hash_unchanged`) | `== 0xe2cf141320774cfc` | passed (f758 file: 5 passed, 2 ignored) | yes |
| B1 `f733_restart_second_function` | pass | 5 passed, 3 ignored | yes |
| B1 `f715_robust_ei` | pass | 6 passed, 2 ignored | yes |
| B1 `--lib` | pass | 141 passed, 4 ignored | yes |
| `docs_site` (rcfd-cli) | pass | 2 passed | yes |
| fmt, clippy `-D warnings` (rcfd-api, all targets) | clean | clean | yes |

Deviations: none. The probes before the registration are disclosed above. Not run: the heavy ignored records of F-758, F-733 and
F-715 (no library line changed), `baseline_hashes` (no shared solver file touched).

Decision by the registered rule: **done**. The plain-pick condition of the repetition trigger now has a fast check that cannot
pass vacuously: N1 asserts its precondition (window inside, plain pick outside by a margin, at least 4 points outside) before the
no-restart assertion, and P1 is the twin. M2 is caught by N1 alone; M3 by both.

Not covered: the window condition (M4 survives: both tests have the window inside, so a dropped window condition is invisible);
windows with a failed observation or a pending one; one function, one seed, d = 2, batch 1; the plain pick being outside through
a large exclusion radius rather than through a natural plain-pick leaving the basin.

Remaining question 1: the window condition has no fast test. Options: (a) leave it (the heavy W1 check of ADR 0680 sees it on
every restart pick); (b) a third test on the same observations with the window's last point moved outside the basin, the plain
pick inside, expecting no restart pick. Recommendation: (b), small and test-only.

## For the lead: roadmap entries

F-766 proposed status: **done**.

> Follow-up of F-758 (ADR 0680, 2026-11-17). DONE. ADR 0686, 2026-11-17: test-only, no library line changed. A fast test (crates/rcfd-api/tests/f766_plain_pick_condition.rs, 0.01 s) on a hand-built 18-observation sequence in two dimensions whose last d + 1 points lie in the incumbent's basin (deltas 0.22 to 0.32 of radius 2) while the plain pick of the step lies outside it (delta 6.98, 13 evaluated points outside): the repetition trigger makes no restart pick and its pick is bit-equal to the plain pick; the preconditions are asserted inside the test so it cannot pass vacuously. A positive twin on the same observations with the default exclusion (plain pick inside at delta 0.70) gets a restart pick at delta 3.27. The surviving ADR 0680 mutation M2 (the plain-pick condition dropped) is caught by the first test and not by the twin; the inverted condition is caught by both. The O1 points hash of ADR 0673 and the f758, f733, f715 and lib tests are unchanged. Not covered: the window condition (dropping it survives), failed or pending observations in the window, one function and one seed.

> Follow-up proposed: A fast check of the window condition of the repetition trigger. Deps F-766. Acceptance: a fast test on an observation sequence whose plain pick lies in the incumbent's basin and whose last d + 1 evaluated points do not all lie in it shows that the repetition trigger makes no restart pick, the survivor of ADR 0686 (mutation M4, the window condition dropped) is caught by it, a twin with the window inside gets a restart pick, and the O1 points hash of ADR 0673 is unchanged.
