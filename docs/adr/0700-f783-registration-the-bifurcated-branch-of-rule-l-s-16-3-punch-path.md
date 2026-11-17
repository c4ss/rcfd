# ADR 0700: F-783 registration: the bifurcated branch of rule L's 16^3 punch path (a branch switch at the bifurcation of ADR 0697 along its antisymmetric mode, traced with the indefinite-tangent Newton; supercritical or subcritical from the cubic coefficient of the reduced bifurcation equation measured on the stable side and from the side on which the switch finds the branch; the inertia and R/mu along the bifurcated branch to full load against the symmetric branch and F-bar)

Status: **REGISTERED 2026-11-17, written before any code of F-783 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no test code of F-783 was written and nothing of this item was run (no probe). The numbers quoted
below come from the published logs of ADR 0697 (`w49i-work/f779-scratch/run4-heavy-amended.log`), ADR 0694
(`w49i-work/f774-scratch/run3-heavy-paths.log`), ADR 0678 (`w49i-work/f747-scratch/run3-heavy.log`) and from the ADR texts.
Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-783 (deps F-779), the follow-up proposed by
[ADR 0697](0697-f779-registration-where-rule-l-s-16-3-punch-path-loses-stability.md) (its remaining questions 1 and 2). Number
0700 given by the lead. Branch `f783-work` from bbb71825 (F-774 / ADR 0694 and F-779 / ADR 0697 merged), worktree
`w49i-work/f783`, scratch `w49i-work/f783-scratch`. The block, the element and the material are those of ADR 0697: the
incompatible-modes Hex8 at finite strain with rule L (`IncompatibleStabilisation::LinearStress`), the neo-Hookean material of
ADR 0636 (`mu = 1e6`, `nu = 0.49`), the punch block of ADR 0678 reading c1 (`[0, 1]^3` on `16^3`, the die constraints, `u_z =
-0.1 lambda` on the `zmax` nodes with `x, y <= 0.5`, four threads, the supernodal factor), as `si_eas_hg15.rs` builds it.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would normally be asked about is
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-783 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. A branch switch at the bifurcation of ADR 0697 along its antisymmetric mode, traced with the indefinite-tangent Newton: whether the pitchfork is supercritical or subcritical, the inertia and R/mu along the bifurcated branch to full load against the symmetric branch and F-bar; a fast control on a closed-form pitchfork, and a control of the continuity test whose Newton converges to a distant root; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** a branch switch at the bifurcation along its antisymmetric mode, traced
with the indefinite-tangent Newton; **c2** supercritical or subcritical; **c3** the inertia and `R/mu` along the bifurcated
branch to full load against the symmetric branch and F-bar; **c4** a fast control on a closed-form pitchfork; **c5** a control
of the continuity test whose Newton converges to a distant root (the mutation MU4 of ADR 0697 that no control caught); **c6**
the default bits unchanged.

From the brief, also registered here: the switch is a perturbation along `+-phi` by a registered amplitude, then Newton with
`indefinite_tangent` at fixed loads; the criterion of super- against subcritical registered before any run; the heavy `16^3`
runs capped in load steps with progress printed, `#[ignore]`d and run once, with fast tests on the same code; rule L's
coefficient not in scope; the named reruns `si_eas_hg15`, `si_eas_hg14`, `si_eas_hg13`, `--lib`.

## What was read

- `AGENT_RULES.md`; the brief; ADR 0697 (whole, and `run4-heavy-amended.log`); ADR 0694 (D2, IN-F, FB, the log
  `run3-heavy-paths.log`); ADR 0678 (EC1 and its log: the F-bar `16^3` neo-Hookean full-load values).
- `crates/rcfd-fem/tests/si_eas_hg15.rs` (whole: `Case`, `analyse`, `continuous`, `attempt`, `bracket`, `read`, `FemCase`,
  `Toy`, `reflection`, `lowest`, `lowest_any`, the block); `crates/rcfd-fem/src/nonlinear.rs` (`solve_increment`: the committed
  displacement must carry `lc v` bit for bit, the converged iterate carries `lt v`; `solve_from`: one Newton attempt at fixed
  prescribed values and a given load vector from a given start, which must carry the prescribed values bit for bit; the
  convergence test: the free residual relative to `max(|f|, |f_int|)` at most `1e-10` and the energy ratio at most `1e-10`, 20
  iterations; `indefinite_tangent` acts in every attempt without a load-factor control, so also in `solve_from`);
  `crates/rcfd-fem/src/constraint.rs` (`Prescribed`, `prescribed_equations`, `reduce`); `crates/rcfd-fem/src/model.rs`
  (`incompatible_mode_warm_start` off by default: the element states do not carry the modes between solves).

## What is known before the run (from the published logs)

1. ADR 0697: the inertia of the symmetric path changes from 0 to 1 in `[A, B] = [0.964899444580078, 0.9649002075195312]` (the
   lower end printed by the test as `0.964899444580078`, Rust's shortest round-trip form, so the literal is that `f64`);
   `mu_1(A) = +2.554`, `mu_1(B) = -0.8131` (`(K, I)`, the symmetrised reduced tangent), so `dmu_1/dlambda ~ -4.41e6` there; the
   next pairs at `A`: `2297.1`, `3058.3`, `3935.9`; the mode `phi` exactly antisymmetric under `P: (x, y, z) -> (y, x, z)`
   (`sigma = -1` to 12 digits), orthogonal to the load vector to `1.5e-12`; the path symmetric to `1e-11`; F-bar stable.
2. ADR 0694: at `lt = 9.656250000000001e-1` the symmetric state has `R/mu = 0.32765165`, `w_c = 0.06039238`, `nu = 1`, lowest
   pair `-3271.88` (hourglass, element 3720); with the indefinite tangent the block solves to full load on the symmetric branch:
   `R/mu = 0.33937627`, `w_c = 0.06245308`, `nu = 1` (IN-F).
3. F-bar on `16^3` (neo-Hookean, ten increments): `R/mu = 0.32599497`, `w_c = 0.06060900` at full load (ADR 0678 log line 20);
   `0.31425746`, `0.05849960` at `0.9625` (ADR 0694 FB); `nu = 0` and `mu_1 = 2262.5` at `B` (ADR 0697).
4. Cost (ADR 0697 log): the observed `16^3` rule L solve 23.5 s; F-bar observed 12.1 s; one indefinite-tangent increment 1.2 to
   1.9 s; a lowest-pairs solve 1.4 s.

**Arithmetic beforehand (disclosed).** (i) With the slope of 1, `mu_1` at `lambda_f = 0.9647` (`A - lambda_f = 1.994e-4`) is
about `2.55 + 4.41e6 * 1.994e-4 = 882`, below the second pair (`~2297`) and well away from zero. (ii) A dimensional estimate of
the cubic coefficient of the reduced equation of an hourglass mode of one element (`h = 1/16`): the stiffness `mu h = 62500` per
unit squared strain of the mode, the strain of a unit-norm mode concentrated on 24 equations about `0.2 a / h`, so `beta ~ mu h
/ h^2 ~ 1.6e7` per unit `a^2` (sign unknown, order of magnitude only). Then the amplitude scale `sqrt(|mu_1| / |beta|)` is about
`7e-3` at `lambda_f` and `1.4e-2` at `lt`. (iii) The noise of a measured amplitude: the Newton residual `1e-10` of `|f_int| ~
1e5` along a mode of stiffness `~880` gives `~1e-8` absolute, `5e-5` of the smallest amplitude `2e-4` below. (iv) The toy of the
continuity control (C5, below) was evaluated by hand arithmetic in a scratch Python script (disclosed; the closed form only, no
test code): `g(u) = u e^-u + u / 20` has its fold at `u = 1.15940`, `p_c = 0.421644`, its local minimum at `u = 4.1399`; Newton
from the root at `p = 0.2` (`u = 0.23878`) to `p = 0.7` converges in five iterations to the distant root `u = 13.99977`. (v)
The switch of D3 on the toys PT-S and PT-U of D5 was likewise evaluated by hand in a scratch Python script before this text
(disclosed; the 2 by 2 coupled Newton of the closed form): from the plain start `u_s + eps phi` PT-S finds its branch for `eps =
a*, 2 a*, 4 a*` and returns to the symmetric state for `a* / 4`, but **PT-U returns to the symmetric state for every `eps`**:
its strong symmetric coupling (`u_1` on the branch is `-0.05`, on the symmetric path `+0.05`) makes the start lie off the slow
manifold; from the second-order start of D3 (`u_1` shifted by `-eps^2 / 2`) both find their branches for `eps = a*, 2 a*, 4 a*`
and return to the symmetric state for `a* / 4`, and the other side returns to the symmetric state in 6 to 19 iterations. This is
why D3 registers the second-order start after the plain one.

## Design

### D1. The reduced bifurcation equation (c2), first principles

On the free equations the equilibrium is `r(u, lambda) = 0` with the symmetric (Hessian) tangent `K`. The block and its loads
are invariant under the reflection `P` (`P r(u) = r(P u)`), and the symmetric path `u_s(lambda) = P u_s(lambda)`. At a
symmetry-breaking simple bifurcation at `lambda_c` with the antisymmetric mode `phi` (`P phi = -phi`), the Lyapunov-Schmidt
reduction (from first principles: split `u = u_s + a phi + w`, `w` orthogonal to `phi`; solve the equations orthogonal to `phi`
for `w(a, lambda)` by the implicit function theorem, which applies because `K` restricted to that complement is regular) gives
one scalar equation `G(a, lambda) = phi . r(u_s + a phi + w(a, lambda), lambda) = 0`. The symmetry makes `G` odd in `a` (`P` maps
`a` to `-a` and `w(-a) = P w(a)`), so `G = a (mu(lambda) + beta a^2 + O(a^4))` with `mu(lambda)` the critical eigenvalue
(`mu(lambda_c) = 0`, `mu' = alpha < 0` here) and `beta` the cubic coefficient, which includes the coupling through `w`, i.e.
through the symmetric modes (the term `-c^2` of the toy of D4). The non-trivial branch is `mu(lambda) + beta a^2 = 0`:

- **supercritical** if `beta > 0`: the branch exists where `mu < 0`, i.e. for `lambda > lambda_c`, `a^2 = -mu(lambda) / beta`;
  there `dG/da = mu + 3 beta a^2 = -2 mu > 0`: the bifurcated branch takes over the stability (its inertia one lower than the
  symmetric branch's);
- **subcritical** if `beta < 0`: the branch exists for `lambda < lambda_c`, `a^2 = mu / |beta|`, with `dG/da = -2 mu < 0`:
  unstable (its inertia one higher).

The sign of the load change needed to sustain an amplitude, `lambda - lambda_c = beta a^2 / |alpha|` (from `mu ~ alpha (lambda
- lambda_c)`), is the sign of `beta`: this is the brief's criterion, measured here twice (D2 and D3).

### D2. `beta` from the forced response on the stable side

**The measurement.** At a symmetric state `u_s` at a load `lambda_f` with `mu_1(lambda_f) > 0` (stable), with `phi_f` the lowest
pair's vector there (unit 2-norm on the free equations, the sign fixed so that its largest component is positive), apply the
external force `eta phi_f` and solve `r(u, lambda_f) = eta phi_f` by Newton at the fixed load. At the solution every component
of `r` orthogonal to `phi_f` vanishes and `phi_f . r = eta`: the solution is exactly the Lyapunov-Schmidt level set `G(a) =
eta` of D1 with `a = phi_f . (u - u_s)`, so the secant stiffness `k_s(a) = eta / a = mu_1 + beta a^2 + O(a^4)` (with `phi_f`
the eigenvector at `lambda_f`, `k_s(0) = 1 / (phi_f . K^-1 phi_f) = mu_1(lambda_f)` exactly; `beta` at `lambda_f` differs from
its value at `lambda_c` by `O(lambda_c - lambda_f)`). Supercritical: the response **stiffens** (`k_s` grows with `a`);
subcritical: it **softens**.

**The ladder (registered).** Target amplitudes `a_j = a_0 2^j`, `j = 0..8` (`a_0 = 2e-4` on the FEM block and on the toys), the
force `eta_j = +- mu_1 a_j`, each level solved from the converged previous level of the same sign (the first from `u_s`), both
signs; a level that fails ends that sign's ladder. Per level with both signs converged: `a = (a_+ - a_-) / 2` (the measured
amplitudes), `k_s = |eta_j| / a`, the **Z2 test** `z = |a_+ + a_-| / |a_+ - a_-|` (0 for an odd `G`; a quadratic term, i.e. a
transcritical rather than a pitchfork bifurcation, would show here). `k_0` by the two smallest levels, `k_0 = (k_s0 a_1^2 -
k_s1 a_0^2) / (a_1^2 - a_0^2)` (the quadratic fit through them), checked against `mu_1(lambda_f)`. `beta_j = (k_s,j - k_0) /
a_j^2`. **The window**: the levels with `1e-3 <= |k_s,j / k_0 - 1| <= 0.5` (the deviation above the noise, arithmetic (iii):
`5e-5 * 1.7` for `k_0`, and small enough for the `a^2` term to dominate). **Reading CF**: "stiffening" if at least two levels are
in the window and every `beta_j` there is positive, "softening" if at least two and every one negative, "undecided" otherwise;
`beta^` is `beta_j` of the first window level. With arithmetic (ii) the window is reached at `a ~ 2e-4` to `2e-3`; if `|beta|`
were below `~340` no level of the ladder (largest `0.0512`) would reach it and CF reads "undecided".

**`alpha` and `lambda_c`.** `alpha^ = (mu_1(A) - mu_1(lambda_f)) / (A - lambda_f)` from the lowest pairs at the symmetric
states at `A` and `lambda_f`, and `lambda_c^ = A + mu_1(A) / |alpha^|` (the linear extrapolation over `5.8e-7`); checked against
ADR 0697's bracket.

`lambda_f = 0.9647` (arithmetic (i): `mu_1 ~ 880`, the lowest pair, nu = 0). Checked there: the lowest pair is the critical
mode (`|phi_f . phi_A| >= 0.99` with the lowest vector at `A`, and `sigma(phi_f) = phi_f . P phi_f = -1` within `1e-6`).

### D3. The branch switch (c1, c2)

**The predicted amplitude.** On the side where the reading CF puts the branch (stiffening: `lambda_s = lt` past the
bifurcation, the symmetric state there with `mu_1^sym(lt) < 0`; softening: `lambda_s = lambda_f`), `a*_pred = sqrt(|mu_1^sym
(lambda_s)| / |beta^|)` (D1 with the measured `beta^` and the eigenvalue of the symmetric state at `lambda_s`).

**The switch.** From the **plain start** `u_s(lambda_s) + eps phi_f` (the free equations; the prescribed values those of
`lambda_s`), Newton with `indefinite_tangent` at the fixed load `lambda_s` (`solve_from`, no external force). If that does not
find the branch, from the **second-order start** `u_s(lambda_s) + eps phi_f + eps^2 w_2`, where `w_2 = s / a^2` is the
symmetric part of the forced response of D2 at its first window level (`s = (u_+ + u_-) / 2 - u_s(lambda_f)` on the free
equations, `a` its measured amplitude): the even term `w(a) = a^2 w_2 + O(a^4)` of the Lyapunov-Schmidt reduction (D1), so that
the start lies on the slow manifold to second order (arithmetic (v): needed on the toy PT-U). `eps` is taken from the registered
ladder `a*_pred {1, 2, 4}`; the rungs in order: `eps_1` plain, `eps_1` second-order, `eps_2` plain, and so on, the next only if
the previous did not find the branch. **Why `a*_pred`**: in the normal form `G = a (mu +
beta a^2)` past the bifurcation the Newton map is `N(a) = 2 beta a^3 / (mu + 3 beta a^2)`; from `a_0 > a* / sqrt(3)` it
converges to `a*`, from `a_0 < a* / sqrt(5)` to 0; so `eps = a*_pred` finds the branch if the prediction is not too large by
more than `sqrt(3)`, and the doubled values cover an underestimate of `a*` up to a factor `6.9`. **Outcome** of a switch: **B**
(on an asymmetric equilibrium) if it converges with `|a| > 1e-3 eps`, `a = phi_f . (u - u_s)`; **S** (back on the symmetric
branch) if it converges with `|a| <= 1e-3 eps`; **F** if it fails. When B is found with `+eps`, the switch with `-eps` (the same
`eps`) is made; its result must be the mirror image (`|u_- - P u_+|_inf / |u_+|_inf <= 1e-6`, estimate `1e-10`: the mirrored
iterates, rounding of the asymmetric assembly order amplified by `|K| / mu ~ 1e4`).

**The other side.** The switch at the other load (`lambda_f` when stiffening, `lt` when softening) with the same `eps` and
start (plain, then second-order) as the branch side's first rung: expected (for each start) **S** or **F** (no small-amplitude asymmetric equilibrium there: in the normal form `N` is a
contraction to 0, `|N(a)| < 2 |a| / 3`).

**Reading CR**: **"supercritical"** if CF is "stiffening", the switch at `lt` gives B and the switch at `lambda_f` gives S or F;
**"subcritical"** if CF is "softening", the switch at `lambda_f` gives B and the switch at `lt` S or F; **"undecided"**
otherwise. Checked at the B state: `a_s / a*_pred` in `[0.5, 2]` (the load-amplitude relation of D1 against the measured
`beta^`; the bounds allow the `O(a^4)` terms and the change of `beta` between `lambda_f` and `lambda_s`) and the inertia: `nu = 0`
if supercritical (the exchange of stability of D1: the symmetric branch has `nu = 1` at `lt`), `nu = 1` if subcritical (`nu =
0` on the symmetric branch at `lambda_f`).

### D4. The trace of the bifurcated branch (c1, c3) and the comparisons

**The grid.** `G = [0.963, 0.9635, 0.964, 0.9645, lambda_f, A, lt, 0.97, 0.975, 0.98, 0.985, 0.99, 0.995, 1.0]`. **The symmetric
branch**: the chain of `solve_increment` with `indefinite_tangent` from the observed `u_c` (0.9625, as ADR 0697) through every
grid load, each from the previous one (the states at `lambda_f`, `A`, `lt` used by D2 and D3 come from this chain). **F-bar**:
the same chain from F-bar's observed state at 0.9 through 0.9625 and every grid load (`indefinite_tangent`, so that a loss of
F-bar's stability would be measured, not refused).

**The bifurcated branch.** Supercritical: from the B state at `lt` upward through the grid loads `0.97 .. 1.0` by
`solve_increment` with `indefinite_tangent` (the predictor and Newton of ADR 0694 D2, the same function as the symmetric chain);
subcritical: from the B state at `lambda_f` downward through `0.9645 .. 0.963` and `0.9625` by the fixed-load Newton of D3 (the
start the previous state with the prescribed values of the new load; `solve_increment` does not take a decreasing load). Each
step is **accepted** if it converges and passes the continuity test of ADR 0697 D1 (`|u - u_from - d v_t|_inf <= |d v_t|_inf`,
`v_t = K^-1 f` at the previous state); otherwise the step is halved toward the same target, at most 4 halvings per grid step,
at most 28 solves in all (the caps); if a cap is reached the branch is **lost** at the last accepted load. At each accepted grid
load: the inertia `nu`, the amplitude `a = phi_f . (u - u_s)` (with `u_s` the symmetric state at that load), the asymmetry `|u
- P u|_inf / |u|_inf`, `R/mu`, `w_c`; the lowest four pairs at the first and the last state of the branch. Progress (each solve,
its time) is printed.

**The comparisons (c3)**: at each grid load reached by the branch: `R/mu` and `w_c` of the bifurcated and of the symmetric
branch against F-bar, `g = R_x / R_Fb - 1`, `w_x / w_Fb - 1`, and the inertia of the three.

### D5. The fast controls on the same code (`f783_fast`)

The forced ladder (D2), the switch (D3), the trace (D4) and the readings are written once over a **branch case** (the `Case`
of ADR 0697 with a fixed-load Newton with an external force on the free equations, and the reflection); the heavy record and
the controls call the same functions.

- **PT, a closed-form pitchfork with a symmetric coupling (c4).** Eight unknowns: `r_0 = (1 - p) u_0 + b3 u_0^3 + 2 c u_0 u_1`,
  `r_1 = 2 u_1 - 0.1 p + c u_0^2`, `r_i = k_i u_i - p b_i` (`k_i = 1 + i`, `b_i = 0.1 i`, `i = 2..7`), `c = 1` (the gradient of
  `E = (1 - p) u_0^2 / 2 + b3 u_0^4 / 4 + c u_0^2 u_1 + u_1^2 - 0.1 p u_1 + ...`, so the tangent is symmetric); the reflection
  `u_0 -> -u_0`; the load vector `f = -dr/dp = (u_0, 0.1, b_i)`. Symmetric path `u_0 = 0`, `u_1 = 0.05 p`, `mu(p) = 1 - 0.9 p`,
  `p_c = 1 / 0.9`, `alpha = -0.9`. The reduction is exact (`r_1` is linear in `u_1`): `G = a (1 - 0.9 p + (b3 - c^2) a^2)`, so
  `beta = b3 - 1` **while the cubic coefficient of `r_0` alone is `b3`**. **PT-S**: `b3 = 2`, `beta = +1` (supercritical);
  **PT-U**: `b3 = 0.5`, `beta = -0.5` (subcritical, though `r_0`'s own cubic is positive: a ladder that held `u_1` fixed would read
  it supercritical). `lambda_f = 1.0` (`mu = 0.1`), `A = 1.11` (`mu = 0.001`), `lt = 1.2` (`mu = -0.08`). The toy's Newton
  (dense, 20 iterations, `|r|_inf <= 1e-14`) both for the fixed-load solve and for the continuation (from the previous state);
  the lowest pair by `symmetric_eigen`. Checks: `phi_f = +-e_0`; `k_0` against 0.1; `z`; the window count; `beta^` against `beta`;
  CF; `alpha^` against -0.9 and `lambda_c^` against `1/0.9`; the switch: B at the first `eps`, `a_s` against the closed form
  (`a*^2 = (0.9 p - 1) / beta`: `0.08` at `p = 1.2` for PT-S, `0.2` at `p = 1.0` for PT-U), the mirror, `a_s / a*_pred`, the
  inertia (0 for PT-S, 1 for PT-U); the other side S or F; CR; the trace (PT-S upward `1.25, 1.3, 1.35, 1.4`; PT-U downward
  `0.95, 0.9, 0.85, 0.8`): every step accepted, `a(p)` against the closed form, `nu` (0, respectively 1) at each.
- **C5, the continuity test against a distant root (c5).** The bracket of ADR 0697 D1 (the same function, copied) on the toy
  `r_0 = u_0 e^-u_0 + u_0 / 20 - p`, `r_i = k_i u_i - p b_i`: from `p = 0.2` to `0.7` with 20 bisections. Newton from the lower
  end to 0.7 converges to the distant root `u_0 ~ 14` on the rising branch beyond the local minimum (arithmetic (iv)), where the
  inertia is 0 again: without the continuity test the bracket would take it as the same branch and read "no change". Checks: the
  first attempt converged and was **rejected by the continuity test** (`hi0` converged, not accepted) at `|u_0| >= 5`; the fold
  `p_c` (computed in the test by 200 bisections of `g'`) in the bracket; width `<= 0.5 / 2^20 (1 + 1e-9)`; the upper end lost;
  reading "limit point".
- **N8, the FEM plumbing on a stable block (negative control).** The `8^3` block, rule L, its symmetric state at 1.0 (the
  observed solve's last attempt, as ADR 0697 NC8; `nu = 0` there): the forced ladder with two levels, `k_0` against `mu_1(1.0)`
  `<= 1e-3` relative (the fixed-load solve with a force, the amplitude and the eigenvector on the FEM case); `z <= 1e-6` if
  `sigma(phi) = -1` within `1e-6` (else `sigma` reported); the switch at 1.0 with `eps = 1e-3`: outcome S (no branch on a stable
  path).

### D6. The fixed-load Newton on the FEM block

`solve_from` holds the model's constraints and the listed prescribed values at their values. The block model of the fixed-load
solves is the block of ADR 0697 without its punch constraint, the punch given as the list `Prescribed(n, 2, lambda * v)` with `v
= -0.1` read from `prescribed_equations` of the block model, and the start carrying `lambda * v` at every prescribed equation
(the same `f64` product `solve_increment` writes, so the states move between the two solves bit for bit at the prescribed
equations). The free equations, the tangent and the load vector are those of the block model. External load `eta phi_f` on the
free equations, zero elsewhere. `NewtonOptions` of ADR 0697 (`options(10).indefinite_tangent(true)`, 20 iterations, `1e-10`).

### D7. The test file `crates/rcfd-fem/tests/si_eas_hg16.rs`

Helpers copied from `si_eas_hg15.rs` (existing tests are not edited). Test-only: no library change is planned, so the default
bits are unchanged by construction (B).

## Checks

Every measure is required finite before it is compared (a non-finite value or a missing step misses).

### The fast test `f783_fast` (default, not ignored)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| PT-S | c4 | `|phi_f,0|`; `k_0` against 0.1; `z` max; window count; `beta^` against 1; CF | `>= 1 - 1e-12`; `<= 1e-9` rel; `<= 1e-12`; `>= 2`; `<= 1e-6` rel; "stiffening" | closed form |
| PT-S | c4 | `alpha^` against -0.9; `lambda_c^` against `1/0.9` | `<= 1e-9` rel; `<= 1e-9` abs | exact (linear `mu`) |
| PT-S | c1, c2, c4 | switch at 1.2: B at the first `eps` (plain or second-order); `a_s` against `sqrt(0.08)`; mirror; `a_s / a*_pred`; `nu`; at 1.0: S or F; CR | B; `<= 1e-10` rel; `<= 1e-12`; `[0.5, 2]`; `= 0`; holds; "supercritical" | closed form |
| PT-S | c1, c3, c4 | trace up to 1.4: every step accepted; `a(p)` against `sqrt(0.9 p - 1)`; `nu` | holds; `<= 1e-10` rel each; `= 0` each | closed form |
| PT-U | c4 | as PT-S: `beta^` against -0.5; CF | `<= 1e-6` rel; "softening" (the other rows as PT-S, `k_0` against 0.1) | closed form |
| PT-U | c1, c2, c4 | switch at 1.0: B; `a_s` against `sqrt(0.2)`; mirror; `a_s / a*_pred`; `nu`; at 1.2: S or F; CR | B; `<= 1e-10` rel; `<= 1e-12`; `[0.5, 2]`; `= 1`; holds; "subcritical" | closed form |
| PT-U | c1, c4 | trace down to 0.8: every step accepted; `a(p)` against `sqrt((1 - 0.9 p) / 0.5)`; `nu` | holds; `<= 1e-10` rel each; `= 1` each | closed form |
| C5 | c5 | the first attempt converged, rejected by the continuity test, `|u_0|`; `p_c` in `[a, b]`; `b - a`; upper end lost; reading | holds, `>= 5`; holds; `<= 4.77e-7`; lost; "limit point" | arithmetic (iv) |
| N8 | c1 | `8^3` at 1.0: `k_0` against `mu_1`; `z` (if antisymmetric); the switch with `eps = 1e-3` | `<= 1e-3` rel; `<= 1e-6`; S | stable path |

### The heavy record `f783_nh16_branch_heavy` (ignored, run once)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| OB | c1 | the observed `16^3` solve: the error text | equal to ADR 0678's | ADR 0697 OB |
| HS1 | c3 | the symmetric chain: converges at every grid load; `nu(lambda_f)`, `nu(A)`, `nu(lt)` | holds; `0, 0, 1` | ADR 0697 |
| HS2 | c3 | the symmetric state at `lt` against ADR 0694 (`0.32765165`, `0.06039238`) | `<= 1e-7` rel each | ADR 0697 H0 `2.8e-10` |
| HS3 | c3 | the symmetric state at 1.0 against ADR 0694 IN-F (`0.33937627`, `0.06245308`) | `<= 1e-6` rel each | the same root |
| HS4 | c3 | `|u - P u|_inf / |u|_inf` at every symmetric grid state | `<= 1e-9` | ADR 0697 `1e-11` |
| HM | c2 | `phi_f`: `sigma` within `1e-6` of -1; `|phi_f . phi_A|`; `mu_1(lambda_f) > 0`; `mu_1(A)` in `(0, 10]`; backward residuals; `lambda_c^` in `[A - 1e-7, B + 1e-7]` | holds; `>= 0.99`; holds; holds; `<= 1e-8`; holds | arithmetic (i), ADR 0697 |
| HF | c2 | the ladder at `lambda_f`: `k_0` against `mu_1(lambda_f)`; `z` at every level with both signs; window count; CF | `<= 1e-3` rel; `<= 1e-6`; `>= 2`; not "undecided" | arithmetic (ii), (iii) |
| HW | c1, c2 | the switch on the branch side: B within the `eps` ladder; mirror; `a_s / a*_pred`; `nu` (0 super, 1 sub); the other side S or F; CR | B; `<= 1e-6`; `[0.5, 2]`; as D3; holds; not "undecided" | D3 |
| HT | c1, c3 | the trace: every grid load of its direction reached within the caps (to 1.0 if supercritical, to 0.9625 if subcritical); `|a| >= 0.5 |a_s|` at each (it stays on the asymmetric branch); `nu`, `R/mu`, `w_c`, `a` at each | holds; holds; reported | D4 |
| HFB | c3 | F-bar: the chain converges; at 0.9625 against ADR 0694 FB; at 1.0 against ADR 0678 (`0.32599497`, `0.06060900`); `nu` at every load | holds; `<= 1e-6` rel each; `<= 1e-6` rel each; reported | the same roots |
| HG | c3 | at each load of the branch: `|R_x / R_Fb - 1|`, `|w_x / w_Fb - 1|` for the bifurcated and the symmetric state | `<= 0.15` each | ADR 0694 EQ |

The answers of c2 and c3 are the readings CF and CR and the table of the trace (reported; the limits above check that they are
what they claim to be).

### The default bits and the static checks

| Id | Clause | Measure | Limit |
|---|---|---|---|
| B | c6 | no file under `crates/*/src` differs from bbb71825 at the tip (`git diff --stat bbb71825 -- crates/*/src` empty) | empty |
| S7 | | `cargo test -p rcfd-fem --release --lib`; `--test si_eas_hg15`, `si_eas_hg14`, `si_eas_hg13` (their default tests) | pass |
| S6 | | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean |
| S8 | | `cargo test -p rcfd-cli --release --test docs_site` (docs/ touched by this ADR) | pass |
| S9 | | every commit of the branch builds (`cargo build -p rcfd-fem --release --tests` or the test target it adds) | builds |

`baseline_hashes` is not run: no shared solver file is touched (B checks it).

### Negative and positive controls

| Id | Control | Expected |
|---|---|---|
| PT-S | a closed-form supercritical pitchfork | "stiffening", "supercritical", the branch and its stable inertia in closed form |
| PT-U | a closed-form subcritical pitchfork whose own cubic term is positive | "softening", "subcritical" (catches a reduction without the symmetric modes) |
| C5 | a fold whose Newton converges to a distant root of the same inertia | rejected by the continuity test; "limit point" |
| N8 | the stable `8^3` block | the switch returns to the symmetric state |
| other side | the switch where the normal form has no branch (PT-S, PT-U, heavy) | S or F |

### Planned mutations (on a scratch copy of the test commit, each run once against `f783_fast`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the reading CF reversed (`beta > 0` read "softening") | PT-S, PT-U (CF, CR) |
| MU2 | the toy's fixed-load Newton updates only `u_0` (the symmetric unknowns held at the start: the reduction without `w`) | PT-U (CF not "softening", `beta^`) |
| MU3 | the continuity test dropped (every converged solve accepted) | C5 ("no change") |
| MU4 | the first `eps` of the switch's ladder `a*_pred / 4` in place of `a*_pred` (the others unchanged) | PT-S, PT-U (B not at the first `eps`) |
| MU5 | the switch perturbs along the normalised load vector `f` (symmetric) in place of `phi_f` | PT-S, PT-U (S, no branch); N8 not (S either way) |
| MU6 | the side rule reversed (the branch-side switch at `lambda_f` when stiffening and at `lt` when softening) | PT-S, PT-U (CR, the switch) |

The heavy record is not run under the
mutations (it uses the same functions, checked by the fast test).

## Decision rule

- **done**: every check of `f783_fast` met; OB, HS1 to HS4, HM, HF, HW, HT, HFB, HG met in their single registered run; CR
  "supercritical" with the branch traced to 1.0 (c3 answered to full load); B, S6, S7, S8, S9 met; every mutation caught; no
  amendment.
- **partial**: B met, but a check missed, CR "undecided", CR "subcritical" (the `lambda`-controlled trace cannot reach full load on
  a subcritical branch: c3 then answered only below the bifurcation, the path to full load needing the arc-length driver), the
  branch lost before 1.0, a mutation not caught, or a check met only after an amendment.
- **todo**: B fails (a default bit could change), or OB fails and nothing is measured.

## Limits (not covered)

- One mesh, one material, one load path; the bifurcation of ADR 0697 only (the second pair, `~2297` at `A`, may itself
  bifurcate later; the inertia along the traces reports it, nothing is switched onto it).
- `beta` is measured at `lambda_f`, not at `lambda_c` (`O(lambda_c - lambda_f)`), with a two-level quadratic fit; higher
  terms of `G` are not identified.
- A subcritical branch is traced downward under load control only; its fold (if any) and the path to full load would need the
  arc-length driver (not run).
- No imperfection sensitivity of the real (imperfect) block is computed; the symmetric block is the model.
- Rule L's coefficient is not in scope (the maintainer's call, ADR 0682 open question 2); no opt-in becomes a default.

## Open questions (taken or open)

1. **The design** (the forced response as the measure of `beta`, `lambda_f = 0.9647`, the ladder `2e-4 2^j`, the window, the
   switch's `eps` ladder from the predicted amplitude, the grid and the caps, the toys, the new test file `si_eas_hg16.rs`):
   taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. **Whether either opt-in of ADR 0694 becomes a default**: not taken (it would change default results); open, as in ADR 0694.

## Amendment 1 (2026-11-17, written and committed before the corrected run)

**The defect.** The registered run of `f783_fast` (on aaa276f9, `f783-scratch/run1-fast.log`) stopped before its first check:
the symmetric state of the toy PT is computed by the toy's Newton from `u = 0`, where the tangent's first entry is `1 - p + 3
b3 u_0^2 + 2 c u_1 = 1 - p`, exactly zero at the force load `lambda_f = 1.0`; the dense elimination refuses the zero pivot
("singular") and the test panics at the `unwrap` of that set-up solve. A defect of the set-up (the start of an auxiliary solve),
not of a registered measure: nothing was measured, and the heavy record was not run.

**The correction.** The symmetric state of PT is solved by the same Newton from its closed form, `u_0 = 0`, `u_1 = 0.05 p`,
`u_i = p b_i / k_i` (D5), where the first entry is `mu(p) = 1 - 0.9 p` (`0.1` at 1.0). No limit, no other row and no other
function changes. The registered test becomes the ignored record `f783_fast_known_miss` (its set-up unchanged, the panic in its
ignore text); the corrected test `f783_fast_amended` is run once. The planned mutations are run against `f783_fast_amended`
(the registered test measures nothing). The decision can be at best **partial** (a check met only after an amendment).

## Amendment 2 (2026-11-17, written and committed before the corrected heavy run)

**The defect.** The registered heavy record (on 158385e5 with amendment 1, `f783-scratch/run3-heavy.log`, 79 s) met OB, HS1 to
HS4 and HM (`mu_1(lambda_f) = 873.67`, `mu_1(A) = 2.554`, `mu_1^sym(lt) = -3271.88`, `sigma(phi_f) = -1` to `2e-15`, `|phi_f .
phi_A| = 0.998`, `lambda_c^ = 0.964900029`), but the forced ladder of D2 missed: from its first level (`a_0 = 2e-4`, `eta =
0.175`) the fixed-load Newton converged to an equilibrium with the amplitude of the **opposite** sign (`a_+ = -5.44e-5`), the
amplitudes then grew like `eta^(1/4)`, the sixth level failed (the internal modes of element 3720 did not converge), `k_0 =
2180` against `mu_1 = 873.7` (HF `1.50 > 1e-3`), no level in the window, CF "undecided", so no switch (HW missed). A probe after
the run on a scratch copy (`f783-scratch/probe-forced.log`, diagnosis only, not a registered result) measured the internal
force along `phi_f` at `u_s + t phi_f`: the secant stiffness is `873.7` at `t = 1e-8`, `881.4` at `1e-6`, `943` at `3e-6`, `1642`
at `1e-5` (a frozen cubic coefficient about `7.7e12`), and the forced solve from `u_s` gives `a / a_target = 1.0000`, `1.0013`,
`1.281` at the targets `1e-7`, `1e-6`, `1e-5` and fails at `5e-5`: the amplitude scale of the reduced equation is `~3e-5`, not
the `7e-3` of arithmetic (ii) (my dimensional estimate of `beta` was low by five orders: the hourglass mode of rule L's element
is far stiffer in its cubic term than the continuum estimate). The registered ladder began above the fold of the forced path
(`eta_max / mu ~ 1e-5`), so its first Newton solve could not find the near equilibrium. A defect of the set-up's parameter
`a_0`, from a wrong estimate, not of a measure.

**The correction.** The heavy record's ladder starts at `a_0 = 1e-7` (ten times the worst-case amplitude noise `1e-8` of
arithmetic (iii)) and has 13 levels (`a_0 2^12 = 4.1e-4`), ending at the first failure as registered; it then spans the window
of D2 wherever the amplitude scale lies between about `1e-6` and `4e-4`, without a prior value of `beta`. Everything else of
D2 to D4 and every limit is unchanged; the fast test (whose toys keep `a_0 = 2e-4`, 9 levels) is unchanged. The registered heavy
test becomes the ignored record `f783_nh16_branch_heavy_known_miss` (unchanged, the measured numbers in its ignore text); the
corrected `f783_nh16_branch_heavy_amended` is run once. The decision can be at best **partial**.
