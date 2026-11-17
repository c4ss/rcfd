# ADR 0726: F-797 registration: the arc-length driver from a given state under prescribed-displacement loading (a library opt-in `arclength::trace_from` that starts the trace of ADR 0480 from a given converged state and increment, the prescribed values scaled by the load factor at every iteration and their load column `-K_Fp v` in the controlled correction; a closed-form snap-back under a prescribed displacement and ADR 0711's bordered trace of the 16^3 punch branch reproduced)

Status: **REGISTERED 2026-11-17, written before any code of F-797 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-797 was written and nothing of this item was run (no probe, no
timing). The numbers quoted below come from the published log of ADR 0711 (`w49i-work/f789-scratch/run2-heavy.log`), from the
ADR texts and from a scratch Python script on the closed-form toy (disclosed below). Results go in the addenda at the end;
nothing above the addenda is changed after a run.

Roadmap item F-797 (deps F-789), the follow-up proposed by
[ADR 0711](0711-f789-registration-the-subcritical-branch-of-rule-l-s-16-3-punch-path-past-0-9645.md) (its remaining question
2, option (a)); the lead's note of 2026-11-17 records that ADR 0723 found a real fold near the end of ADR 0711's trace, the
condition under which ADR 0711 recommended this item. Number 0726 given by the lead. Branch `f797-work` from cea9dd0e,
worktree `w49i-work/f797`, scratch `w49i-work/f797-scratch`. The block, element, material and branch are those of ADR 0711:
the incompatible-modes Hex8 at finite strain with rule L, the neo-Hookean material of ADR 0636 (`mu = 1e6`, `nu = 0.49`), the
punch block of ADR 0678 reading c1 on `16^3` (`u_z = -0.1 lambda` on the punch), the plain local mode iteration (no warm
start), four threads.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would normally be asked about is
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-797 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. A library opt-in, off by default, that starts the arc-length trace of ADR 0480 from a given converged state and increment with the prescribed values scaled by the load factor and their load column in the controlled correction; a closed-form control under a prescribed displacement through a limit point and ADR 0711's bordered trace of the 16^3 punch branch reproduced; baseline_hashes and the default-path bits unchanged.

Clauses: **c0** every limit registered here; **c1** a library opt-in, off by default; **c2** that starts the trace of ADR 0480
from a given converged state and increment; **c3** with the prescribed values scaled by the load factor and their load column
in the controlled correction; **c4** a closed-form control under a prescribed displacement through a limit point; **c5**
ADR 0711's bordered trace of the 16^3 punch branch reproduced; **c6** `baseline_hashes` and the default-path bits unchanged.

From the brief, also registered here: the API (function and type names), where the load column enters, how the given state
and increment are passed, how the refusals of the other options change (D1 to D4); `indefinite_retry` stays refused. Reruns
by name: `sa_arclength`, `sa_arclength_sphere_f649`, `si_eas_hg17`, `si_eas_hg19`, `si_eas_hg20` (their default tests),
`--lib`, `baseline_hashes`, `docs_site`. The new test file is `crates/rcfd-fem/tests/sa_arclength_given_state_f797.rs`
(`si_eas_hg19` to `hg21` are taken or reserved). F-843 and F-844 run in parallel and edit no library file.

## What was read

- `AGENT_RULES.md`; the brief; ADR 0711 (whole, and `run2-heavy.log`); ADR 0723's status and its roadmap note (the fold at
  `a ~ 2.170e-4`, `lambda ~ 0.95847209`, found on the warm model); ADR 0480 (D4, A1 measured `1.61e-13`); ADR 0694 D2; ADR
  0719 D1.
- `crates/rcfd-fem/src/arclength.rs` (whole); in `crates/rcfd-fem/src/nonlinear.rs`: `Problem::attempt_with`,
  `Problem::iterate` (the controlled branch, the `increment` of the prescribed equations, the update `u[eq] = lt * v` made only
  at `k == 0`), `LoadFactorControl`, `controlled_correction`, `controlled_attempt`, `load_direction`, `NewtonOptions::new`;
  `crates/rcfd-fem/src/constraint.rs` `reduce` (the right-hand side `f_i - sum_{j fixed} k_ij g_j` in row order);
  `crates/rcfd-linalg/src/skyline.rs` (the symmetry test `1e-12` of the envelope factor).
- `crates/rcfd-fem/tests/sa_arclength.rs` (the truss of ADR 0480) and `si_eas_hg17.rs` (the block, the observer, `FemCase`,
  `FemBranch`, `Bordered`, `bordered`, `amp_trace`, the heavy record).

**What the driver does today with a non-zero prescribed value.** In the controlled iteration the prescribed equations start
at the committed values (`increment = (lt - lc) v = 0`, since a controlled attempt starts at `lt = lc`), the correction
`du_f = K^-1 f_F` has no column of the prescribed values, and `u_p = lt v` is written only after iteration 0. So the load
factor of a prescribed displacement would change without the free equations seeing the change in the linearisation, and
after iteration 0 the prescribed values would lag the load factor. `trace` refuses any non-zero prescribed value for that
reason (ADR 0480 D4); the refusal stays.

## Design

### D1. The linearisation with prescribed values scaled by the load factor (first principles)

With `u_p = lambda v` on the prescribed equations (`v` their values at load 1: the model's constraints and the given list,
as `nonlinear::solve` scales them), the free residual is `R_F(u_F, lambda) = lambda f_F - f_int,F(u_F, lambda v)` and

`dR_F/du_F = -K_FF`, `dR_F/dlambda = f_F - K_Fp v`,

so Newton's correction of the controlled iteration is `K_FF du_F = R_F + dl (f_F - K_Fp v)`, with `du_p = dl v`. The load
column of the correction is `q = f_F - K_Fp v` (with `contribution_load` of ADR 0560 D3, `f` is first replaced by its
direction `f - sum_c d g_c / d lambda`, and `K` is the tangent with the contributions, the one the iteration factors). This is
the bordered Newton of ADR 0711 D3 (`K y_2 = f`, its `f` the right-hand side `-K_Fp v` of `reduce`) with the spherical
constraint of ADR 0480 in place of the linear one.

**Where it enters (the shared iteration, `nonlinear.rs`).**
- `LoadFactorControl` gains `fn prescribed_load(&self) -> bool { false }` (the default keeps every existing control as it
  is). Read once per attempt.
- In `Problem::iterate`, controlled branch: when the control's `prescribed_load()` is true, the load vector handed to
  `controlled_correction` is `q`, computed by a new private function `prescribed_load_column(kt, f, fixed)` as `reduce` forms
  its right-hand side (`q_i = f_i - sum_{j fixed} k_ij v_j` over the row of `i` in stored order, the same `f64` operations),
  on the tangent of the iteration after the contributions; otherwise the vector is the one used today.
- The update of the prescribed equations `u[eq] = lt * v` is made after every iteration when `prescribed_load()` is true
  (after iteration 0 only otherwise, as today). The residual of iteration `k` is then evaluated with `u_p = lambda_k v`.
- Nothing else changes: the energy is `du . (R_F + dl q)` (the right-hand side of the correction, as today with `f`), the
  convergence test, the factor (the envelope `L D L^T` with `NonZero` pivots and the pivot test of ADR 0480), the
  unsymmetric option (`q` is formed from the rows of the full tangent, which is what the unsymmetric factor solves with).

With `prescribed_load()` false the arithmetic of every attempt is the one of today, operation for operation: the only new
code on that path is one boolean read before the loop and the condition `k == 0 || prescribed_load` (check B1 to B4).

### D2. The given start (`arclength.rs`)

New public items:

- `pub struct PathStart` (`#[non_exhaustive]`, `Debug, Clone, PartialEq`): `load_factor: f64`, `displacement: Vec<f64>` (one
  value per equation of the model's `DofMap`), `states: Vec<ElementState>` (the committed element states, one per element),
  `increment: Option<(Vec<f64>, f64)>` (the increment `(Du, Dlambda)` that led to the start, `Du` one value per equation, of
  which only the free equations are read; `None`: no previous increment). Constructors `PathStart::new(load_factor,
  displacement, states)` (no increment), `PathStart::unloaded(model)` (zero displacement, load factor 0, `initial_states`),
  and `PathStart::increment(self, du, dlambda)`.
- `pub fn trace_from(model, prescribed, forces, options, contributions, start: &PathStart, stop) -> Result<ArcLengthPath>`:
  the trace of `trace` (the same options, points, halvings, doubling, stop and errors) with three differences: (i) it starts
  at `(start.displacement, start.load_factor, start.states)`; (ii) the previous increment of the first point is
  `start.increment` restricted to the free equations in their ascending order (the order of the reduced system), which selects
  the root of the first correction as the converged increment of the previous point does in `trace`; with `None` the root of
  the larger `Dlambda` is taken (as from the unloaded start in `trace`); (iii) the prescribed values (the model's and
  `prescribed`) are the values at load 1 scaled by the load factor, with the load column of D1 (`Spherical` gains a field
  `prescribed_load`, true in `trace_from`, false in `trace`).

`trace_from` is the opt-in: `trace` is unchanged in behaviour (it calls the same loop with the unloaded start, `None`, and
`prescribed_load = false`), and nothing calls `trace_from` by default.

**The refusals of `trace_from`** (in this order, after the model checks and `options.check()` of `trace`):
- `start.displacement.len() != n` or a `Du` of another length: `FemError::FieldLength` (key `"arc-length start"`);
- a non-finite `load_factor`, displacement value, `Du` or `Dlambda`; `states.len()` not the number of elements; an increment
  that is zero on every free equation with `Dlambda = 0`: `FemError::InvalidValue` (key `"arc-length start"`);
- a prescribed equation whose start value is not `load_factor * v` (compared as `f64` values, `0.0 == -0.0`; the product
  written as the driver writes it): `InvalidValue` (key `"arc-length start"`), naming the node and component;
- the load: refused (key `"arc length"`) when the load (or with `contribution_load` its direction at the start) is zero on
  every free equation **and** every prescribed value is zero (no load direction); a non-zero prescribed value alone is a load
  direction now;
- `check_rigid_body_motions` as in `trace`.

**The refusals of the options are unchanged** (`ArcLengthOptions::check`, the same for both functions): the factor must be
`Envelope`; `line_search`, `predictor_damping`, `indefinite_tangent`, `predictor_backtracking` and `indefinite_retry` are
refused (ADR 0607 D1, 0688 D3, 0694 D2 and D3, 0719 D1). `trace` keeps its refusal of any non-zero prescribed value (key
`"arc length"`).

### D3. The closed-form control: a snap-back under a prescribed displacement (c4)

ADR 0480's two-bar truss (supports `(-a, 0)`, `(a, 0)`, apex `(0, h)`, `L0 = 1`, `h = 0.2`, each bar a bipyramid of four Tet4
of section `s = 0.1`, `E = 2e11`, `nu = 0`, effective area `A = s^2/3`, every node's `z` fixed; the vertical force of the
truss on its apex at the deflection `w` is `P(w) = E A (h - w)(2 h w - w^2) / L0^3`, A1 of ADR 0480 measured `1.61e-13` of
`P_max`) with a **series bar**: a third bipyramid of the same section from the apex to a top node `(0, h + L_s, 0)`, `L_s =
1`, of the material `E_s = E / 50`, `nu = 0`; the top node prescribed in `x` (0) and in `y` (`-lambda d`, `d = 1`), no force.
The series bar is in uniform axial deformation (the affine state of its bipyramid is in equilibrium: the stress is uniaxial
along its axis and the axial nodal forces of its middle nodes cancel by the symmetry about its middle plane; the whole model
is symmetric about `x = 0`), with stretch `r = 1 - (lambda d - w) / L_s` and the compressive force `N(r) = E_s A r (1 - r^2)
/ 2` (Saint Venant-Kirchhoff: the second Piola-Kirchhoff stress `E_s (r^2 - 1) / 2`, the nominal force `A r` times it).
Equilibrium of the apex, `N(r) = P(w)`, gives the closed form `lambda(w) = (w + L_s (1 - r(w))) / d`, `r(w)` the root of the
cubic `r (1 - r^2) / 2 = P(w) / (E_s A)` on the branch `r > 1/sqrt(3)` through `r = 1` (found in the test by Newton's method
from `r = 1`, 100 iterations, to rounding). `dlambda/dw = (1 + P'(w) / k_t) / d`, `k_t = -dN/dr / L_s`, vanishes where the
condensed stiffness of the apex `P'(w) + k_t` vanishes: the free tangent has one negative pivot between the two folds.

**Arithmetic (scratch `f797-scratch/scripts/toy.py`, disclosed):** the folds at `w_1 = 0.48704 h`, `w_2 = 1.33564 h`;
`lambda_max = lambda(w_1) = 0.3137389`, `lambda_min = lambda(w_2) = 0.1643759`; at `w_1` the series bar is at `r = 0.784`
(above `1/sqrt(3)`); `lambda'' (w_1) = -67.5`. Beyond `w ~ 2 h` the truss force exceeds what the series bar can carry in
compression (`r` leaves its branch at `w = 2.2 h`), so the trace is stopped at `w >= 1.8 h` (the series bar in tension there).
Under load control (stepping `lambda`) the path has no continuation past `lambda_max` on its branch: it is a limit point under
a prescribed displacement.

### D4. The 16^3 punch branch: ADR 0711's bordered trace reproduced (c5)

**The start and the reference (copied from `si_eas_hg17.rs`, existing tests not edited).** OB, R0 and R1 of ADR 0711 (the
observed solve, the symmetric chain, the mode `phi_f` at 0.9647, the switch and the step to 0.9645: the B state `s_b`), then
**BT**: ADR 0711 D3's bordered Newton over its rungs `a_k = a_b 2^(k/2)`, `k = 1 .. 5` (the tangent predictor, the continuity
test, no halvings needed in ADR 0711's run: a rejected rung here is a miss), on the same code, giving the reference states
`b_0 = s_b, b_1 .. b_5` with their `lambda`, `a = phi_f . u_F` (absolute) and `sigma = 1 / (phi_f . v_t)`.

**The arc-length trace (`trace_from`, on the punch model `m`, no force, no contribution).** Options: `ArcLengthOptions::new(Ds)`
with Newton options `NewtonOptions::new(1).threads(4)` (the envelope factor, 20 iterations, `1e-10`, `1e-10`), 5 halvings, the
load scale `c = |v_t|_2^2` given explicitly, `v_t` the tangent of the B state (the test's `K^-1 f` from the supernodal factor of
the symmetrised reduced tangent, as `analyse` gives it). Two calls, both from given states:
- **stage 1**: from `PathStart::new(0.9645, u_b, states_b).increment(-v_t (expanded, the prescribed entries -v), -1)` (the
  branch tangent toward lower loads: on the branch `dlambda/da < 0`, ADR 0711), `Ds_1 = 2.5e-4 sqrt(2) |v_t|_2` (a load step
  of about `2.5e-4` along the tangent, the halved step that ADR 0711's HL1 accepted where the full `5e-4` step fell back onto
  the symmetric branch), `max_points 2`;
- **stage 2**: from stage 1's second point (its displacement and load factor, stage 1's committed states) with the increment
  `(u_2 - u_1, lambda_2 - lambda_1)`, `Ds_2 = 2 Ds_1` (about the `5e-4` steps that HL1 accepted from 0.964 on), `max_points
  40`, the stop: the second point after a fold has been read (D5), or the first point with `a > 1.5 a_5`, with `lambda`
  outside `[0.95, 0.97]`, or with `a` lower than at the previous point (a fall-back).

Every point of both stages is recorded by the stop closure (a lost trace keeps its points). A trace that ends in an error
(lost) is a reading, not a miss, as long as the coverage check AL1 is met.

**The comparison (AL3).** For every arc-length point `j` with `a_b < a_j <= a_5`: the bordered Newton of ADR 0711 D3 with
`g = phi_f`, `h = 0`, `c = a_j`, from the tangent predictor at the reference state `b_k` with the largest `a_k <= a_j` (`dp =
(a_j - a_k) / (phi_f . v_t)`, as `amp_trace` predicts); the reference `(u^B_j, lambda^B_j)`; `|lambda_j - lambda^B_j|` and
`|u_j - u^B_j|_inf / |u^B_j|_inf` (all equations). Two different algorithms (the spherical constraint with the envelope factor
and the library's controlled iteration; the amplitude constraint with the supernodal factor in the test) on the same equation.

**Limits from the tolerances.** Both iterations stop at a relative residual `<= 1e-10` of `|f_int|_2`. ADR 0711 AT0 measured
`2.3e-10` in `lambda` and `2.7e-10` in `u` between two solves of the same state at the start (lowest pair `-3499`); toward
rung 5 the lowest pair rises to `-323` (ADR 0711), about ten times closer to singular, so the error bound grows about ten
times: **`|Dlambda| <= 1e-8`, `|Du|_inf / |u|_inf <= 1e-7`** (ten times AT0's limits `1e-9`, `1e-8`).

**Expected cost (estimated, not measured):** the envelope factor of about 13,200 free equations with a reverse Cuthill-McKee
profile of about 900 per row is about `1e10` operations, 5 to 10 s per factor; about 15 to 20 points of 3 to 4 iterations, the
bordered references (about 2.5 s each, ADR 0711) and the set-up (about 60 s): **10 to 20 minutes**. The heavy record is
`#[ignore]`d and run once with a time budget of 60 minutes; the fast test covers the same library path.

### D5. The readings on the block

- **The end of the trace.** "stopped" (which stop rule) or "lost at point `j` (a, lambda)" with the cause.
- **A fold.** Between consecutive points `j-1, j` of stage 2 with `lambda_j - lambda_{j-1} > 0` after a decrease and the
  negative pivots of the points going from 1 to 0: "fold between `a_{j-1}` and `a_j`", with the loads; compared (reported, no
  limit) with ADR 0723's estimate (`a ~ 2.170e-4`, `lambda ~ 0.95847209`, found on the warm model; this trace is on the plain
  model and may be lost before it, as ADR 0711's rung 6 was).

## Checks

Every measure is required finite before it is compared (a non-finite value or a missing step misses).

### The fast test `f797_fast` (default, not ignored)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| FC1 | c3, c4 | D3 model, `trace_from(PathStart::unloaded)`, `Ds = 0.01`, 600 points at most, stop at `w >= 1.8 h`: `max_j |lambda_j - lambda(w_j)|` | `<= 1e-9` | A1 of ADR 0480 (`1.6e-13` of `P_max`), the residual tolerance: `~1e-11` |
| FC2 | c4 | the trace stopped at `w >= 1.8 h` within 600 points | holds | about 150 points |
| FC3 | c4 | `|max_{w < h} lambda_j / lambda_max - 1|`; `|min_{h < w < 1.8h} lambda_j / lambda_min - 1|` | `<= 5e-3` each | the spacing: `0.5 |lambda''| (Dw/2)^2 ~ 4e-4` |
| FC4 | c4 | the negative pivots: 0 at `w < w_1 - 0.02 h` and `w > w_2 + 0.02 h`, 1 at `w_1 + 0.02 h < w < w_2 - 0.02 h` | each | D3 |
| FC5 | c3 | the constraint at every point: `|Du_F^2 + c Dlambda^2 - Ds^2| / Ds^2` (free equations) | `<= 1e-10` | ADR 0480 A4 |
| FC6 | c3 | at every point the top node's `y` equals `-lambda d` (`lambda * v`, `f64` equality) | each | D1 |
| GS | c2 | restart: the first point `k` of FC1 with `w > h` (`lambda` decreasing there); a trace of `k` points (its states), then `trace_from` from point `k` with the increment `(u_k - u_{k-1}, lambda_k - lambda_{k-1})` and the load scale of FC1: its first 20 points against points `k+1 .. k+20` of FC1: `lambda`, `u`, `length`, `halvings`, negative pivots | equal (`f64` equality) | the same sphere and centre, the same root choice; requires point `k`'s length to equal `Ds` (checked) |
| G0 | c6 | ADR 0480's truss and force (`v = 0`): `trace` against `trace_from(PathStart::unloaded)`, `Ds = 0.05 h`, 50 points: `lambda`, `u`, `length`, `halvings`, negative pivots, residuals | equal (`f64` equality) | D1: `q = f` when `v = 0` |
| N1 | c6 | `trace` on the D3 model (non-zero prescribed value) | refused, key `"arc length"` | unchanged |
| N2 | c2 | `trace_from` with the top node's start value `lambda_0 v + 1e-3` | refused, key `"arc-length start"` | D2 |
| N3 | c6 | `trace_from` with `indefinite_retry`; with `indefinite_tangent` | refused, keys `"indefinite_retry"`, `"indefinite_tangent"` | unchanged |
| N4 | c2 | `trace_from` on ADR 0480's truss with no force (`f = 0`, `v = 0`) | refused, key `"arc length"` | D2 |
| N5 | c2 | `trace_from` with a displacement one value short | `FieldLength`, key `"arc-length start"` | D2 |

### The heavy record `f797_nh16_punch_branch_heavy` (ignored, run once)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| OB | c5 | the observed `16^3` solve: the error text | equal to ADR 0678's | ADR 0711 OB |
| R0 | c5 | the chain converges; `mu_1(0.9647)` against `873.6706402833074`; `|phi_f . u_s(0.9645)|` | holds; `<= 1e-9` rel; `<= 1e-9` | ADR 0711 R0 |
| R1 | c5 | the switch: `a` at 0.9647 against `2.694982374354194e-5`; at 0.9645 `a - a_s` against `3.827194475619e-5`, absolute `a_b` against `3.827194241e-5`; `nu` | `<= 1e-9` rel; `<= 1e-11` rel, `<= 1e-9` rel; `= 1` | ADR 0711's log (bit-identical code and library default path) |
| BT | c5 | ADR 0711's rungs 1 to 5: each accepted; `lambda_k` against `0.964113319598, 0.963379741112, 0.962070119673, 0.960069738280, 0.958472223921`; `sigma_k` against `-2.808553e1, -3.695525e1, -4.454366e1, -4.163801e1, -5.382683e-1` | holds; `<= 1e-11` abs each; `<= 1e-6` rel each | the log's printed digits |
| AL0 | c2, c3 | stage 1: two points; the first with `lambda < 0.9645` and `a > a_b` | holds | D4 |
| AL1 | c5 | coverage: at least 5 compared points (`a_b < a_j <= a_5`), at least one with `a_4 < a_j <= a_5` (`a_4 = 1.530877696e-4`, `a_5 = 2.164988001e-4`) | holds | about 12 points to `a_5` |
| AL2 | c5 | no fall-back: `a_j` strictly increasing up to the last compared point; negative pivots `= 1` at every compared point | holds; each | ADR 0711 (`nu = 1` along the branch) |
| AL3 | c5 | at every compared point: `|lambda_j - lambda^B_j|`; `|u_j - u^B_j|_inf / |u^B_j|_inf` | `<= 1e-8`; `<= 1e-7` | D4 |
| AL4 | c3 | the constraint at every point of both stages (free equations) | `<= 1e-10` rel | FC5 |
| AL5 | c3 | at every point the punch equations equal `lambda v` (`f64` equality) | each | D1 |
| AR | | the readings of D5 (end, fold) | reported | D5 |

### The default bits and the static checks

| Id | Clause | Measure | Limit |
|---|---|---|---|
| B1 | c6 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass |
| B2 | c6 | `git diff cea9dd0e -- crates/*/src` touches only `arclength.rs` and `nonlinear.rs`; in `nonlinear.rs` only the trait method with its default, the boolean read, the guarded column and the guarded update (listed with line ranges in the results) | holds (inspection) |
| B3 | c6 | G0 (the default `trace` equals the new path at `v = 0`) | holds |
| S7 | | `cargo test -p rcfd-fem --release --lib`; `--test sa_arclength`, `sa_arclength_sphere_f649`, `si_eas_hg17`, `si_eas_hg19`, `si_eas_hg20` (their default tests) | pass |
| S6 | | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` | clean |
| S8 | | `cargo test -p rcfd-cli --release --test docs_site` (docs/ touched) | pass |
| S9 | | every commit builds | builds |

### Negative and positive controls

| Id | Control | Expected |
|---|---|---|
| FC | the closed-form snap-back under a prescribed displacement | the closed form through both limit points, the inertia 0, 1, 0 |
| G0 | the default `trace` against `trace_from` with zero prescribed values | bit-equal points |
| GS | a restart from a converged point with its increment | the continuing trace, bit-equal |
| N1 to N5 | the refusals | as listed |
| BT, AL3 | the bordered Newton of ADR 0711 as an independent reference on the block | the same states |

### Planned mutations (on a scratch copy of the library commit, each run once against `f797_fast`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the load column without the prescribed part (`q = f`) | FC1, FC2 (with `f = 0` the load scale is 0: the trace fails at point 1) |
| MU2 | the prescribed values written after iteration 0 only (as today) | FC1, FC6 |
| MU3 | the prescribed part of the column with the opposite sign (`f + K_Fp v`) | FC1, FC2 |
| MU4 | the start increment ignored (`previous = None`) | GS (the restart turns back to increasing `lambda`) |
| MU5 | the start load factor ignored (the trace starts at `lambda = 0` from the given displacement) | GS |

The heavy record is not run under the mutations.

## Decision rule

- **done**: every check of `f797_fast` and of the heavy record met in their single registered runs, B1 to B3, S6 to S9 met,
  every mutation caught, no amendment.
- **partial**: the library opt-in with FC, GS, G0, N1 to N5, B1 to B3 met, but a check of the heavy record missed (diagnosed,
  recorded as a `*_known_miss`), a mutation not caught, or a check met only after an amendment.
- **todo**: B1 or B3 fails, or FC1 misses.

## Limits (not covered)

- The plain local mode iteration only on the block; the fold of ADR 0723 (warm model) is a reading, not a check: this trace
  may be lost before it as ADR 0711's rung 6 was. Tracing through that fold with the warm model is left as a follow-up.
- One spherical constraint (ADR 0480), on the free equations only (the prescribed increment `Dlambda v` enters through `c
  Dlambda^2`); no other constraint (cylindrical, the amplitude of a mode) in the library.
- The start must be converged by the caller; `trace_from` does not check its residual (its first correction does).
- No CLI, case file or API surface; `trace` keeps refusing non-zero prescribed values (the default path).
- `indefinite_retry`, `indefinite_tangent`, the line search, the damping and the backtracking stay refused.

## Open questions (taken or open)

1. **The design** (the trait method, the new function `trace_from` and `PathStart` rather than a field of
   `ArcLengthOptions`, the column formed as `reduce` forms it, the update at every iteration, the refusals, the snap-back toy,
   the two stages and their lengths, the bordered references at the arc-length amplitudes, the tolerances): taken on the
   maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. **Whether `trace` should accept non-zero prescribed values by default** (it would change no default result, since it
   refuses them today, but it changes an error into a result): not taken; `trace` keeps the refusal.
