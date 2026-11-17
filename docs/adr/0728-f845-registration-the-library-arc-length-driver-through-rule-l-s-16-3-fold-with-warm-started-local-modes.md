# ADR 0728: F-845 registration: the library arc-length driver through rule L's 16^3 fold with warm-started local modes (ADR 0726's `arclength::trace_from` from ADR 0711's rung 5 on the model with the warm start of the incompatible modes, the committed element states carried by the driver, through the fold computed by ADR 0724: the fold read from the load factor and the inertia, the states against a bordered reference on the warm model, the constraint measured with a floor for short arcs; a fast control on a closed-form fold with a local unknown)

Status: **REGISTERED 2026-11-17, written before any code of F-845 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no test code of F-845 was written and nothing of this item was run (no probe, no timing). The
numbers quoted below come from the published logs of ADRs 0723, 0724 and 0726 (`w49i-work/f841-scratch/run2-heavy.log`,
`f843-scratch/run2-heavy.log`, `f797-scratch/run2-heavy.log`), from the ADR texts and from a scratch Python script on the
fast toy (disclosed below). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-845 (deps F-797, F-841), the follow-up proposed by
[ADR 0724](0724-f843-registration-the-fold-of-rule-l-s-16-3-punch-branch-by-an-extended-system-with-warm-started-local-modes.md)
(its remaining question 2, option (a)); the lead's note of 2026-11-17 merges into it the follow-up proposed by
[ADR 0726](0726-f797-registration-the-arc-length-driver-from-a-given-state-under-prescribed-displacement-loading.md) (its
remaining questions 1 and 2). Number 0728 given by the lead. Branch `f845-work` from cd548b33 (ADRs 0711, 0718, 0723, 0724,
0725 and 0726 merged), worktree `w49i-work/f845`, scratch `w49i-work/f845-scratch`. The block, the element, the material and
the branch are those of ADRs 0711, 0723, 0724 and 0726: the incompatible-modes Hex8 at finite strain with rule L
(`IncompatibleStabilisation::LinearStress`), the neo-Hookean material of ADR 0636 (`mu = 1e6`, `nu = 0.49`), the punch block
of ADR 0678 reading c1 on `16^3` (`u_z = -0.1 lambda` on the punch), four threads; the model's opt-in warm start of the local
mode iteration (ADR 0660 D1, `SolidModel::set_incompatible_mode_warm_start(true)`).

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would normally be asked about is
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-845 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. From ADR 0711's rung 5, the arc-length opt-in of F-797 on the model with the warm start of the incompatible modes passes the fold computed by ADR 0724 and reads it: a sign change of dlambda/da with nu from 1 to 0 whose bracket contains a_f 2.170072e-4 and lambda_f 0.958472086948, the steps and the local fallbacks, to registered limits; a fast control on a closed-form fold with a local unknown; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** from ADR 0711's rung 5; **c2** the arc-length opt-in of F-797
(`trace_from`) on the warm model; **c3** passes the fold of ADR 0724; **c4** reads it: a sign change of `dlambda/da` with
`nu` from 1 to 0 whose bracket contains `a_f = 2.170072e-4` and `lambda_f = 0.958472086948`; **c5** the steps and the local
fallbacks, to registered limits; **c6** a fast control on a closed-form fold with a local unknown; **c7** the default bits
unchanged.

**The lead's added clauses** (the F-845 note, from ADR 0726's proposal): **n1** the committed element states carried by the
driver; **n2** the states against a bordered reference on the warm model; **n3** the constraint measured with a floor for short
arcs (ADR 0726 AL4 missed at `2.6e-6` for arcs of `6e-11`); **n4** the start: ADR 0711's state at 0.9645 may replace rung 5 if
registered.

**The start registered: rung 5** (clause c1, the binding text), not the state at 0.9645. Reasons: (i) the acceptance names rung
5; (ii) ADR 0724 W5 measured that rung 5 committed on the warm model (`S5_W`) evaluates bit for bit as the plain `S5`, and ADR
0723 RS that it is the state of the warm trace from rung 4 at `t_8` (modes to `9e-12`, `lambda` to `1.2e-14`), so the start is
on the warm branch; (iii) cost: ADR 0726's trace from 0.9645 took 657 s to `a = 2.06e-4` at 8 to 10 s a point with the
envelope factor. The stretch from 0.9645 to rung 5 by the library driver on the warm model is therefore not covered here
(ADR 0726 covered it on the plain model up to `a = 2.058e-4`; listed under the limits).

From the brief, also registered here: test-only (no library change is needed: D1); heavy `16^3` runs capped under about 15
minutes, `#[ignore]`d and run once, the fast toy always; F-846 measures the warm start as a default in its own files and no
library default is edited. Reruns by name: `sa_arclength_given_state_f797`, `sa_arclength`, `si_eas_hg20`, `si_eas_hg21` (their
default tests), `--lib`; `baseline_hashes` only if a library line changes (none is planned). The new test file is
`crates/rcfd-fem/tests/sa_arclength_fold_f845.rs`.

## What was read

- `AGENT_RULES.md`; the brief; the F-845 note in `docs/roadmap.json`; ADR 0726 (whole, and its `run2-heavy.log`); ADR 0723
  (whole); ADR 0724 (whole); ADR 0725 (D1, D2, the results of the traces).
- `crates/rcfd-fem/src/arclength.rs` (`PathStart`, `Spherical`, `trace_from`, `run`): `run` passes the committed states of the
  previous point to `controlled_attempt` and takes the attempt's returned states as the next committed ones; in
  `nonlinear.rs` `controlled_attempt`, `contribute` (a contribution's force is added to the internal force, its triplets to the
  tangent; an error of a contribution fails the attempt with the text `contribution '<name>': ...`), the `Contribution` trait.
- `crates/rcfd-fem/tests/sa_arclength_given_state_f797.rs` (whole: the truss of ADR 0480 with the series bar, the closed form,
  `constraint_error`, `prescribed_follow`, the heavy record) and `si_eas_hg21.rs` (the rebuild to rung 5, `commit_fem`, the
  helpers of the element's local problem, `fold_between`, the heavy record).

**No library hook is needed (n1).** `run` hands the attempt the committed states of the previous point and commits the
attempt's own trial states at convergence. On the warm model those trial states are the converged modes of every element from
its committed modes (from `alpha = 0` if that fails), by the same `converged` iteration that `commit_fem` of ADR 0723 calls
test-side (ADR 0725 reading c3 and its WS trace reproduced ADR 0723's commit to `4e-13` in `lambda`). So the driver carries the
warm modes from point to point by itself, and the test only reads them (check HL2 compares the driver's states at the last
point with the test-side commit chain bit for bit). No library file is changed (check B); `baseline_hashes` is not run.

## What is known before the run

1. **Rung 5 and the fold** (ADR 0724's log): `S5` at `a_5 = 2.164988000533e-4`, `lambda_5 = 0.958472223921`, `dlambda/da =
   -5.382683e-1`, `nu = 1`; the fold `a_f = 2.170071672061e-4`, `lambda_f = 0.958472086948445` (null residual `3.6e-11`),
   `a_f - a_5 = 5.0837e-7`. ADR 0723's bracket upper end `a = 2.2046e-4`, `lambda 0.958478524`, `dlambda/da = +3.755`, `nu = 0`.
   The curvature `lambda'' ~ (3.755 + 0.538) / (2.2046e-4 - 2.1650e-4) = 1.08e6` (two-point estimate).
2. **The warm trace past the fold** (ADR 0723, ADR 0725 WS): no fallback of any of the 4096 elements in any commit from rung 4
   to `2.32e-4`, every state in 3 bordered iterations, no local failure.
3. **The arc on the block** (ADR 0726's log): `|v_t| = 2.963` at the state at 0.9645, `c = |v_t|^2 = 8.779261`; near `a =
   2.05e-4` consecutive points give `|Du_F| / Da ~ 33` (e.g. points 19 to 20: `Ds = 4.09e-6`, `Dlambda = 9.76e-7`, `Da =
   8.71e-8`), and 2 to 3 iterations a point at 8 to 10 s; the bordered reference about 2 to 3 s.
4. **Cost** (ADR 0724's log): the rebuild to `S5` and its commit about 55 s; a commit about 4 s; an analysis (tangent and one
   supernodal factor) about 2 s.

**Arithmetic beforehand (disclosed).** (i) The step (D2): with `Da_t = 2e-7` the points fall at about `a_5 + j Da_t`; the fold
lies at `2.54 Da_t` past `a_5`, between points 2 and 3, `0.54 Da_t` and `0.46 Da_t` from them; there the load factor exceeds
`lambda_f` by about `0.5 lambda'' (0.46 Da_t)^2 = 4.6e-9`, and the tangent-line lower bound of D3 lies about `0.5 lambda''
(0.54 Da_t)(0.46 Da_t) = 5.4e-9` below it. (ii) The cost: about 7 points to the stop of D2, each about 10 s in the driver, 4 s
in the commit, 2 s in the analysis and 3 s in the bordered reference, with the rebuild: **about 4 to 6 minutes**. (iii) The
toy of D4, in a scratch Python script (`f845-scratch/scripts/toy_sbz.py`, the closed form and the local Newton only, no test
code), with `c_z = 200`, `d_z = 1`, `kappa = 5e4`: the plain local Newton from `z = 0` converges for every `w` of a grid of
`1e-4` up to `0.0835` and fails at every grid point from `0.0836` to `0.4` (`w_c = 0.08350358` by bisection); the closed-form
folds at `w_1 = 0.493382320 h` (`lambda_max = 0.317109990221`) and `w_2 = 1.283613473 h` (`lambda_min = 0.185532706250`), the
series bar at `r = 0.782` and `1.071` there (on its branch, `r > 1/sqrt(3)`); the local force is 1 per cent of the truss force
at `w_1` and 24 per cent at `w_2` (the folds move from ADR 0726's `0.48704 h`, `1.33564 h`); `lambda'' = -65.3` and `+18.5` at
the folds; the warm Newton from the root at `w` converges for every step `Dw` up to `0.047` at `w = 0.05` and `0.0098` at `w =
0.35`; from `z = 1e3` at `w = 0.05` it fails and from `0` it converges to `z*(0.05)`; the condensed tangent of D4 equals the
derivative of the closed-form local force to `4e-16` relative at `w = 0.17`.

## Design

### D1. The warm model and the start (c1, c2, n1)

The heavy record rebuilds ADR 0711's rungs 1 to 5 on the plain model and commits rung 5 on the warm model `M_W` by the code of
ADR 0724 D1 (copied from `si_eas_hg21.rs`: the observed `16^3` solve, the symmetric chain, the mode `phi` at 0.9647, the switch
and the step to 0.9645, `amp_trace` with 5 rungs, `commit_fem`), with ADR 0724's checks OB, R1, S5 and W5 and their limits.
`S5_W` (displacement `u_5`, load factor `lambda_5`, the committed states) is analysed on `M_W`: the symmetrised reduced
tangent, `nu_5`, `v_t = K^-1 f` (`f` the load column `-K_Fp v`), `sigma_5 = 1 / (phi . v_t)`.

**The start of `trace_from`**: `PathStart::new(lambda_5, u_5, states_5).increment(du_0, -1)` with `du_0` the expansion of
`-v_t` (the prescribed entries `-v`): the branch tangent toward lower loads, which is toward larger `a` since `sigma_5 < 0`
(`phi . (-v_t) = -1 / sigma_5 > 0`), as ADR 0726 D4's stage 1.

### D2. The trace (c2, c3, c5)

`trace_from(M_W, &[], &[], options, &[], start, stop)` with `options = ArcLengthOptions::new(Ds).load_scale(c).max_points(12)
.newton(NewtonOptions::new(1).threads(4))` (the envelope factor, 20 iterations, `1e-10`, `1e-10`, 5 halvings: ADR 0726 D4),
the load scale **`c = |v_t(B)|^2`** at the B state at 0.9645 (ADR 0726's, `8.779261`, so that the driver is configured as in
ADR 0726 and only the start and the model differ), and the arc length

    Ds = Da_t sqrt(|w_5|^2 + c sigma_5^2),   w_5 = sigma_5 v_t (the branch tangent per unit amplitude at S5_W),   Da_t = 2e-7,

the arc of a step `Da_t` along the tangent at `S5_W` (arithmetic (i): it places the fold between points 2 and 3 away from both;
`Da_t` was chosen from ADR 0724's `a_f`, disclosed). Since `Ds` doubles only up to `options.length`, every point without a
halving has the same arc.

**At each converged point `j` (in the stop closure)**: (a) the test-side commit of ADR 0723 D1 (`commit_fem` on `M_W` from
the test-side committed states of point `j-1`, those of `S5_W` for `j = 1`): the committed states, the number of elements that
fell back (the warm iteration from the committed modes failed and the one from `alpha = 0` was taken), the failed elements;
(b) the analysis of the committed state on `M_W`: `nu_j`, `v_t`, `sigma_j = 1 / (phi . v_t)`, `a_j = phi . u_F`; (c) element
3720: whether the plain iteration from `alpha = 0` at `u_j` reaches its committed modes (reported, as ADR 0723's table). The
reading of D3 is updated. **The stop**: the fourth point after the point that completes the fold's bracket; or a point with
`a_j <= a_{j-1}` (a fall-back) or `lambda` outside `[0.95, 0.97]`. The cap: 12 points (`max_points`).

**After the trace**: (d) **the bordered reference on the warm model (n2)** at every point `j`: ADR 0711 D3's bordered Newton
(`g = phi`, `h = 0`, `c = a_j`, residual and energy `1e-10`, the constraint `1e-9` relative, 20 iterations) on `M_W` with the
committed states of point `j-1` (the states the driver's attempt `j` evaluates with), from the tangent predictor at point `j-1`
(`dp = (a_j - a_{j-1}) / (phi . v_t)`); the reference `(u^B_j, lambda^B_j)`. The two solve the same equations (the free
residual on `M_W` with the same committed states) under two constraints that meet at the same point, with two factors (the
envelope `L D L^T` and the supernodal one). (e) The constraint measure with a floor (n3, D5).

### D3. The reading of the fold (c4)

On the sequence of points `0` (`S5_W`), `1, 2, ...` with `(a_j, lambda_j, sigma_j, nu_j)`: **the fold** is the first
consecutive pair `(j-1, j)` with `sigma_{j-1} < 0 <= sigma_j` and `nu_{j-1} = 1`, `nu_j = 0`. Its bracket: `a` in `[a_{j-1},
a_j]`; `lambda_f` in `[lo, hi]` with `hi = min(lambda_{j-1}, lambda_j)` and `lo` the value at the intersection of the two
tangent lines `lambda_{j-1} + sigma_{j-1} (a - a_{j-1})` and `lambda_j + sigma_j (a - a_j)` (ADR 0711's `fold_between`; for a
minimum with `lambda'' > 0` on the bracket the curve lies above both tangent lines, so `lo <= lambda_f <= hi`). The
estimate of ADR 0711 (`a_est` the zero of the linear interpolation of `sigma`, `l_est`) is reported against ADR 0724's fold.
A pair with a sign change of `sigma` without that change of `nu` is read **"undecided"** and ends the reading (no fold read).

**The containment tolerance `tau = 1e-9`** in `lambda` (registered): the two solve errors on the block are bounded by the
residual tolerance; ADR 0726 AL3 measured at most `5.3e-10` between two algorithms on the same equation; arithmetic (i) gives
margins of `4.6e-9` and `5.4e-9`. The containment of `a_f` is strict (the amplitudes are exact to `1e-15` relative).

### D4. The fast control (`f845_fast`): a closed-form fold with a local unknown (c6)

**The model SZ**: ADR 0726 D3's snap-back (ADR 0480's two-bar truss, `E = 2e11`, `nu = 0`, `h = 0.2`, `L0 = 1`, bipyramids of
section `s = 0.1`, `A = s^2/3`; the series bar of `E_s = E/50`, `L_s = 1`; the top node prescribed to `y = -lambda d`, `d =
1`), copied, with **a local unknown at the apex**: a test-side `Contribution` on the apex's `y` equation with the energy
(ADR 0723 D5's local energy PW with the apex deflection `w = -u_y` for `u_0`)

    W(w, z) = kappa [Phi(x) + (d_z / 2) w^2 z],   x = z - c_z w^2,   Phi(x) = x atan x - ln(1 + x^2) / 2,

`c_z = 200`, `d_z = 1`, `kappa = 5e4`. The local equation `g = atan(x) + d_z w^2 / 2 = 0` has the single regular root `x* =
-tan(d_z w^2 / 2)`, `z*(w) = c_z w^2 + x*` (`g_z = 1 / (1 + x^2) > 0`). The contribution solves it at every call (Newton, 25
iterations, `|g| <= 1e-14`, a non-finite iterate or no convergence an error `local: ...`); with **the warm start** from the
committed `z_c`, and if that fails from `z = 0` (the rule of ADR 0660 D1; a fallback is counted); **plain**: from `z = 0`. Its
force on the apex's `y` equation is `dW/du_y = -kappa (-2 c_z w atan(x) + d_z w z)` and its tangent the condensed `kappa (W_ww
- W_wz^2 / W_zz)`, `W_zz = 1 / (1 + x^2)`, `W_wz = -2 c_z w / (1 + x^2) + d_z w`, `W_ww = -2 c_z atan(x) + 4 c_z^2 w^2 / (1 +
x^2) + d_z z`, at the root found. **The commit** of `z_c` is done in the stop closure at each converged point (the root at the
point's `w` by the same rule), so within an attempt every call starts from the committed root, as an element of `M_W` does
within an increment.

**The closed form**: at the root the local force is `Q(w) = kappa d_z w (2 c_z w^2 - tan(d_z w^2 / 2))` (resisting), the apex
equilibrium `N(r) = P(w) + Q(w)` with ADR 0726's `P(w) = E A (h - w)(2 h w - w^2) / L0^3` and `N(r) = E_s A r (1 - r^2) / 2`,
`lambda(w) = (w + L_s (1 - r(w))) / d`, `r(w)` the root through `r = 1` (Newton from 1, 100 iterations), `dlambda/dw = (1 +
(P'(w) + Q'(w)) / k_t) / d`, `k_t = -dN/dr / L_s`. The folds `w_1`, `w_2` by bisection of `dlambda/dw` on `(0, h)` and `(h, 2h)`
(200 halvings); `w_c` by bisection on the plain local Newton's outcome (60 halvings from `[1e-4, 0.2]`).

**SZ-W, the warm trace**: `trace_from(SZ, PathStart::unloaded)` with `ArcLengthOptions::new(0.005).max_points(600)` (default
Newton options) and the warm contribution. At each point: the commit, then the analysis (the reduced tangent with the
contribution's triplet at the committed root, the supernodal factor: `nu_j`, `v_t`, `sigma_j = dlambda/dw = 1 / (-v_t[apex
y])`), the reading of D3 generalised to both folds: **the first fold** (a maximum: `sigma` from `> 0` to `<= 0`, `nu` from 0 to
1; bracket `[max(lambda_{j-1}, lambda_j), the tangent-line intersection]`) and **the second fold** (a minimum, exactly D3); the
stop: the second point after the second fold's bracket is complete. **SZ-P, the negative control**: the same trace with the
plain contribution (no commit). **SZ-F**: one local solve at `w = 0.05` from a committed `z = 1e3`. **FW, the driver's carry
on a small warm block**: the punch block of D1 on `4^3` with the warm start, `trace_from(PathStart::unloaded)` with the load
scale `c = |v_t(0)|^2` (the analysis at the unloaded state), `Ds = 0.1 sqrt(2 c)` (a load step of about 0.1), 3 points: the
driver's committed states at the last point against the test-side commit chain from the initial states (point 1 from `alpha =
0`, points 2 and 3 warm), bit for bit.

### D5. The constraint measure with a floor (n3)

At every point `j`, from the previous point (the start for `j = 1`): `e_j = |sum_F (u_j - u_{j-1})^2 + c (lambda_j -
lambda_{j-1})^2 - Ds_j^2| / Ds_j^2` (free equations), against

    e_j <= 1e-10 + 4 (k_j + 1) eps (|u_j|_F,2 + sqrt(c) |lambda_j|) / Ds_j,

`eps = f64::EPSILON`, `k_j` the iterations of the point (`residuals.len() - 1`), `|u_j|_F,2` the 2-norm over the free equations.
**Derivation**: the driver's displacement is updated `k_j` times in an attempt, each update rounding a component by up to `eps
|u_i| / 2`, and its load factor likewise; the stored `u_j - u_{j-1}` differs from the driver's increment `Du` by at most `k_j
eps |u_j|_F,2 / 2` in the 2-norm and `Dlambda` by `k_j eps |lambda_j| / 2`, so the measured `Ds^2` differs from the
driver's by a relative `k_j eps (|u|_F,2 + sqrt(c) |lambda|) / Ds` to first order; the limit is four times that with `k_j + 1`
updates, plus ADR 0480 A4's `1e-10` (the driver's own constraint, measured `2.2e-14` in ADR 0726 FC5). ADR 0726's miss
`2.6e-6` at `Ds = 6e-11` lies under this floor there (about `16 eps 5 / 6e-11 = 3e-4`). On the block here, arithmetic (i) and
item 3: `Ds ~ 2e-7 x 33 ~ 7e-6`, a floor of about `3e-9`.

## Checks

Every measure is required finite before it is compared (a non-finite value or a missing state misses).

### The fast test `f845_fast` (default, not ignored)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| TZ0 | c6 | the closed form: `w_c < w_1 < h < w_2 < 1.8 h`; `r(w) > 1/sqrt(3)` at `w_1`, `w_2` | holds | (iii): `0.0835`, `0.0987`, `0.2567` |
| TZ1 | c6 | SZ-W at every point: `|lambda_j - lambda(w_j)|`; `|z_c - z*(w_j)| / max(1, |z*(w_j)|)` | `<= 1e-9`; `<= 1e-12` | ADR 0726 FC1 `1e-13`; the local tolerance |
| TZ2 | c5, c6 | SZ-W: halvings; fallbacks in the commits; stopped by the rule within 600 points; the last `w > w_2`; the first `w > w_c` reached | 0 at every point; 0; holds; holds; holds | (iii): steps `Dw <= 0.005` against the warm margin `>= 0.0098` |
| TZ3 | c4, c6 | SZ-W: the first fold read (a maximum, `nu` 0 to 1); `w_1` strictly in its `w` bracket; `lambda_max` in `[lo - tau, hi + tau]` | holds; holds; holds (`tau = 1e-9`) | D3 |
| TZ4 | c4, c6 | SZ-W: the second fold read (a minimum, `nu` 1 to 0); `w_2` strictly in its `w` bracket; `lambda_min` in `[lo - tau, hi + tau]` | holds; holds; holds | D3 |
| TZ5 | n3 | SZ-W: the constraint of D5 at every point | each | D5 |
| TZ6 | c2 | SZ-W: the top node at `lambda v` at every point (`f64` equality) | each | ADR 0726 D1 |
| TZ7 | c4 | SZ-W: the driver's negative pivots equal `nu_j` at every point with `|w_j - w_1|, |w_j - w_2| > 0.02 h` | each | ADR 0726 FC4 |
| TZ-P | c6 | SZ-P: the trace lost (an error); its cause contains `local`; the last converged `w` below `w_c`, within `0.01` of it | holds; holds; holds | (iii): the halvings close in on `w_c` |
| TZ-F | c6 | SZ-F: the warm solve fails and the fallback from 0 converges, counted; `|z - z*(0.05)|` | holds; `<= 1e-12` | (iii) |
| FW | n1 | the `4^3` warm block: 3 points; every committed state of nine values; the driver's states at point 3 equal the test-side commit chain (all elements, bit for bit) | holds; holds; holds | ADR 0725 reading c3 |

### The heavy record `f845_nh16_fold_heavy` (ignored, run once)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| OB, R1, S5, W5 | c1 | as ADR 0724 (the error text; `a` at 0.9645, `nu`; rungs 1 to 5, `lambda_5`, `a_5`, `sigma_5`, `mu_1`; S5 committed on `M_W`: failed elements, CW, the free residual, `sigma`, `nu`) | ADR 0724's limits | ADR 0724's run |
| HA0 | c2 | the first point: `a_1 > a_5`, `lambda_1 < lambda_5` | holds | D1 |
| HF1 | c3, c4 | the fold of D3 read (not "undecided") | holds | ADR 0723, 0724 |
| HF2 | c4 | `a_f = 2.170071672061e-4` strictly inside the `a` bracket | holds | (i) |
| HF3 | c4 | `lambda_f = 0.958472086948445` in `[lo - tau, hi + tau]`, `tau = 1e-9` | holds | (i): margins `4.6e-9`, `5.4e-9` |
| HF4 | c3 | `a_j` strictly increasing over the points; `sigma_j < 0`, `nu_j = 1` at every point before the bracket's second point; `sigma_j > 0`, `nu_j = 0` at it and every point after it (at least 3 points after it) | holds | ADR 0723's table past the fold |
| HS1 | c5 | the stop by the rule of D2 within 12 points; halvings at every point; iterations at every point | holds; `0`; `<= 6` | about 7 points; ADR 0725 WS; ADR 0726's 2 to 4 |
| HL1 | c5 | local fallbacks in the test-side commit of every point (of the 4096 elements); failed elements | `0`; `0` | ADR 0723 RT |
| HL2 | n1 | the driver's committed states at the last point against the test-side commit chain | bit-identical (all elements) | D1 |
| HN | c4 | the driver's negative pivots equal `nu_j` at every point | each | the last iterate one step from the point: the lowest pair there about `+-50` (arithmetic (i): `dmu_1/da ~ 6e8` from ADR 0724, `0.46 Da_t`) |
| HB | n2 | the bordered reference at every point: converged; `max |lambda_j - lambda^B_j|`; `max |u_j - u^B_j|_inf / |u^B_j|_inf` | holds; `<= 1e-8`; `<= 1e-7` | ADR 0726 AL3's limits (measured `5.3e-10`, `6.1e-10`) |
| HC | n3 | the constraint of D5 at every point | each | D5 |
| HP | c2 | the punch equations at `lambda v` at every point (`f64` equality) | each | ADR 0726 AL5 |
| HR | c4, c5 | reported: per point `a`, `lambda`, `sigma`, `nu`, the driver's negative pivots, length, halvings, iterations, fallbacks, element 3720's plain iteration from 0, the HB differences, `e_j` and its limit; the fold estimate against ADR 0724's `a_f`, `lambda_f` | reported | |

### The default bits and the static checks

| Id | Clause | Measure | Limit |
|---|---|---|---|
| B | c7 | `git diff --stat cd548b33 -- crates/*/src` empty at the tip | empty |
| S7 | | `cargo test -p rcfd-fem --release --lib`; `--test sa_arclength_given_state_f797`, `sa_arclength`, `si_eas_hg20`, `si_eas_hg21` (their default tests); `--test sa_arclength_fold_f845` (`f845_fast`) | pass |
| S6 | | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` | clean |
| S8 | | `cargo test -p rcfd-cli --release --test docs_site` (docs/ touched by this ADR) | pass |
| S9 | | every commit builds | builds |

`baseline_hashes` is not run: no shared solver file is touched (B checks it).

### Negative and positive controls

| Id | Control | Expected |
|---|---|---|
| SZ-P | the toy without the warm start | lost below `w_c`, before the first fold, a local failure |
| SZ-W | the toy with the warm start and the commit | both closed-form folds read and bracketed |
| SZ-F | a warm start outside the convergence region | the fallback counted, the root found |
| FW | the driver's carry of the warm modes on a small block | the test-side commit chain bit for bit |
| W5 | rung 5 committed on the warm model | the plain evaluation bit for bit |
| HB | the bordered Newton of ADR 0711 on the warm model as an independent reference | the same states |

### Planned mutations (on a scratch copy of the test commit, each run once against `f845_fast`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the toy's commit left out (`z_c` kept at 0) | TZ2 (lost at `w_c`), TZ3, TZ4 |
| MU2 | the warm start ignored (the toy's local Newton always from 0) | TZ2, TZ3, TZ4, TZ-F |
| MU3 | the fallback never counted | TZ-F |
| MU4 | the fold reader's `nu` condition of a minimum reversed (0 to 1) | TZ4 |
| MU5 | FW's test-side commit from `alpha = 0` (the committed modes ignored) | FW |

The heavy record is not run under the mutations.

## Decision rule

- **done**: every check of `f845_fast` and of the heavy record met in their single registered runs, B and S6 to S9 met, every
  mutation caught, no amendment.
- **partial**: B met and the heavy trace reaches a point past `a_f` (HF1 or HF2 measured), but another check missed, a mutation
  not caught, or a check met only after an amendment.
- **todo**: B fails; or OB fails and nothing is measured; or the heavy trace does not reach a point past `a_f`.

## Limits (not covered)

- The start is rung 5 (c1): the library trace on the warm model from 0.9645 to rung 5 is not run (ADR 0726 ran it on the plain
  model to `a = 2.058e-4`).
- One fold, one load path (the branch with `+phi`); about four points past the fold (the cap of 12 points).
- The reading uses the test's analysis of each point (the symmetrised tangent and the supernodal factor); the driver's own
  negative pivots (of its last iterate) enter through HN only.
- The fast control's local unknown is a test-side contribution committed in the stop closure, not element states; the
  driver's carry of element states is checked on the `4^3` warm block (FW) and on the block (HL2).
- The step `Da_t` was chosen with ADR 0724's fold known; the trace is not a search for an unknown fold.
- No library change; the warm start stays an opt-in (F-846 measures it as a default).

## Open questions (taken or open)

1. **The design** (test-only, the start at rung 5, the load scale of ADR 0726, `Da_t = 2e-7`, the stop and cap, the reading
   and `tau`, the floor of D5, the toy SZ with `c_z = 200`, `d_z = 1`, `kappa = 5e4` and its commit in the stop closure, FW on
   `4^3`, the file name): taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. **Whether the warm start becomes a default**: not taken (it would change default results); open (F-846).
3. **Whether `trace` should accept non-zero prescribed values by default** (ADR 0726 question 3): open, not in scope.

## Amendment 1 (2026-11-17, written and committed before any run of F-845's checks)

**TZ-P, the cause of the plain control's loss.** Writing the test (before any run) showed a defect of TZ-P's registered row:
`run` halves the arc on a failure and doubles it after a success, so a plain trace that cannot pass `w_c` creeps toward it
with ever shorter arcs (ADR 0726's heavy record shrank its arcs to `6e-11` the same way), and the trace is lost when six
attempts in a row fail at one point. The last of those six can fail because an iterate passes `w_c` (a local cause) or, once
the arc is at the rounding of the displacement, because the Newton iteration's energy test cannot be met (ADR 0726's final
cause, "not converged in 20 iterations"); which one ends the trace is not determined by the toy. The registered requirement
"its cause contains `local`" therefore tests the order of the halvings, not the toy. **Corrected row TZ-P**: the plain trace
does not pass `w_c`: it ends lost (an error) or at the cap of 600 points, every converged `w` is below `w_c`, and the last
converged `w` is within `0.01` of `w_c`; the cause of the loss is reported. The limits of the other rows are unchanged. The
mutations MU1 and MU2 are caught by TZ2 to TZ4 as registered, not by TZ-P.
