# ADR 0723: F-841 registration: the subcritical branch of rule L's 16^3 punch path past amplitude 1.65e-4 with warm-started local modes (ADR 0718's fine trace from rung 4 repeated with the model's warm start of the incompatible modes, ADR 0660 D1, and the element states committed along the trace, to its fold or to a registered cap; whether ADR 0711's rung 5 is reached on the same local root)

Status: **REGISTERED 2026-11-17, written before any code of F-841 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no test code of F-841 was written and nothing of this item was run on the block (no probe). The
numbers quoted below come from the published log of ADR 0718 (`w49i-work/f796-scratch/run3-heavy.log`) and from the ADR
texts; the arithmetic on the toy was done beforehand in a scratch Python script (disclosed below). Results go in the addenda
at the end; nothing above the addenda is changed after a run.

Roadmap item F-841 (deps F-796), the follow-up proposed by
[ADR 0718](0718-f796-registration-the-end-of-the-subcritical-branch-of-rule-l-s-16-3-punch-path-at-amplitude-2-2e-4.md)
(its remaining question 1, option (a)). Number 0723 given by the lead. Branch `f841-work` from cec33559 (ADRs 0694, 0697,
0700, 0711 and 0718 merged), worktree `w49i-work/f841`, scratch `w49i-work/f841-scratch`. The block, the element, the
material and the branch are those of ADRs 0711 and 0718: the incompatible-modes Hex8 at finite strain with rule L
(`IncompatibleStabilisation::LinearStress`), the neo-Hookean material of ADR 0636 (`mu = 1e6`, `nu = 0.49`), the punch block
of ADR 0678 reading c1 on `16^3`, four threads, the supernodal factor. New here: the model's opt-in warm start of the local
mode iteration (ADR 0660 D1, `SolidModel::set_incompatible_mode_warm_start(true)`), the plain local update otherwise.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would normally be asked about is
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-841 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. ADR 0718's fine trace from rung 4 repeated with the model's warm start of the incompatible modes (ADR 0660 D1) and the element states committed along the trace, to its fold if it folds or to a registered cap: dlambda/da, the lowest pairs and element 3720's mode tangent along it, whether ADR 0711's rung 5 is reached on the same local root; a fast control on a toy whose plain local iteration from zero fails while its root exists; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** ADR 0718's fine trace from rung 4 repeated with the warm start of ADR 0660
D1 and the element states committed along the trace; **c2** to its fold if it folds or to a registered cap; **c3**
`dlambda/da`, the lowest pairs and element 3720's mode tangent along it; **c4** whether ADR 0711's rung 5 is reached on the
same local root; **c5** a fast control on a toy whose plain local iteration from zero fails while its root exists; **c6** the
default bits unchanged.

From the brief, also registered here: the cap (steps, amplitude), the "same local root" criterion and the fold criterion (D3,
D4); test-only is expected, and a hook to commit the element states between trace steps, if one were needed, would be an
opt-in that changes no default computation, with `baseline_hashes`; the Newton driver is not edited (F-780, ADR 0719, adds an
opt-in there); heavy `16^3` runs capped, `#[ignore]`d and run once, the fast toy always; rule L's coefficient and the
defaults are not in scope. Reruns by name: `si_eas_hg18`, `si_eas_hg17`, `si_eas_hg16`, `si_eas_hg15`, `si_eas_hg14`, the
test file of ADR 0660 (`si_eas_hg8`), `--lib`. The new test file is `crates/rcfd-fem/tests/si_eas_hg20.rs` (`si_eas_hg19.rs`
is ADR 0719's, the lead's note of 2026-11-17).

## What was read

- `AGENT_RULES.md`; the brief; ADR 0718 (whole, and `run3-heavy.log`); ADR 0711 D3; ADR 0660 D1 (the warm start); ADR 0694 D2.
- `crates/rcfd-fem/tests/si_eas_hg18.rs` (whole: the block, the observer, `FemCase`, `FemBranch`, `Bordered`, `bordered`,
  `amp_trace`, the element helpers `element_of`, `mode_block`, `spectrum`, `elem_diag`, `record_from`, the fine trace
  `fine_trace`, `reading`, `companions`, the toy `Pl`, the heavy record).
- `crates/rcfd-fem/src/nl_incompatible.rs`: `evaluate` with `warm` (the state carries the nine committed modes after the
  material state; the local iteration starts from them and, if that iteration fails with any error, runs once more from
  `alpha = 0`; the trial state is the material trial state followed by the converged modes; a state without modes starts from
  `alpha = 0`), `warm_state`, `converged` (the start `Option<[f64; 9]>`), `mode_iteration_record_from` (the same `converged`
  with a record, so its modes are those of the driver's iteration from the same start bit for bit),
  `condensed_force_and_tangent_warm` (the driver's warm evaluation and its trial state, for checks).
- `crates/rcfd-fem/src/model.rs` (`set_incompatible_mode_warm_start`, default `false`); `nonlinear.rs`: `internal_force_field`
  and the tangent pass `model.incompatible_mode_warm_start()`; `initial_states` (an empty state per element: the neo-Hookean
  material has `state_size() = 0`).

**No library hook is needed.** The warm evaluation reads the committed modes from the element states passed to
`internal_force` and `tangent`, both public; the test-side bordered Newton of ADR 0711 D3 passes the states of the state it
starts from, so "committing" is replacing, after each accepted state, every element's state by its trial state at that state.
That trial state is computed test-side with `mode_iteration_record_from` from the committed modes, and from `alpha = 0` when it
fails (the start rule of ADR 0660 D1; the same `converged`, so the same modes bit for bit), and checked against
`condensed_force_and_tangent_warm` on element 3720 (check CW). No library file is changed (check B); `baseline_hashes` is not
run.

## What is known before the run (from ADR 0718's log)

1. Rung 4 `S4`: `a_4 = 1.530877696e-4`, `lambda 0.960069738280`, `dlambda/da -4.163801e1`, `nu 1`, the lowest pair
   `-2.465610e4`; element 3720's `K_aa` `l_min 2.273518e4`, `l_max 2.040042e7`. Rung 5 `S5`: `a_5 = 2.164988001e-4`, `lambda
   0.958472223921`, `dlambda/da -5.382683e-1`, the lowest pair `-323.40057121476354`; element 3720's `l_min 3.749e4`.
2. ADR 0718's fine trace (plain, `Da = (a_5 - a_4) / 8 = 7.926379e-6`): accepted `t_1 = 1.610141484e-4` with `lambda
   0.959749331159`, `dlambda/da -3.909602e1`, the lowest pair `-2.365429e4`, element 3720's `l_min 2.426846e4`, the warm-start
   agreement `1.9e-10`; then `1.649773378e-4` (halved); lost after it, every failure in element 3720 with an inverted point
   reached by the plain iteration from `alpha = 0`, while the iteration from the last converged modes converges in 3 to 7
   iterations (X4).
3. Cost (from the same log): the observed solve 26 s; the symmetric chain 6 s; ADR 0711's five rungs 3.5 to 4.3 s each; an
   accepted fine state 22 to 27 s, almost all of it in `lowest_any` (its bracket of the shift costs up to 70 inertia factors).

**Arithmetic beforehand (disclosed).** (i) Linear extrapolation of ADR 0718's last two states puts `sigma = 0` at `a = 2.6e-4`
(within the cap below; a reading of two points, not a prediction). (ii) The cap: `j = 1 .. 16` targets reach `a_4 + 16 Da =
2.799e-4`, beyond `a_5` (`j = 8`) and beyond ADR 0711's second failing target `2.613e-4`. (iii) Cost: with the cheaper bracket
of D2 an accepted state should take about 8 to 15 s (three to four bordered iterations of about 1 s, the analysis, the
commit over 4096 elements, eight inertia factors and the eigen solve); 16 states and the rebuild of about 70 s give 3 to 5
minutes, above the brief's 170 s: the run is ignored and made once. (iv) In a scratch Python script
(`f841-scratch/scripts/toy_pw.py`, the closed form and the local Newton only, no test code), on the toy PW of D5 with `c =
2.84`, `d = 0.01`: the fold of the branch at `a_f = 0.9415412906` (`p_f = 0.8928104858`), the local tangent there `g_z =
0.99998`; the plain local Newton from `z = 0` converges for every `a` on a grid of `1e-4` up to `0.7005` and fails at every
grid point from `0.7006` to `1.5` (one threshold, `a_c = 0.7005357469` by bisection); the Newton from the root at the previous
grid point (`Da = 0.05`) converges in 3 or 4 iterations everywhere from 0.5 to 1.3.

## Design

### D1. The start: ADR 0711's rungs rebuilt (plain), then the warm model

The heavy record rebuilds rungs 1 to 5 by the code of ADR 0718 D1 (copied: the observed `16^3` solve, the symmetric chain,
the switch, the step to 0.9645, `amp_trace` with 5 rungs) on the plain model, with ADR 0718's checks OB, R1 and S5 and their
limits. `S4` and `S5` are rungs 4 and 5.

The warm model `M_W` is the block model with `set_incompatible_mode_warm_start(true)` (and its copy without the punch
constraint, for the `FemBranch` structure only). **The commit** of a state `s` on `M_W`: for every element `e`, its material
state (empty) and its committed modes `alpha_c` (the last nine values of `s.states[e]`, if present); `mode_iteration_record_from`
(plain update, rule L) from `alpha_c`, and if that fails, or if there is no `alpha_c`, from `alpha = 0`; the element's new
state is the material state followed by the converged modes; the element **fell back** if the iteration from `alpha_c`
existed and failed. A commit that fails in an element (both iterations fail) is not possible at an accepted state (its residual
was evaluated there by the same rule).

`S4` is committed on `M_W` (no modes yet: every element from `alpha = 0`, the plain modes) to `S4_W`, then analysed on `M_W`.
Since every element's warm iteration then starts at its own converged root, its first iterate already passes the local test
and the evaluation is the plain one bit for bit (check W0).

### D2. The fine trace with the commit (c1, c2, c3)

ADR 0718 D4's fine trace (copied) with one change: **an accepted state is committed (D1) before it is analysed**, and the next
attempt starts from the committed state (its bordered Newton evaluates `internal_force` and `tangent` of `M_W` with the
committed states, as the driver of ADR 0660 D1 does within an increment). The targets `t_j = a_4 + j Da`, `Da = (a_5 - a_4) /
8`, `j = 1 .. 16` (**the cap: 16 steps, amplitude `a_4 + 16 Da = 2.799e-4`**), a failure halved toward its target, the
halvings reset on acceptance, at most 6 in a row (the 7th loses the trace), at most **40 solves**; stops "fold + 2", "steps"
(the cap), "lost". The same predictor, convergence tests and continuity test as ADR 0711 D3.

At each accepted state: `a`, `lambda`, `sigma = dlambda/da`, `nu`, the four lowest pairs, the share of the lowest vector on
element 3720's free equations, element 3720's `K_aa` at its committed modes (`l_min`, `l_max`, the asymmetry), its
committed modes' step `delta_j = |alpha_j - alpha_{j-1}|_inf / |alpha_j|_inf`, whether it fell back, the number of elements
that fell back in the commit, the plain iteration from `alpha = 0` at the same nodal displacement (converged or not, and if so
`|alpha_0 - alpha_j|_inf / |alpha_j|_inf`: whether the model's default would land on the same root), `R / mu`.

**The lowest pairs with a nearby bracket.** ADR 0718's `lowest_any` brackets the shift from `1e-6 rho` upward in up to 60
doublings and 10 bisections. Here, at a state with `nu >= 1` and a previous lowest value `mu_p < 0`: `K + s I` is tested at `s
= 1.1 |mu_p|` (zero inertia required) and `s = 0.9 |mu_p|` (non-zero inertia required), then 8 bisections, the shift `1.05
hi`, the same subspace options; if either end fails, `lowest_any` itself. With `nu = 0`, `lowest` as before. The shift only
speeds the subspace iteration; the pairs are those of the same tolerance `1e-8` (check LP0).

### D3. The same local root (c4)

**Along the trace**: the trace follows **one local root of element 3720** if element 3720 never fell back in a commit and
`delta_j <= 0.1` at every accepted state (a change of a tenth of the modes in a step of 3.7 per cent of `a_5` reads as a jump).
Otherwise the reading names the first state where it did not. Reported, not a limit.

**At rung 5**, with the classifier `root(d, dl)` on `d = |alpha_W - alpha_5|_inf / |alpha_W|_inf` (element 3720's committed
modes on the warm trace against its modes at `S5`, the plain iteration from `alpha = 0` there) and `dl = |lambda_W -
lambda_5|`: **"same root"** if `d <= 1e-6` and `dl <= 1e-8`; **"another root"** if `d >= 1e-3` or `dl >= 1e-6`;
**"undecided"** otherwise. Estimates: two solves of the same equilibrium to the bordered tolerance differ in `lambda` by
`< 1e-10` (ADR 0718 FT1: the constraint `9e-14`, S5 reproduced to `2.3e-13`); ADR 0718 X4 found the warm roots near `S5` at
`1.4` to `24` per cent from `S5`'s modes and the lowest pair at the end of the plain fine trace at `-2.29e4` against `-323`. If
the warm trace accepts `t_8` (the rung 5 amplitude, `rung == 8`), the reading **RS** is `root(d, dl)` there with the lowest
pair beside it; if not, RS is "rung 5 not reached" (with the reason: the trace's stop below `a_5`).

### D4. The reading of the trace (c2)

ADR 0718 D4's criterion unchanged (`rho_reg = 0.1`, with `l_min` now that of element 3720's `K_aa` at its committed modes):
"fold", "local breakdown: the mode tangent", "local breakdown: the local iteration", "undecided"; with one registered
addition for the cap: a trace that stops at "steps" without a sign change of `sigma` and without a sign change of `l_min`
reads **"no fold up to the cap"** (ADR 0718 read it "undecided"; here the cap is part of the acceptance). The companions of
ADR 0718 (the extrapolations, `|sigma_last| / max |sigma|`, the end interval) are reported beside it.

### D5. The fast control (`f841_fast`): the toy PW

**The toy PW**: ADR 0711's PT (`b3 = 0.5`, `b5 = 0.25`, `sb = 1`, the eight free unknowns) with a local unknown `z` of the
energy `W_l = Phi(x) + (d/2) u_0^2 z`, `x = z - c u_0^2`, `Phi(x) = x atan x - ln(1 + x^2) / 2`: the local equation `g =
atan(x) + d u_0^2 / 2 = 0` has the single root `x* = -tan(d a^2 / 2)` for `d a^2 < pi` (`g` is strictly increasing in `z`),
regular (`g_z = 1 / (1 + x^2) > 0`), `z*(a) = c a^2 + x*`. Newton on `atan` from far away overshoots and diverges (a textbook
example, cited from memory; the threshold is derived in the script, item (iv)): from `z = 0` (`x_0 = -c a^2`) it fails for `a >
a_c`, while from a nearby root it converges. The local Newton: 25 iterations, `|g| <= 1e-14`, a non-finite iterate or no
convergence an error `local: ...`; with the warm start (a flag of the toy) from the committed `z` in the state, then from 0 if
that fails (ADR 0660 D1's rule; a fallback is flagged).

The residual adds `dW_l/du_0 = -2 c u_0 atan(x) + d u_0 z` to `r_0`; the condensed tangent adds `W_uu - W_uz^2 / W_zz` to
`K_00` with `W_zz = 1 / (1 + x^2)`, `W_uz = -2 c u_0 / (1 + x^2) + d u_0`, `W_uu = -2 c atan(x) + 4 c^2 u_0^2 / (1 + x^2) + d
z`. On the branch (`u_1 = (0.1 p - a^2) / 2`) the closed form `p(a) = (1 + (b3 - 1) a^2 + b5 a^4 + 2 c d a^2 - d tan(d a^2 /
2)) / 0.9`, `dp/da = (2 (b3 - 1) a + 4 b5 a^3 + 4 c d a - d^2 a / cos^2(d a^2 / 2)) / 0.9`. Parameters **`c = 2.84`, `d =
0.01`**: `a_c = 0.7005357469 < a_f = 0.9415412906` (both computed in the test by bisection: `a_c` on the plain local Newton's
outcome, `a_f` on `dp/da`).

- **PW-P, the plain trace**: the fine trace of D2 without commit and without warm start, from `a = 0.5` (the closed form
  polished), `Da = 0.05`, 16 steps, 6 halvings, cap 60. Expected: accepted to `0.70`, then 7 failures in a row from `0.75`
  down to `0.70078`, lost; read "local breakdown: the local iteration".
- **PW-W, the warm trace with the commit**: the same with the warm start and the commit after each accepted state (`z`
  committed in the state). Expected: no failure, no fallback, past `a_c`, a sign change of `sigma` between `0.90` and `0.95`,
  "fold + 2", read "fold".
- **PW-F, the fallback**: one local solve at `a = 0.5` from a committed `z = 10` (`x_0 = 9.29`, far outside the convergence
  region): the warm iteration fails, the fallback from 0 converges to `z*(0.5)`, flagged.
- **RC, the root classifier** of D3 on PL's quadratic local problem of ADR 0718 D5 (`k_l = 1`, `c_l = 1/16`, at `a = 1`: the
  roots `z_- = (1 - sqrt(0.75)) / 2` and `z_+ = 1 - z_-`): `root(z_-, z_-)`, `root(z_-, z_+)` and `root(z_-, z_- (1 + 1e-4))`,
  with `dl = 0`.

## Checks

Every measure is required finite before it is compared (a non-finite value or a missing state misses).

### The fast test `f841_fast` (default, not ignored)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| PP-0 | c5 | the start at 0.5: `|p - p(0.5)|` | `<= 1e-12` | closed form |
| PP-1 | c5 | at every accepted state: `|p - p(a)|`; `|sigma - dp/da(a)| / |dp/da(a)|`; `|g_z - 1 / (1 + x*(a)^2)|` | `<= 1e-12`; `<= 1e-8`; `<= 1e-12` | closed form, the Newton tolerances |
| PP-2 | c5 | the stop; `a_last < a_c < t_fail,min`; `t_fail,min - a_last` | "lost"; holds; `<= Da / 2^5 = 1.5625e-3` | arithmetic (iv): `7.8e-4` |
| PP-3 | c5 | the reading; every failed attempt local; at every failed target: the root exists (`g_z(x*) >= 0.99`) and the local Newton from the last accepted `z` converges to `z*` within `1e-12` | "local breakdown: the local iteration"; holds; holds | the root exists, arithmetic (iv) |
| PW-0 | c1, c5 | the start: `|p - p(0.5)|` | `<= 1e-12` | closed form |
| PW-1 | c1, c5 | at every accepted state: as PP-1; `|z_committed - z*(a)|` | as PP-1; `<= 1e-12` | closed form |
| PW-2 | c1, c2, c5 | failed attempts; fallbacks in the commits; the stop; the last `a > a_c` | 0; 0; "fold + 2"; holds | arithmetic (iv) |
| PW-3 | c2, c5 | the reading; `a_f` in the bracket; `nu` before and after; `rho` at both | "fold"; holds; `1`, `0`; `>= 0.1` | as ADR 0718 PF-3 |
| PW-F | c1 | the fallback flagged; `|z - z*(0.5)|` | holds; `<= 1e-12` | D5 |
| RC | c4 | the three readings | "same root", "another root", "undecided" | D3 |

### The heavy record `f841_nh16_warm_trace_heavy` (ignored, run once)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| OB, R1, S5 | | as ADR 0718 (the error text; `a` at 0.9645, `nu`; rungs 1 to 5, `lambda_5`, `a_5`, `sigma_5`, `mu_1`) | ADR 0718's limits | ADR 0718's run |
| W0 | c1 | `S4_W` analysed on `M_W` against `S4` on the plain model: the free residual, `sigma`, `nu` | bit-identical (difference `0`), equal | D1 |
| LP0 | c3 | at `S4_W`: `mu_1` of the nearby bracket (from `mu_p` = `lowest_any`'s `mu_1`) against `lowest_any`'s | `<= 1e-7` relative | the same tolerance `1e-8` |
| CW | c1 | at every accepted state: element 3720's committed state against the trial state of `condensed_force_and_tangent_warm` from the previous committed state | bit-identical | the same `converged` |
| W1 | c1 | if `t_1` accepted: `lambda` against `0.959749331159`; `sigma` against `-3.909602e1`; `mu_1` against `-2.365429e4` | `<= 1e-9`; `<= 1e-6` rel; `<= 1e-6` rel | ADR 0718's `t_1`, the same root (agreement `1.9e-10`), printed digits |
| FW1 | c1 | at every accepted state: the constraint (relative); the continuity test | `<= 1e-9`; holds | ADR 0718 FT1 |
| FW2 | c2 | the reading | not "undecided" | D4 |
| FW3 | c3 | the table: `a`, `lambda`, `sigma`, `nu`, the four lowest pairs, the share on 3720, `K_aa` `l_min`, `l_max`, `rho`, `delta_j`, the fallbacks, the plain iteration's agreement; the failed attempts | reported | D2 |
| RT | c4 | the one-root reading along the trace (D3) | reported | D3 |
| RS | c4 | the rung-5 reading (D3), `d`, `dl`, `mu_1` at `t_8` | not "undecided" | D3 |

### The default bits and the static checks

| Id | Clause | Measure | Limit |
|---|---|---|---|
| B | c6 | `git diff --stat cec33559 -- crates/*/src` empty at the tip | empty |
| S7 | | `cargo test -p rcfd-fem --release --lib`; `--test si_eas_hg18`, `si_eas_hg17`, `si_eas_hg16`, `si_eas_hg15`, `si_eas_hg14`, `si_eas_hg8` (their default tests); `--test si_eas_hg20` (`f841_fast`) | pass |
| S6 | | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` | clean |
| S8 | | `cargo test -p rcfd-cli --release --test docs_site` (docs/ touched by this ADR) | pass |
| S9 | | every commit builds | builds |

`baseline_hashes` is not run: no shared solver file is touched (B checks it).

### Negative and positive controls

| Id | Control | Expected |
|---|---|---|
| PW-P | the plain local iteration from zero fails while the root exists | lost at `a_c`, "local breakdown: the local iteration" |
| PW-W | the same toy with the warm start and the commit | passes `a_c`, "fold" at the closed-form `a_f` |
| PW-F | a warm start outside the convergence region | the fallback flagged, the root found |
| RC | the classifier on two roots and a perturbed root | the three readings |
| W0 | the warm model at its own roots | the plain evaluation bit for bit |
| W1 | the first warm step against ADR 0718's plain `t_1` | the same state |

### Planned mutations (on a scratch copy of the test commit, each run once against `f841_fast`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the commit left out (the states of the start kept along the warm trace) | PW-2 (lost before the fold: from `z(0.5)` the warm start fails above about 0.86, the fallback from 0 too) |
| MU2 | the warm start ignored (the toy's local Newton always from 0) | PW-2 (failures, lost at `a_c`), PW-3 |
| MU3 | the classifier's "same root" with `d <= 1e-3` | RC (the perturbed root read "same root") |
| MU4 | the fallback flag never set | PW-F |
| MU5 | the fallback left out (a failed warm iteration is final) | PW-F (the solve fails) |

The heavy record is not run under the mutations.

## Decision rule

- **done**: every check of `f841_fast` and of the heavy record met in their single registered runs, B and S6 to S9 met, every
  mutation caught, no amendment, the reading FW2 one of "fold", "no fold up to the cap" or a "local breakdown" (c2 answered)
  and RS not "undecided" (c4 answered).
- **partial**: B met, but FW2 or RS "undecided", or a check missed, a mutation not caught, or a check met only after an
  amendment.
- **todo**: B fails, or OB fails and nothing is measured.

## Limits (not covered)

- One load path, the branch with `+phi_f` only, one element watched; the other elements enter only through the count of
  fallbacks.
- The warm start alone among the local opt-ins (no backtracking, decrease test or trust region); the commit along the trace is
  that of the test-side bordered Newton, not of the library's arc-length driver (ADR 0711 D1, F-797).
- "One local root" is the registered reading of D3 (no fallback, `delta_j <= 0.1`), not a proof that no root was skipped;
  the classifier's thresholds of RS are registered estimates.
- The cap: 16 steps of `Da` (to `a = 2.799e-4`). A fold beyond it is not looked for.
- Rule L's coefficient is not in scope; no opt-in becomes a default.

## Open questions (taken or open)

1. **The design** (test-only, the commit test-side by the record function, the nearby bracket of the lowest pairs, the cap of
   16 steps, the one-root criterion and the classifier of RS, the toy PW with `c = 2.84`, `d = 0.01`, the file
   `si_eas_hg20.rs`): taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. **Whether the warm start becomes a default**: not taken (it would change default results); open.
