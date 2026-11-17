# ADR 0697: F-779 registration: where rule L's 16^3 punch path loses stability (the load between 0.9625 and 0.965625 where the inertia changes, bracketed by bisection with the indefinite-tangent Newton; a limit point told from a bifurcation by the critical mode's projection on the load and the sign of the load parameter's derivative along the path; F-bar's path at the same loads with its inertia)

Status: **REGISTERED 2026-11-17, written before any code of F-779 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no test code of F-779 was written and nothing of this item was run (no probe). The numbers quoted
below come from the published logs of ADR 0694 (`w49i-work/f774-scratch/run3-heavy-paths.log`), ADR 0688 and ADR 0628 and
from the ADR texts. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-779 (deps F-774), the follow-up proposed by
[ADR 0694](0694-f774-registration-the-corner-element-of-rule-l-at-the-punch-corner-and-the-equilibrium-past-the-predictor.md)
(its remaining question 3). Number 0697 given by the lead. Branch `f779-work` from 5c79fb44 (F-763 / ADR 0682, F-767 / ADR
0688 and F-774 / ADR 0694 merged), worktree `w49i-work/f779`, scratch `w49i-work/f779-scratch`. The block, the element and the
material are those of ADR 0694: the incompatible-modes Hex8 at finite strain with rule L (`IncompatibleStabilisation::
LinearStress`), the neo-Hookean material of ADR 0636 (`mu = 1e6`, `nu = 0.49`), the punch block of ADR 0678 reading c1 (`[0,
1]^3` on `16^3`, the die constraints, `u_z = -0.1 lambda` on the `zmax` nodes with `x, y <= 0.5`, four threads, the supernodal
factor), as `si_eas_hg14.rs` builds it.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would normally be asked about is
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-779 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The load between 0.9625 and 0.965625 where the inertia of rule L's 16^3 neo-Hookean punch path changes, bracketed to a registered width; a limit point told from a bifurcation by a registered criterion (the load parameter's derivative and the critical mode's projection on the load); F-bar's path at the same loads with its inertia; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** the load where the inertia changes, bracketed to a registered width; **c2**
a limit point told from a bifurcation by a registered criterion built on (a) the critical mode's projection on the load and (b)
the load parameter's derivative along the path; **c3** F-bar's path at the same loads with its inertia; **c4** the default bits
unchanged.

From the brief, also registered here: bisection on the load with the indefinite-tangent solve, each solve from the converged
neighbour, with a registered width and iteration cap; the arc-length driver refuses `indefinite_tangent` (ADR 0694 D2), so this
ADR says how a path past a limit point is traced or bounded (D5); heavy runs are ignored records run once, with fast tests on
the same code path; the named reruns `si_eas_hg14`, `si_eas_hg13`, `si_eas_hg12`, `--lib`.

## What was read

- `AGENT_RULES.md`; the brief; ADR 0694 (whole, and `run3-heavy-paths.log`); ADR 0688 (the positive control PC on the ADR
  0628 patch, the location of the negative eigenvalue in element 3720); ADR 0628 (case C49, table Z: the plain element's zero
  `lambda_s = 0.9507467` on the 6^3 patch, 20 bisections); ADR 0480 (the arc-length control).
- `crates/rcfd-fem/tests/si_eas_hg14.rs` (whole: the block, `solve_observed`, `failing_attempt`, `free_tangent`,
  `symmetrise`, `inertia`, `lowest`, `lowest_any`, `sb_holds`, `block_measures`, `patch`, `boundary_nodes`, `Rng`);
  `crates/rcfd-fem/src/nonlinear.rs` (`NewtonOptions`, `solve_increment`: the committed displacement must carry `lc v` bit for
  bit, the predictor with the prescribed increment `(lt - lc) v`, then Newton, 20 iterations, residual and energy `1e-10`);
  `crates/rcfd-fem/src/constraint.rs` (`reduce`: the right-hand side `f_F - K_FP g`); `crates/rcfd-fem/src/arclength.rs`
  (`ArcLengthOptions::check`: `indefinite_tangent` refused because the controlled correction factors the indefinite tangent
  already, Envelope only); `crates/rcfd-linalg/src/sparse_chol.rs` (`SupernodalLdlt::factor_with`, `inertia`, `solve`);
  `crates/rcfd-linalg/src/eigen.rs` (`symmetric_eigen`, `DenseEigen`).

## What is known before the run (from the published logs)

1. At `u_c` (load factor `0.9625`) the driver converged; its tangent passes the positive pivot test, so `nu = 0` there (ADR
   0694 IN-A iterate 0: `nu = 0`).
2. The indefinite Newton from `u_c` to `lt = 0.965625` (`9.656250000000001e-1` as the driver attempts it) converges in 3
   iterations to `u*` with `nu* = 1`, `R*/mu = 0.32765165`, `w_c* = 0.06039238`, `|u* - u_c|_inf = 3.678e-4`, `|u* - u_1|_inf =
   3.306e-7` (`u_1` the driver's linear predictor), in 1.3 s. Its lowest pair of `(K_ff, I)` is `-3271.88` (`-0.0524 mu h`), the
   second `+2285.07`; the mode is located in element 3720 `(8, 8, 14)`, hourglass share 0.93.
3. ADR 0688: the lowest pencil value goes from `+0.063` (at `u_c`) to `-0.029` over the increment, so the lowest eigenvalue
   falls roughly linearly by about `0.11 mu h` per `0.003125` of load, `dmu_1/dlambda ~ -36 mu h` (`mu h = 62500`).
4. F-bar solves the block at `0.9625` and `0.965625` with the positive driver (ADR 0694 FB: `R/mu 0.31425746, 0.31524232`,
   `w_c 0.05849960, 0.05867666`); the gaps of rule L to F-bar at `u_c` are `g_R = 3.923e-2`, `g_w = 2.914e-2`.
5. Cost (ADR 0694 log): the observed `16^3` solve 27 s; one indefinite attempt 1.3 s; a tangent with the supernodal `NonZero`
   factor about 1 s; `lowest_any` 3 to 15 s; the F-bar `16^3` ten-increment solve 13 s.

**Arithmetic beforehand (disclosed).** With the slope of 3, the lowest eigenvalue at a bracket end of width `w` is at most
`36 mu h w`; for `w = 7.6e-7` that is `|mu_1| <= 1.7` (absolute), far above the indefinite factor's pivot refusal `1e-10 |a_ii|
~ 2.8e-4` (`a_ii ~ 2.8e6`, ADR 0694 log), so the bracket ends stay factorisable and the eigen tool's bracket search starts at
its first shift.

## Design

### D1. The bracket (c1)

**The path and its parameter.** The load parameter is the load factor `lambda` of the prescribed punch displacement, `u_P =
lambda v`. On the free equations the equilibrium is `r(u_F, lambda) = f_int,F(u_F, lambda v) = 0`; its tangent `K = dr/du_F`
(the reduced tangent, symmetrised as ADR 0694 does for the inertia) and its **load vector** `f = -dr/dlambda = -K_FP v` (the
right-hand side of `reduce` with the prescribed increment `v` and a zero force). The path's tangent at a regular point in the
`lambda` parameter is `v_t = du_F/dlambda = K^{-1} f` (differentiating `r = 0`).

**The bisection.** From the lower end `(a, u_a)` with `nu_a` and the upper end `b` with `nu_b != nu_a`: the midpoint `m = (a
+ b) / 2` is solved by `solve_increment` with `indefinite_tangent` from the converged lower end `(a, u_a)` (the converged
neighbour on the side of the starting inertia; the driver's predictor and Newton iteration, 20 iterations, residual and energy
`1e-10`, the supernodal factor, four threads); then `u_m` is **accepted on the branch** if it converged and is continuous:
`|u_m,F - u_a,F - (m - a) v_t(a)|_inf <= |(m - a) v_t(a)|_inf` (the correction from the predictor no larger than the predictor
step itself; for a regular branch the left side is `O((m - a)^2)`, the right `O(m - a)`). If accepted with `nu_m = nu_a`: `a =
m`, `u_a = u_m`; otherwise `b = m` (with `u_b = u_m` if accepted, else the upper end is marked **lost**: no equilibrium
continuous with the branch found there). The inertia `nu` is the number of negative pivots of the supernodal `L D L^T` with the
`NonZero` rule of the symmetrised reduced tangent (ADR 0694 `inertia`, Sylvester's law); a refused factor counts as a change.
The same factor solves `K v_t = f`.

**The start.** The lower end is the observed `u_c` at `0.9625` (`nu` measured); the upper end is first solved from `u_c` to `lt`
(the driver's `9.656250000000001e-1`) by the same rule (the attempt IN-A of ADR 0694 without its observer). If `nu` is equal at
both ends (or the upper end is accepted with the same inertia), the result is **"no change"** and nothing is bisected.

**The width and the cap (registered).** 12 bisections on `16^3`, so the bracket width is `(lt - 0.9625) / 2^12 = 7.63e-7`
(`3.125e-3 / 4096`); the cap is the same 12 (bisection halves the bracket every step whatever the outcome, so the width is
reached exactly at the cap). Estimate of the cost: 13 solves of about 1.3 to 3 s, each with one tangent and factor (1 s) for
`nu` and `v_t`. Why `2^-12`: with the slope of the arithmetic above the lowest eigenvalue at the ends is `<= 1.7` (factorisable,
see the arithmetic) and the bifurcation bound of D2 (`cos <= |mu_1| |v_t| / |f|`) is `<= 3e-5`, two orders below the threshold
of D2; a finer bracket is not needed for the reading.

### D2. A limit point or a bifurcation (c2)

At the bracket ends `a` and `b` (each where it exists): the lowest pair `(mu_1, phi)` of `(K, I)` (`lowest` when `nu = 0`,
`lowest_any` otherwise; 500 iterations, tolerance `1e-8`, as ADR 0694) and the load vector `f`.

**(a) The projection.** `cos = |phi . f| / (|phi| |f|)`. **Derivation.** Differentiating `r(u_F(s), lambda(s)) = 0` along
the path in its arc length `s`: `K u_F' = f lambda'`; at the critical point `K phi = 0`, and `K` is symmetric, so `0 = phi . K
u_F' = (phi . f) lambda'`. At a **limit point** `phi . f != 0`, so `lambda' = 0`: the path turns back in the load. At a
**(simple) bifurcation** `phi . f = 0` and the path passes with `lambda' != 0` (Keller's and Riks' classification of the
singular points of a one-parameter path, recalled from memory and used only through this one-line derivation). Away from the
critical point the identity `phi . f = phi . K v_t = mu_1 phi . v_t` holds exactly for the symmetric `K`: on a branch that
passes (bifurcation) `v_t` stays bounded and `cos <= |mu_1| |v_t| / |f|` tends to zero with `mu_1`; at a limit point `cos` stays
finite while `|v_t|` grows like `1/|mu_1|`. **The threshold `theta = 1e-3`**: between the bifurcation's bound at the registered
width (`<= 3e-5`, D1, reported as `b_bif = |mu_1| |v_t| / |f|` at each end) and the cosine of a vector unrelated to `f` on
`n_F ~ 12000` free equations (`1/sqrt(n_F) ~ 9e-3`). `cos > theta` reads "limit point", `cos < theta` "bifurcation", at each end.

**(b) The sign of the load parameter's derivative.** In the arc-length orientation that is continuous along the path, the
tangent is `t = (v_t, c) / |(v_t, c)|` with `lambda' = c / |(v_t, c)| > 0` at the lower end; `c = |v_t(u_c)|` (the tangent's
size at the start of the interval, so that `lambda` is measured in the units of `u`, registered). The path continued from `a` to
`b` keeps its orientation, so `sign(lambda'(b)) = sign(t_a . t_b)`: **`DS = t_a . t_b`**. At a limit point the `phi` part of
`v_t` dominates on both sides with opposite signs (`v_t ~ phi (phi . f) / mu_1`, `mu_1` changes sign), so `DS < 0`: `lambda'`
changes sign. At a bifurcation `v_t` is smooth through the critical point and `DS ~ +1`: `lambda'` keeps its sign. If the upper
end is **lost** (no continuous equilibrium past `b` in the `lambda` parameter), that is itself the mark of a limit point under a
`lambda`-controlled path (`lambda'` cannot stay positive): read as `DS < 0` ("lambda' changes sign: no equilibrium past").

**Reading LB**: **"limit point"** if `cos > theta` at every end where it is measured and (`DS < 0` or the upper end lost);
**"bifurcation"** if the upper end exists, `cos < theta` at both ends and `DS > 0`; **"undecided"** otherwise. Also reported (no
limits): `|v_t| / c` at every accepted point of the bisection; `b_bif` at each end; the symmetry of `phi` under the reflection
`P: (x, y, z) -> (y, x, z)` of the block, `sigma = phi . P phi / |phi|^2` (`+1` symmetric, `-1` antisymmetric), and the
asymmetries `|u - P u|_inf / |u|_inf`, `|f - P f| / |f|` (the block, its die constraints and its punch `x, y <= 0.5` are
symmetric under `P`, and `hex_box` places its nodes at `i/n` in both directions, so an antisymmetric `phi` has `phi . f = 0`
exactly when `f` is symmetric: a symmetry-breaking bifurcation). **MS**: at the ends `mu_1(a) > 0 > mu_1(b)` (the sign agrees
with `nu`), each pair with residual `<= 1e-8`.

### D3. F-bar's path at the same loads (c3)

The F-bar block (`SolidFormulation::BBar`, rule `Off`, ADR 0678): the observed ten-increment solve (`solve_observed`, the
default driver, punch `-0.1`) gives the converged state at its committed load factor before the last increment (0.9); from it a
chain of `solve_increment` with `indefinite_tangent` (so that a loss of stability of F-bar is also measured, not refused) to
`0.9625`, `a`, `b` and `lt`, each from the previous converged state. At each: `nu_Fb`, `R/mu`, `w_c`; at `b` the four lowest
pairs of `(K, I)` (`lowest_any`). **FB0**: the F-bar state at `0.9625` against ADR 0694's FB (`0.31425746`, `0.05849960`): `<=
1e-6` relative (the same equilibrium within the residual tolerance; a different path to it). **Gaps**: `g = (R_L / R_Fb - 1, w_L
/ w_Fb - 1)` at `a` and `b`, `|g| <= 0.15` (ADR 0694 EQ) and continuous with the gaps at `u_c` (`|g(a) - g(u_c)|, |g(b) - g(u_c)|
<= 0.01`, ADR 0694 EQ's continuity). **Reading FS**: "F-bar stable at the bracket" if `nu_Fb = 0` at the four loads, else "F-bar
also loses stability" (with the loads).

### D4. The fast controls on the same code (`f779_fast`)

The bracket (D1) and the readings (D2) are written once, generic over a **case** (the solve from a converged neighbour, the
symmetrised reduced tangent and the load vector at a state, the lowest pair); the heavy record and the fast controls call the
same functions.

- **LPT, a closed-form limit point.** Eight unknowns: `r_0 = u_0 - u_0^3 / 3 - p`, `r_i = k_i u_i - p b_i` (`k_i = 1 + i`, `b_i =
  0.1 i`, `i = 1..7`); `K = diag(1 - u_0^2, k_i)`, `f = (1, b_i)`. The branch from `u_0 = 0` has a fold at `u_0 = 1`, `p_c = 2/3`
  (`u_0 - u_0^3/3` has its maximum `2/3` at 1); past it no root near the branch. The case's solve is the same predictor and
  Newton (`u += K^{-1}(-r)`, 20 iterations, `|r|_inf <= 1e-14`, a non-finite or non-converged iteration is a failure). Bracket on
  `[0.6, 0.7]` with 20 bisections (width `0.1 / 2^20 = 9.54e-8`): contains `2/3`; the upper end lost; `cos(a)` against the closed
  form `|f_0| / |f| = 1 / sqrt(2.4)` (`phi = e_0`, `1 - u_0^2` the smallest) `<= 1e-10` relative; reading "limit point". The
  lowest pair by `symmetric_eigen` (dense; the matrix is 8 by 8).
- **BFT, a closed-form bifurcation.** The same with `r_0 = (1 - p) u_0 + u_0^3` (a pitchfork: `u_0 = 0` for every `p`, `K_00 = 1
  - p`, `f_0 = 0`, `p_c = 1`). Bracket on `[0.9, 1.2]` (1 is not a dyadic point of it) with 20 bisections (width `2.86e-7`):
  contains 1; `nu_b = 1`; `cos <= 1e-12` at both ends; `DS >= 1 - 1e-12`; reading "bifurcation".
- **DSF, the sign function.** `DS` of synthetic tangents: `v_a = 1e3 e_0 + w`, `v_b = -1e3 e_0 + w` (`w = (0, 1, 1, ...)`, `c =
  1`): `DS < 0`; `v_a = v_b`: `DS = 1` to `1e-15`.
- **PB, a bifurcation in the finite elements (positive control on the same FEM case code).** The ADR 0628 patch (`6^3`, interior
  nodes moved, `nu = 0.49`), the plain element (`Off`), confined compression `F = diag(1, 1, 1 - p)`: every boundary node
  prescribed by the list `u = p v`, `v = (0, 0, -Z)` (`Prescribed`, scaled by the load factor in `solve_increment`). The
  homogeneous field `u = p v` is an equilibrium for every `p` (the patch test of the incompatible element: ADR 0628 H3b), so its
  path passes every critical point: a bifurcation, `phi . f = 0` exactly at the critical point (D2). ADR 0628 table Z: the plain
  element's first zero `lambda_s = 0.9507467`, i.e. `p_c = 0.0492533` (quoted to 7 digits, `+- 5e-8`; its 20 bisections `1e-8`).
  The lower end `p = 0.04` is the homogeneous field `0.04 v` (bit for bit `lc v` at the boundary, initial states; an equilibrium
  to rounding), `nu = 0` (`m_Inc > 0` above `0.9507`); the upper end `0.05` (`m_Inc(0.95) = -0.0547`, ADR 0628). 16 bisections
  (width `0.01 / 2^16 = 1.53e-7`). Checks: `a - 6e-8 <= p_c <= b + 6e-8`; `nu_a = 0`, `nu_b >= 1`; `cos < theta` at both ends;
  `DS >= 0.99`; **PV**: `v_t(a)` against the exact path tangent `v` on the interior, `|v_t - v|_2 / |v|_2 <= 1e-6`; MS; reading
  "bifurcation".
- **NC8, a stable path (negative control).** The `8^3` block, rule L: the observed ten-increment solve, its last attempt from
  `0.9`: the bracket's start from `u_c(0.9)` to `1.0` with the block case: reading "no change" (`nu = 0` at both ends); `R`, `w_c`
  at `1.0` against ADR 0678 (`0.36659596`, `0.06631664`) `<= 1e-6` relative; `cos` and `|v_t|` at `1.0` finite.

### D5. Past a limit point (the arc-length refusal)

The arc-length driver (`arclength::trace`, ADR 0480) refuses the opt-in `indefinite_tangent`, but only because its controlled
correction factors the indefinite tangent already (the envelope `L D L^T` with its inertia, ADR 0542 D2): it traces a path
through a limit point by construction. So: if the reading is **"bifurcation"**, the `lambda`-controlled path of D1 passes the
critical point (ADR 0694 IN-F did, to full load) and no arc length is needed for the branch of `u_c`; the bifurcated branch is
not traced (a branch switch onto `phi` is not in scope). If the reading is **"limit point"**, the `lambda`-controlled path cannot
pass it: the bracket bounds `lambda_c` within its width, the lost upper end is reported, and the path past it is the arc-length
driver's (the envelope factor, about ten times the supernodal time on `16^3`, ADR 0542, from rest: estimated 10 to 20 minutes),
proposed as a follow-up and not run here.

### D6. The test file `crates/rcfd-fem/tests/si_eas_hg15.rs`

Helpers copied from `si_eas_hg14.rs` (existing tests are not edited). Test-only: no library change is planned, so the default
bits are unchanged by construction (B).

## Checks

Every measure is required finite before it is compared (a non-finite value or a missing step misses).

### The fast test `f779_fast` (default, not ignored)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| LPT | c1, c2 | the fold: `a <= 2/3 <= b`; `b - a`; the upper end; `cos(a)` against `1/sqrt(2.4)`; reading | holds; `<= 9.6e-8`; lost; `<= 1e-10` relative; "limit point" | closed form |
| BFT | c1, c2 | the pitchfork: `a <= 1 <= b`; `b - a`; `nu_b`; `cos(a)`, `cos(b)`; `DS`; reading | holds; `<= 2.9e-7`; `= 1`; `<= 1e-12` each; `>= 1 - 1e-12`; "bifurcation" | closed form |
| DSF | c2 | `DS` of the flipped and of the equal synthetic tangents | `< 0`; `|DS - 1| <= 1e-15` | exact |
| PB | c1, c2 | the patch: `p_c` in the bracket (`+- 6e-8`); `b - a`; `nu_a`, `nu_b`; `cos` at both ends; `DS`; PV; MS; reading | holds; `<= 1.6e-7`; `= 0`, `>= 1`; `< 1e-3` each; `>= 0.99`; `<= 1e-6`; holds; "bifurcation" | ADR 0628 Z |
| NC8 | c1 | the `8^3` block from 0.9 to 1.0: reading; `R`, `w_c` against ADR 0678; `cos(1.0)`, `|v_t(1.0)|` | "no change"; `<= 1e-6` each; finite | stable path |

### The heavy record `f779_nh16_bracket_heavy` (ignored, run once)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| OB | c1 | the observed `16^3` solve: the error text | equal to ADR 0678's | ADR 0694 OB |
| H0 | c1 | `nu(u_c)`; the upper end from `u_c` to `lt`: accepted, `nu`, `R`, `w_c` against ADR 0694's `u*` | `= 0`; accepted, `= 1`, `<= 1e-7` relative each | ADR 0694 IN-A |
| BR | c1 | the bracket `[a, b]`: `0.9625 <= a < b <= lt`; `b - a`; the bisections made; every midpoint solve's outcome | holds; `<= 7.63e-7 (1 + 1e-9)`; `= 12`; reported | D1 |
| MS | c2 | `mu_1(a) > 0 > mu_1(b)`; the residuals of the pairs | holds; `<= 1e-8` | ADR 0694 EI* |
| LB | c2 | `cos(a)`, `cos(b)`; `DS`; `b_bif`; `|v_t| / c` along; `sigma`, the asymmetries | reading LB (reported; `theta = 1e-3`) | D2 |
| FB0 | c3 | F-bar at 0.9625 against ADR 0694 FB | `<= 1e-6` relative each | the same equilibrium |
| FBP | c3 | F-bar at 0.9625, `a`, `b`, `lt`: converges; `nu_Fb`; the four lowest at `b`, largest residual | each converges; reported (reading FS); `<= 1e-8` | ADR 0678 |
| GP | c3 | rule L against F-bar at `a`, `b`: `|g|`; the change from `g(u_c)` | `<= 0.15` each; `<= 0.01` each | ADR 0694 EQ |

### The default bits and the static checks

| Id | Clause | Measure | Limit |
|---|---|---|---|
| B | c4 | no file under `crates/*/src` differs from 5c79fb44 at the tip (`git diff --stat 5c79fb44 -- crates/*/src` empty) | empty |
| S7 | | `cargo test -p rcfd-fem --release --lib`; `--test si_eas_hg14` (its default tests), `si_eas_hg13`, `si_eas_hg12` | pass |
| S6 | | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean |
| S8 | | `cargo test -p rcfd-cli --release --test docs_site` (docs/ touched by this ADR) | pass |
| S9 | | every commit of the branch builds (`cargo build -p rcfd-fem --release --tests`) | builds |

`baseline_hashes` is not run: no shared solver file is touched (B checks it).

### Negative and positive controls

| Id | Control | Expected |
|---|---|---|
| LPT | a closed-form fold | "limit point", the upper end lost, `cos` the closed form |
| BFT | a closed-form pitchfork | "bifurcation", `cos = 0`, `DS = 1` |
| PB | a finite-element bifurcation with a reference load (ADR 0628) | the bracket contains it; "bifurcation" |
| NC8 | a stable finite-element path | "no change" |
| DSF | flipped and equal tangents | `DS < 0`; `DS = 1` |

### Planned mutations (on a scratch copy of the test commit, each run once against `f779_fast`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the bisection keeps the wrong half (`a = m` when the inertia changes, `b = m` when it does not) | LPT, BFT, PB (the bracket misses `p_c`) |
| MU2 | the load vector of the FEM case taken with a zero prescribed increment (`f = -r_F`, the residual) | PB (`cos` not finite or PV), NC8 |
| MU3 | `DS` without the `phi` orientation (its absolute value) | DSF |
| MU4 | the continuity test dropped (every converged midpoint accepted) | LPT (the upper end not lost) |
| MU5 | the threshold comparison reversed (`cos > theta` read "bifurcation") | LPT, BFT, PB |
| MU6 | the bisection cap 8 instead of the registered one | LPT, BFT, PB (the width) |

The heavy record is not run under the mutations (it uses the same functions, checked by the fast test).

## Decision rule

- **done**: every check of `f779_fast` met; OB, H0, BR, MS, FB0, FBP, GP met in their single registered run; reading LB
  "limit point" or "bifurcation" (c2 answered); F-bar's inertia measured at the four loads (c3); B, S6, S7, S8, S9 met; every
  mutation caught; no amendment.
- **partial**: B met, but a check missed, LB "undecided", a mutation not caught, or a check met only after an amendment.
- **todo**: B fails (a default bit could change), or OB fails and no bracket is made.

## Limits (not covered)

- One mesh, one material, one load path; the bracket finds one change of the inertia in the interval (bisection finds a change
  between ends of different inertia, not every change; `nu` is measured at the 14 states only).
- The bifurcated branch is not traced (no branch switch); a limit point would be bounded, not passed (D5).
- The threshold `theta` separates the two cases at the registered width; a limit point whose mode is nearly orthogonal to the
  load (`cos < 1e-3`) would read as a bifurcation by (a) and is then caught only by `DS` (the reading is "undecided" if they
  disagree).
- Rule L's coefficient is not in scope (the maintainer's call, ADR 0682 open question 2); no opt-in becomes a default.

## Open questions (taken or open)

1. **The design** (the bisection from the stable neighbour with the continuity test, 12 bisections, `theta = 1e-3`, `c =
   |v_t(u_c)|`, the toys, the patch control, the F-bar chain with the indefinite tangent, the new test file `si_eas_hg15.rs`):
   taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. **Whether either opt-in of ADR 0694 becomes a default**: not taken (it would change default results); open, as in ADR 0694.
