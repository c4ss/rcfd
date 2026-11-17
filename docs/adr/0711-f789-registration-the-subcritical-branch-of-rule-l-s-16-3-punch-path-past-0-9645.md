# ADR 0711: F-789 registration: the subcritical branch of rule L's 16^3 punch path past 0.9645 (the bifurcated branch of ADR 0700 traced toward lower loads under load control with a continuity test that also bounds the change of the amplitude along the critical mode, and under amplitude control to its fold if it folds; the inertia and R/mu along it against the symmetric branch and F-bar)

Status: **REGISTERED 2026-11-17, written before any code of F-789 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no test code of F-789 was written and nothing of this item was run on the block (no probe). The
numbers quoted below come from the published log of ADR 0700 (`w49i-work/f783-scratch/run4-heavy-amended.log`) and from the
ADR texts; the arithmetic on the closed-form toys was done beforehand in a scratch Python script (disclosed below). Results go
in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-789 (deps F-783), the follow-up proposed by
[ADR 0700](0700-f783-registration-the-bifurcated-branch-of-rule-l-s-16-3-punch-path.md) (its remaining question 1). Number
0711 given by the lead. Branch `f789-work` from b3af848a (F-774 / ADR 0694, F-779 / ADR 0697 and F-783 / ADR 0700 merged),
worktree `w49i-work/f789`, scratch `w49i-work/f789-scratch`. The block, the element and the material are those of ADR 0700:
the incompatible-modes Hex8 at finite strain with rule L (`IncompatibleStabilisation::LinearStress`), the neo-Hookean material
of ADR 0636 (`mu = 1e6`, `nu = 0.49`), the punch block of ADR 0678 reading c1 (`[0, 1]^3` on `16^3`, `u_z = -0.1 lambda` on
the `zmax` nodes with `x, y <= 0.5`), four threads, the supernodal factor, as `si_eas_hg16.rs` builds it.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would normally be asked about is
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-789 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The bifurcated branch of ADR 0700 traced from its state at 0.9645 toward lower loads with a continuity test that also bounds the change of the amplitude along the critical mode, and by the arc-length driver to its fold if it folds, with the inertia and R/mu along it against the symmetric branch and F-bar; a fast control on the closed-form subcritical pitchfork of ADR 0700 and on a step that falls back onto the symmetric branch; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** the branch traced from its state at 0.9645 toward lower loads with a
continuity test that also bounds the change of the amplitude along the critical mode; **c2** traced by the arc-length driver to
its fold if it folds; **c3** the inertia and `R/mu` along it against the symmetric branch and F-bar; **c4** a fast control on
the closed-form subcritical pitchfork of ADR 0700; **c5** a fast control on a step that falls back onto the symmetric branch;
**c6** the default bits unchanged.

From the brief, also registered here: if the arc-length driver must accept `indefinite_tangent`, that is a library opt-in, off
by default, registered before the run, with `baseline_hashes` and the default-path bits shown; if it cannot be done inside
this item, the fold is bounded from an amplitude-controlled trace and that is said. Heavy `16^3` runs capped in steps, progress
printed, `#[ignore]`d and run once, with fast tests on the toys. Rule L's coefficient and any default change are not in scope.
Reruns by name: `si_eas_hg16`, `si_eas_hg15`, `si_eas_hg14`, `si_eas_hg13`, the arc-length test files (`sa_arclength`,
`sa_arclength_sphere_f649`), `--lib`.

## What was read

- `AGENT_RULES.md`; the brief; ADR 0700 (whole, and `run4-heavy-amended.log`); ADR 0697 D1 (the continuity test); ADR 0694
  D2 (`indefinite_tangent`).
- `crates/rcfd-fem/tests/si_eas_hg16.rs` (whole: `Case`, `Branch`, `FemBranch`, `Pitch`, `forced`, `switch_one`, `switch`,
  `trace`, `continuous`, the fast and the heavy tests).
- `crates/rcfd-fem/src/arclength.rs` (whole) and in `crates/rcfd-fem/src/nonlinear.rs` `controlled_attempt`,
  `controlled_correction`, `LoadFactorControl`, `Problem::attempt_with` / `iterate` (the residual reference and the
  convergence test: the free residual's 2-norm relative to `max(|lt f_free|, |f_int|)` at most `1e-10`, the energy of the last
  correction at most `1e-10` times that of the first, `k >= 1`, 20 iterations), `solve_from`, `tangent`, `internal_force`.

## What is known before the run (from ADR 0700's log)

1. `lambda_c^ = 0.964900029325`, `alpha^ = -4.367713e6` (the slope of `mu_1`), `beta^ = -1.157900e12` (the cubic coefficient
   of the reduced equation at 0.9647): a subcritical pitchfork. With these, D1 of ADR 0700 predicts the branch amplitude
   `a_red(lambda) = sqrt(|alpha^| (lambda_c^ - lambda) / |beta^|)`: `2.75e-5` at 0.9647, `3.88e-5` at 0.9645, `5.83e-5` at
   0.964, `7.27e-5` at 0.9635, `8.47e-5` at 0.963, `9.51e-5` at 0.9625 (measured: `2.695e-5`, `3.827e-5` at the first two).
2. The B state at 0.9647: the plain switch from the symmetric state with `eps = 2.7468713663317314e-5` (`a*_pred`, the `f64`
   printed by the test's `Debug` form, so the literal is that value) gives `a = +2.694982374354194e-5`, `nu = 1`, lowest pair
   `-1760.44`; the fixed-load Newton from it to 0.9645 gives `a = +3.827194476e-5`, `nu = 1`, `R/mu = 0.32726797`,
   `|u - P u|_inf / |u|_inf = 2.745e-4`. The symmetric state at 0.9645: `R/mu = 0.32726801`; F-bar there `0.31488787`.
   `mu_1(0.9647) = 873.6706402833074`.
3. **The fall-back**: the fixed-load Newton from the 0.9645 state to 0.964 converged to `a = -1.22e-11` (the symmetric root,
   `nu = 0`), and the continuity test of ADR 0697 D1 (`|u - u_from - d v_t|_inf <= |d v_t|_inf`) accepted it: its bound is the
   inf norm of the whole predicted change, dominated by the punch region (`~1e-4` per `5e-4` of load), while the amplitude
   change along the mode is `~4e-5` spread over ~24 equations (components `~1e-5`).
4. Cost (ADR 0700 log): the observed `16^3` rule L solve 20.7 s; F-bar 11.9 s; one fixed-load or increment solve 1.5 to 2.5
   s; a lowest-pairs solve 1.4 to 1.7 s.

**Arithmetic beforehand (disclosed).** (i) Along the branch `a^2 = mu / |beta|` with `mu ~ |alpha| (lambda_c - lambda)`, so
`da/dlambda = -a / (2 (lambda_c - lambda))`: `-0.048` at 0.9645; for the step to 0.964 (`d = -5e-4`) the linear prediction of
the amplitude change is `+2.4e-5` against the reduced equation's `+2.0e-5` (`0.17` relative), and the fall-back of item 3
changes it by `-3.83e-5`. (ii) In a scratch Python script (`f789-scratch/scripts/toys.py`, the closed form and the toy Newton
only, no test code), on the toy PT of ADR 0700 D5 generalised to `r_0 = (1 - p) u_0 + b3 u_0^3 + b5 u_0^5 + 2 u_0 u_1`, `r_i =
k_i u_i - p sb b_i` (`i >= 2`): PT-U (`b3 = 0.5`, `b5 = 0`, `sb = 1`) traced down from its branch at 1.0 to 0.95, 0.9, 0.85,
0.8 by the fixed-load Newton passes both the old test and the amplitude bound of D2 with `theta_a = 0.5` at every step (ratios
`|da - da_pred| / |da_pred|` 0.09 to 0.07); with `sb = 100` (PT-UF) the single step from 1.0 to 0.8 converges to the
symmetric root (`u_0 = 0`), **accepted by the old test** (`0.850 <= 1.750`) and **rejected by the amplitude bound** (`da =
-0.447` against the predicted `+0.402`), and with one halving the trace reaches 0.8 on the branch (`a = 0.748331`); PT-U5
(`b3 = 0.5`, `b5 = 0.25`, `sb = 1`) traced from its branch at 1.0 (`a = 0.474767`) by the amplitude-controlled Newton of D3
over the rungs `a_b 2^(k/2)` gives `p` equal to the closed form `(1 - 0.5 a^2 + 0.25 a^4) / 0.9` to `4e-16`, `dp/da` from
`-0.1038` (rung 2, `a = 0.9495`) to `+1.1985` (rung 3, `a = 1.3428`), `nu` from 1 to 0 across: the fold `a = 1`, `p = 0.833333`
is bracketed, the tangent-intersection lower bound of D3 is `0.8125` and the upper bound `0.8360`, the estimate of D3 `0.8344`.
PT-U (no quintic) by the same trace: `dp/da < 0` at each rung, no fold. These fixed the toys' parameters below.

## Design

### D1. The arc-length driver cannot trace this branch inside this item (c2): the substitute

`arclength::trace` (ADR 0480 D4) starts from the unloaded state, refuses any non-zero prescribed value (key `"arc length"`)
and takes the load direction from nodal forces; the punch of this block is a prescribed displacement `u_z = -0.1 lambda`, and
the branch must be started from a given converged state (the B state at 0.9645). Making the driver trace it needs, in the
controlled iteration of `nonlinear.rs` (`Problem::iterate` with a `LoadFactorControl`), the prescribed values carried at the
control's load factor at every iteration and the load column `-K_fp v` of the prescribed displacements in `du_f`, and a start
from a given state and a given previous increment in `arclength.rs`: a change of the shared Newton iteration that every solve
of the crate runs. The refusal of `indefinite_tangent` is not the obstacle (the controlled correction already factors the
indefinite tangent, ADR 0694 D2). This is not done inside F-789 (it would put the core iteration of every default solve at
risk for one test; its own item, with `baseline_hashes`, is proposed at the end). **Following the brief, the fold is bounded
from an amplitude-controlled trace (D3), written in the test over the library's public `tangent` and `internal_force`, and
this is said in the results.** Amplitude control is the natural parameter of a pitchfork branch (Keller's pseudo-arclength
idea with the constraint along the critical mode; Keller, in Applications of Bifurcation Theory, Academic Press 1977, cited
from memory; derived here from first principles in D3): it passes through a fold in the load, where the load-controlled Newton
cannot, as long as the amplitude itself does not turn back. No library file changes: the default bits are unchanged by
construction (check B).

### D2. The load-controlled trace with the amended continuity test (c1)

From the B state at 0.9645 (D4), down the grid `0.964, 0.9635, 0.963, 0.9625` (the last the observed committed load `lc`,
ADR 0700 D4) by the fixed-load Newton of ADR 0700 D6 (`FemBranch::fixed` with no force: the start is the previous state with
the prescribed values of the new load; `options(10).indefinite_tangent(true)`). A converged step from `(u_from, lambda_from)`
to `lambda` with `d = lambda - lambda_from` and `v_t = K^{-1} f` at the previous state is **accepted** if both:

- **the test of ADR 0697 D1**: `|u - u_from - d v_t|_inf <= |d v_t|_inf` (free equations);
- **the amplitude bound** (new): with `Da = phi_f . (u - u_from)` and `Da_pred = d (phi_f . v_t)` (`phi_f` the critical mode
  at 0.9647, unit 2-norm, on the free equations), `|Da - Da_pred| <= theta_a |Da_pred|`, **`theta_a = 0.5`**.

Why: on the branch `phi . v_t = da/dlambda`, so `Da_pred` is the linear prediction of the change of the amplitude; a fall-back
onto the symmetric branch changes the amplitude by `-a_from`, of the opposite sign to `Da_pred` when the step goes away from
the bifurcation (here, downward on a subcritical branch: `Da_pred > 0` for `a > 0`), so `|Da - Da_pred| = a_from + Da_pred >
theta_a |Da_pred|` for any `theta_a <= 1`. A true branch step has a relative prediction error of `O(d / (lambda_c - lambda))`
(arithmetic (i): 0.17 for the full grid step, quartered by each halving); `theta_a = 0.5` admits that and is the midpoint
between no tolerance and the `theta = 1` of the D1 test. Otherwise the step is halved toward the same target, at most 4
halvings per grid step, at most 28 solves in all (ADR 0700 D4's caps); if a cap is reached the branch is lost at the last
accepted load. Each attempt prints the outcome of both tests and their numbers.

At each accepted grid load: `nu`, `a = phi_f . (u - u_s)` (with `u_s` the symmetric state there), `a / a_red`, the asymmetry,
`R/mu`, `w_c`, and the symmetric branch and F-bar at the same load (c3).

### D3. The amplitude-controlled trace (c2 substitute, c3)

**The bordered Newton (first principles).** On the free equations `R(u, lambda) = f_int,F(u)` with the prescribed values
`lambda v` (`v` the model's constraint values at load 1, `-0.1` on the punch, 0 elsewhere): `dR/du = K` (the reduced tangent),
`dR/dlambda = K_Fp v = -f` (`f` the reduced right-hand side of `reduce` with the constraint values, as `FemCase::system`
builds it). A general linear constraint `g . u_F + h lambda = c` closes the system. Newton: `K y_1 = -R`, `K y_2 = f`, `dl =
(c - g . u_F - h lambda - g . y_1) / (g . y_2 + h)`, `du = y_1 + dl y_2` (the block elimination of the bordered matrix
`[K, -f; g^T, h]`; regular as long as the bordered matrix is, i.e. through a simple fold in `lambda` where `K` is singular only
at the fold point itself). After each update the prescribed values are set to `lambda v` (the same `f64` product as
`FemBranch::fixed`). Amplitude control: `g = phi_f`, `h = 0`, `c = a_t`. The factor: the supernodal `L D L^T` with the
`NonZero` rule of the symmetrised `K` (`factor_nonzero`, as the inertia of ADR 0694), refused pivots are a failure. The
element states: those of the start state throughout (neo-Hookean: no history; `incompatible_mode_warm_start` off, so the
states carry no modes). **Convergence** (the driver's criteria): at `k >= 1`, the free residual's 2-norm relative to
`|f_int|_2` (all equations; no external force) at most `1e-10`, the energy `|du . (dl f - R)|` of the last correction at most
`1e-10` times that of the first, and `|g . u_F + h lambda - c| <= 1e-9 max(|c|, 1e-300)`; 20 iterations. On the toys: `|r|_inf
<= 1e-14` (their Newton's test) and the constraint, no energy test.

**The amplitude.** `a = phi_f . u_F` (absolute: the symmetric states have `phi_f . u_s = 0` up to rounding, `P` being a
signed permutation and `phi_f` antisymmetric; checked at 0.9645: `|phi_f . u_s| <= 1e-9`).

**The trace.** From the B state `(u_b, 0.9645)` with `a_b = phi_f . u_b`: the rungs `a_k = a_b 2^(k/2)`, `k = 1 .. 18` (`a_18
= 512 a_b ~ 0.0196`; the components of the unit mode are `~0.2`, so the nodal hourglass reaches `~4e-3`, 6 per cent of the
element size `1/16`). From the current state with `v_t = K^{-1} f` and `s = phi_f . v_t` (`= da/dlambda`): the **tangent
predictor** `dp = (a_t - a) / s`, `u_F,pred = u_F + dp v_t`, `lambda_pred = lambda + dp`; the bordered Newton from it. A rung is
**accepted** if it converges and `|u - u_pred|_inf <= |u_pred - u_from|_inf` (free equations; the D1 test on the amplitude
step). Otherwise the amplitude step is halved toward the rung target, at most 4 halvings per rung, at most 40 bordered solves
in all. Stop: when the fold is bracketed and two further rungs are accepted; at `k = 18`; when `lambda` leaves `[0.5, 1.2]`;
or when a cap is reached (the branch lost at the last accepted amplitude).

**The fold.** At every accepted state the slope `sigma = dlambda/da = 1 / (phi_f . v_t)`. A **fold** is read between two
consecutive accepted states `k-1, k` (the start, halved and rung states alike) when `sigma_{k-1} < 0 <= sigma_k` (the load
decreases, then increases with the amplitude) **and** `nu` changes by exactly one between them (a simple fold); then the fold
amplitude is in `[a_{k-1}, a_k]`, the fold load `lambda_f <= U = min(lambda_{k-1}, lambda_k)` and, where `lambda(a)` is convex
on the segment, `lambda_f >= L`, the load at the intersection of the tangent lines at the two states; the estimate
`a_f^ = a_{k-1} - sigma_{k-1} (a_k - a_{k-1}) / (sigma_k - sigma_{k-1})`, `lambda_f^ = lambda_{k-1} + sigma_{k-1} (a_f^ -
a_{k-1}) / 2` (the slope linear in `a`). A sign change without the change of `nu` by one reads **"undecided"**; no sign change
up to the last accepted state reads **"no fold up to a = ..."**; a lost trace before any sign change reads **"lost at a = ...,
lambda = ..."** with the last `sigma`. The lowest pairs (`lowest_any`) at the start, at the two states of the fold bracket and
at the last state.

**The cross-check with D2.** For each accepted grid state `(lambda_j, a_j)` of D2 (absolute amplitude) with `a_j` between two
consecutive accepted states of D3: the cubic Hermite interpolant in `a` of `lambda` with the slopes `sigma` at those two states,
`lambda_H(a_j)`, against `lambda_j`.

**The comparisons (c3).** At each accepted rung state with `lambda` in `[0.5, 1.2]`: the symmetric state and the F-bar state at
the same load, each by the fixed-load Newton chained from the previous comparison state of its branch (from their states at
0.9645), a step larger than `0.02` in load split into equal substeps of at most `0.02`; `nu`, `R/mu`, `w_c` of the three.

### D4. The start (reproducing ADR 0700's state at 0.9645)

The observed `16^3` solve (OB) gives `u_c` at 0.9625; the symmetric chain by `solve_increment` with `indefinite_tangent` from
`u_c` through `0.963, 0.9635, 0.964, 0.9645, 0.9647` (ADR 0700's chain prefix, so the same states bit for bit); `phi_f` the
lowest pair at 0.9647 (sign by its largest component); the plain switch at 0.9647 with `eps = 2.7468713663317314e-5` along
`+phi_f`; the fixed-load Newton from it to 0.9645. F-bar: the observed ten-increment solve, then `solve_increment` with
`indefinite_tangent` from its last committed state through `0.9625, 0.963, 0.9635, 0.964, 0.9645` (ADR 0700 HFB's chain
prefix).

### D5. The fast controls on the same code (`f789_fast`)

The amended trace (D2), the bordered Newton and the amplitude trace (D3), the fold reading and the cross-check are written
once over the branch case of ADR 0700 extended with the residual, the load-setting and the convergence measure; the heavy
record and the controls call the same functions.

- **PT-U (c4)**: ADR 0700's subcritical toy (`b3 = 0.5`, `beta = -0.5`), its branch state at 1.0 from the closed form (`u_0 =
  sqrt(0.2)`, `u_1 = (0.1 - 0.2) / 2`, `u_i = b_i / k_i`) polished by the toy's Newton. (a) The D2 trace down `0.95, 0.9, 0.85,
  0.8`: every step accepted, `a(p)` against `sqrt((1 - 0.9 p) / 0.5)`, `nu = 1`, `a` increasing. (b) The D3 trace with 2
  rungs: `p` against the closed form `(1 - 0.5 a^2) / 0.9`, `nu = 1`, reading "no fold". (c) The cross-check of (a)'s states
  against (b) (the interpolant is exact for a quadratic `lambda(a)`).
- **PT-UF, the fall-back (c5)**: PT-U with `sb = 100` (the symmetric unknowns `u_2 .. u_7` respond a hundred times more
  strongly to the load, as the punch region does on the block; the reduction is unchanged since they are uncoupled). (a) The
  single fixed-load step from the branch at 1.0 to 0.8: converges with `|u_0| <= 1e-12` (the symmetric root), accepted by the
  test of ADR 0697 D1, rejected by the amended test. (b) The D2 trace to 0.8: reaches it, at least one attempt rejected, `a(0.8)`
  against `sqrt(0.56)`, `nu = 1`.
- **PT-U5, a closed-form fold (c2)**: `b3 = 0.5`, `b5 = 0.25` (the reduction stays exact: `G = a (1 - 0.9 p - 0.5 a^2 + 0.25
  a^4)`, the fold at `a = 1`, `p_f = 0.75 / 0.9`), from its branch at 1.0 (the small root `a^2 = (0.5 - sqrt(0.15)) / 0.5`):
  the D3 trace with 5 rungs: `p` against the closed form at each, the reading "fold", `a = 1` in the bracket, `p_f` in `[L,
  U]`, the estimate, `nu` 1 before and 0 after.
- **N8, the FEM plumbing (negative control of the bordered Newton)**: the stable `8^3` block, its symmetric state at 1.0 (as
  ADR 0700 N8). (a) Load control (`g = 0`, `h = 1`, `c = 0.99`) from the state at 1.0 against `FemBranch::fixed` to 0.99. (b)
  Corner control (`g` the unit vector of the corner's `u_z`, `h = 0`, `c` = that component of the fixed-load state at 0.99) from
  the state at 1.0: the load and the state of (a)'s reference recovered.

## Checks

Every measure is required finite before it is compared (a non-finite value or a missing step misses).

### The fast test `f789_fast` (default, not ignored)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| PU-a | c1, c4 | PT-U D2 trace: every step accepted (4 of 4); `a(p)` against the closed form; `nu`; `a` increasing | holds; `<= 1e-10` rel each; `= 1` each; holds | arithmetic (ii) |
| PU-b | c2, c4 | PT-U D3 trace, 2 rungs: both accepted; `p` against `(1 - 0.5 a^2) / 0.9`; `nu`; reading | holds; `<= 1e-12` abs each; `= 1` each; "no fold" | arithmetic (ii) |
| PU-c | c1, c2 | `|lambda_H(a_j) - lambda_j|` for the PU-a states inside PU-b's range (at least 3) | `<= 1e-12` each | exact for a quadratic |
| PF-a | c5 | PT-UF step 1.0 to 0.8: converged; `|u_0|`; the D1 test; the amended test | holds; `<= 1e-12`; accepts; rejects | arithmetic (ii) |
| PF-b | c1, c5 | PT-UF D2 trace to 0.8: reached; rejected attempts; `a(0.8)` against `sqrt(0.56)`; `nu` | holds; `>= 1`; `<= 1e-10` rel; `= 1` | arithmetic (ii) |
| P5 | c2 | PT-U5 D3 trace, 5 rungs: all accepted; `p` against the closed form; reading; `1` in `[a_{k-1}, a_k]`; `p_f` in `[L, U]`; `|lambda_f^ - p_f|`; `nu` before, after | holds; `<= 1e-12` abs each; "fold"; holds; holds; `<= 5e-3`; `1`, `0` | arithmetic (ii): `1.1e-3` |
| N8 | c2 | `8^3`: (a) `|u - u_fixed|_inf / |u_fixed|_inf`, `|lambda - 0.99|`; (b) the same against the fixed-load state, `|lambda - 0.99|` | `<= 1e-8`, `<= 1e-14`; `<= 1e-8`, `<= 1e-8` | the same root, Newton tolerance |

### The heavy record `f789_nh16_subcritical_heavy` (ignored, run once)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| OB | | the observed `16^3` solve: the error text | equal to ADR 0678's | ADR 0700 OB |
| R0 | c1 | the symmetric chain converges; `R/mu` at 0.9645 against `0.32726801`; `mu_1(0.9647)` against `873.6706402833074`; `|sigma(phi_f) + 1|`; `|phi_f . u_s(0.9645)|` | holds; `<= 1e-7` rel; `<= 1e-9` rel; `<= 1e-6`; `<= 1e-9` | ADR 0700, the same states |
| R1 | c1 | the switch: B with `a` against `2.694982374354194e-5`; the step to 0.9645: `a` against `3.827194476e-5`, `R/mu` against `0.32726797`, `nu` | `<= 1e-9` rel; `<= 1e-8` rel, `<= 1e-7` rel, `= 1` | the same root |
| HL0 | c1, c5 | the first attempt 0.9645 to 0.964 (ADR 0700's fall-back): converged with `|a| <= 1e-3 a(0.9645)`; the D1 test accepts; the amended test rejects | holds; holds; holds | ADR 0700 HT |
| HL1 | c1 | the D2 trace: every grid load reached within the caps (4 of 4) | holds | arithmetic (i) |
| HL2 | c1, c3 | at each reached grid load: `nu`; `a` larger than at the previous load; `a / a_red` | `= 1`; holds; `[0.5, 2]` | D1 of ADR 0700 |
| HL3 | c3 | at each: `|R_bif / R_sym - 1|`; `|R_x / R_Fb - 1|`, `|w_x / w_Fb - 1|` for the bifurcated and the symmetric state | `<= 1e-5`; `<= 0.15` each | ADR 0700 `1.2e-7`, HG |
| AT0 | c2 | the bordered Newton at the start: from `(u_b, 0.96451)` with `c = a_b`: `|lambda - 0.9645|`, `|u - u_b|_inf / |u_b|_inf` | `<= 1e-9`; `<= 1e-8` | the same root |
| AT1 | c2 | the first rung: accepted; `(lambda_c^ - lambda) |alpha^| / (|beta^| a^2)` | holds; `[0.25, 4]` (the square of the `[0.5, 2]` of HL2) | the reduced equation |
| AT2 | c2 | every accepted state: the constraint; the D3 continuity; the trace reaches a stop (fold + 2, `k = 18`, the load range) rather than being lost | `<= 1e-9` rel; holds; reported (lost is a reading, not a miss) | D3 |
| AT3 | c1, c2 | `|lambda_H(a_j) - lambda_j|` for the D2 states inside the D3 range | `<= 1e-2 (lambda_c^ - lambda_j)` each | the Hermite error of a near-quadratic |
| AF | c2 | the reading (fold / no fold / lost / undecided); if "fold": `[a_{k-1}, a_k]`, `[L, U]`, `lambda_f^`, `nu` on both sides | not "undecided"; reported | D3 |
| AC | c3 | at each compared rung state: the symmetric state symmetric (`|u - P u|_inf / |u|_inf`); `|R_x / R_Fb - 1|`, `|w_x / w_Fb - 1|` (bifurcated, symmetric); `nu` of the three | `<= 1e-9`; `<= 0.15` each; reported | ADR 0700 HS4, HG |
| HFB | c3 | F-bar: the chain converges; at 0.9645 `R/mu` against `0.31488787` | holds; `<= 1e-7` rel | ADR 0700 HFB |

The answers of c1 to c3 are the table of the D2 trace, the table of the D3 trace and the reading AF (reported; the limits
above check that they are what they claim to be).

### The default bits and the static checks

| Id | Clause | Measure | Limit |
|---|---|---|---|
| B | c6 | `git diff --stat b3af848a -- crates/*/src` empty at the tip | empty |
| S7 | | `cargo test -p rcfd-fem --release --lib`; `--test si_eas_hg16`, `si_eas_hg15`, `si_eas_hg14`, `si_eas_hg13`, `sa_arclength`, `sa_arclength_sphere_f649` (their default tests) | pass |
| S6 | | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean |
| S8 | | `cargo test -p rcfd-cli --release --test docs_site` (docs/ touched by this ADR) | pass |
| S9 | | every commit builds (`cargo build -p rcfd-fem --release --tests` or the test target it adds) | builds |

`baseline_hashes` is not run: no shared solver file is touched (B checks it).

### Negative and positive controls

| Id | Control | Expected |
|---|---|---|
| PU | the closed-form subcritical pitchfork of ADR 0700 | every amended step accepted; the closed form; "no fold" |
| PF | a step that falls back onto the symmetric branch and passes the test of ADR 0697 D1 | rejected by the amended test; the halved trace stays on the branch |
| P5 | a closed-form subcritical branch with a fold | "fold", bracketed and bounded |
| N8 | the bordered Newton on the stable `8^3` block under load and corner control | the fixed-load state |
| HL0 | ADR 0700's fall-back on the block | rejected |

### Planned mutations (on a scratch copy of the test commit, each run once against `f789_fast`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the amended test without the amplitude bound (the D1 test alone) | PF-a (accepts), PF-b (no rejection, `a(0.8)` off) |
| MU2 | `Da_pred` with the opposite sign | PU-a (steps rejected, lost), PF-b |
| MU3 | the bordered Newton updates `lambda` by `-dl` | PU-b, P5 (not converged or off the closed form), N8 |
| MU4 | the Hermite interpolant with the slopes `phi . v_t` in place of `1 / (phi . v_t)` | PU-c |
| MU5 | the fold read on `sigma_{k-1} > 0 >= sigma_k` (reversed) | P5 (no fold) |

The heavy record is not run under the mutations.

## Decision rule

- **done**: every check of `f789_fast` and of the heavy record met in their single registered runs, B, S6 to S9 met, every
  mutation caught, no amendment, **and the fold (if any) traced by the library's arc-length driver** (c2 as worded).
- **partial**: B met, but c2 answered by the amplitude-controlled trace of D1 in place of the arc-length driver (this is
  registered now: it is the expected outcome), or a check missed, a mutation not caught, or a check met only after an
  amendment.
- **todo**: B fails, or OB fails and nothing is measured.

## Limits (not covered)

- The library arc-length driver is not extended (D1); the fold is bounded from the amplitude-controlled trace. If the
  amplitude itself turns back along the branch, the amplitude-controlled trace cannot pass that point (it is lost there).
- One mesh, one material, one load path; only the branch of ADR 0700 with `+phi_f` (its mirror image is the same branch by the
  symmetry, ADR 0700 HW).
- No imperfection sensitivity of the real block; no switch onto any further bifurcation along the branch (the inertia reports
  it).
- The fold bound `L` assumes `lambda(a)` convex on the bracket segment (reported, not proven).
- Rule L's coefficient is not in scope (ADR 0682 open question 2); no opt-in becomes a default.

## Open questions (taken or open)

1. **The design** (the amplitude bound with `theta_a = 0.5`, the amplitude-controlled substitute for the arc-length driver,
   the rungs `a_b 2^(k/2)` to 18 with the caps, the fold reading and bounds, the toys PT-UF and PT-U5, the new test file
   `si_eas_hg17.rs`): taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. **The arc-length driver from a given state under prescribed-displacement loading**: not taken here (a change of the
   shared Newton iteration); proposed as a follow-up item.
3. **Whether either opt-in of ADR 0694 becomes a default**: not taken (it would change default results); open.
