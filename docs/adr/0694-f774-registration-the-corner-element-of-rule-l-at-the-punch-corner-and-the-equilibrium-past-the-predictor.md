# ADR 0694: F-774 registration: the corner element of rule L at the punch corner and the equilibrium past the predictor (det F and the local mode solution of element 3959 and its neighbours, cold, warm and continued, at the converged state 0.9625 and along the lift and the predictor line of the failing attempt; an opt-in Newton that accepts an indefinite tangent, the equilibrium at 0.965625 with its inertia against F-bar at the same load; an opt-in backtracking of the predictor on an inadmissible trial, on the attempt and on the whole 16^3 solve)

Status: **REGISTERED 2026-11-17, written before any code of F-774 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-774 was written and nothing of this item was run (no probe). The
numbers quoted below come from the published logs of ADR 0688 (`w49i-work/f767-scratch/probe1.log`, `run1-fast.log`,
`run2-heavy-iterates.log`, `run3-heavy-remedies.log`), ADR 0682 (`w49i-work/f763-scratch/run4-heavy-variants.log`) and ADR 0678
(`w49i-work/f747-scratch/run3-heavy.log`) and from the ADR texts. Results go in the addenda at the end; nothing above the addenda
is changed after a run.

Roadmap item F-774 (deps F-767), the follow-up proposed by
[ADR 0688](0688-f767-registration-the-failing-newton-iterate-of-rule-l-at-the-punch-corner.md) (its remaining questions 1, 2,
4 and 5, each option (a) recommended there). Number 0694 given by the lead. Branch `f774-work` from 2c0ea11c (F-763 / ADR 0682
and F-767 / ADR 0688 merged), worktree `w49i-work/f774`, scratch `w49i-work/f774-scratch`. The block, the element and the
material are those of ADR 0688: the incompatible-modes Hex8 at finite strain of ADR 0608 with rule L
(`IncompatibleStabilisation::LinearStress`, ADR 0630 D1), the neo-Hookean material of ADR 0636 (`mu = 1e6`, `nu = 0.49`), the
punch block of ADR 0678 reading c1 (`[0, 1]^3` on `16^3`, the die constraints, `u_z = -0.1` on the `zmax` nodes with `x, y <=
0.5`, ten increments, four threads, the supernodal factor), as `si_eas_hg13.rs` builds it.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the
open questions at the end.

## What F-774 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. On the punch block of ADR 0688 (rule L, neo-Hookean, 16^3) at the converged state 0.9625 and along the predictor line of the failing attempt: det F at the integration points of element 3959 and its neighbours and the local mode solution there (cold and warm, its root), and whether the inversions found by the probe of ADR 0688 are a jump of that solution; the equilibrium at 0.965625 by a path that accepts an indefinite tangent, with its inertia, against F-bar at the same load; a backtracking of the predictor from the full step on an inadmissible trial, opt-in, and whether the block then solves to full load; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** `det F` at the integration points of element 3959 and its neighbours at
`u_c` (0.9625) and along the predictor line; **c2** the local mode solution there, cold and warm, and its root; **c3** whether
the inversions of the probe of ADR 0688 are a jump of that solution; **c4** the equilibrium at `0.965625` by a path that accepts
an indefinite tangent, with its inertia, against F-bar at the same load; **c5** an opt-in backtracking of the predictor from the
full step on an inadmissible trial; **c6** whether the block then solves to full load; **c7** the default bits unchanged.

From the brief, also registered here: the surviving mutations MU1 and MU2 of ADR 0688 get stronger controls (D7); the eigen tool
runs with at least 500 iterations in both branches (ADR 0688 remaining question 4); rule L's coefficient is not in scope.

## What was read

- `AGENT_RULES.md`; the brief; ADR 0688 (whole; its logs, in particular `probe1.log` and its source `zz_probe_f767.rs`); ADR
  0682 (the variants S and W in `run4-heavy-variants.log`: backtracking with decrease, and the warm start, fail with the same
  pivot at node 4768; 20 and 40 increments fail earlier, at `0.870`, with element 3959 inverted at point 4); ADR 0655 (the
  backtracking of the local mode step), ADR 0658 (the decrease test), ADR 0660 (D1 the warm start from the committed modes, D2 the
  trust region); ADR 0607 (the line search); ADR 0480 (the arc-length control, its indefinite envelope factor).
- `crates/rcfd-fem/tests/si_eas_hg13.rs` (whole: the observer, `replicate_step`, `lowest_any`, `share_l`, the patch);
  `crates/rcfd-fem/src/nonlinear.rs` (`NewtonOptions`, `check`, `Problem::attempt_with`, `trial_slope`, `predictor_length`,
  `search_from`, `factor_solve`, `controlled_correction`, `solve_with`, `solve_increment`, `solve_from`, `Failure`, `classify`);
  `arclength.rs` (`ArcLengthOptions::check`); `nl_incompatible.rs` (`extended`: the inversion test on the enhanced `F = I + H_c +
  F_m` at each point, `converged`: the local Newton iteration from `alpha = 0` or from a start, `evaluate` and the warm start
  of ADR 0660 D1, `mode_iteration_record`, `mode_iteration_record_from`, `point_gradients`); `mat_hyper.rs` (the neo-Hookean
  energy `W = (mu/2)(Ibar1 - 3) + (kappa/2)(J - 1)^2`); `crates/rcfd-linalg/src/sparse_chol.rs` (`SupernodalLdlt::factor_with`
  with `PivotRule::NonZero`, `factor_analysed` with positive pivots only), `skyline.rs` (`SkylineOptions`, `PivotRule`),
  `eigen.rs` (`Shift::Value`, `GeneralizedEigen::shift`).

## What is known before the run (from the published logs)

1. **The probe of ADR 0688** (`probe1.log`, after its registered runs): along the predictor line `u_L + beta du` of the failing
   attempt (`u_L` the lifted iterate: the free values of `u_c`, the prescribed values at `0.965625`), the whole-model evaluation
   (the default cold local update) fails at `beta = 0` (`element 3959: det F = -5.964573875984064e-2 at point 4`), `0.0625`
   (`det F = -7.824161087854067e-1 at point 0`) and `0.125` (`det F = -1.8630342959045024e1 at point 0`), and succeeds at
   `0.03125`, `0.25`, `0.5` (slopes `-24.2, -19.0, -12.9`, the driver's factor passes, `nu = 0`) and `1` (slope `+4.0e-3`, the
   factor refused, `nu = 1`). A `det F` of `-18.6` from a prescribed lift of `3.1e-4` (`0.5` per cent of `h`) is not a
   compatible field: the compatible gradient changes by `O(3.1e-4 / h) = 5e-3`, so it can only come from the modes. The
   inversion test is made on the enhanced gradient at every local iterate (`extended`), so a local Newton iterate that
   overshoots is refused even if a root with `det F > 0` exists. These are the two candidates c3 tells apart.
2. **At `u_c`** the driver converged (the cold local iteration converges in every element), and the predictor's iterate `u_1`
   is evaluated by the driver (every element converges cold), its tangent has one negative eigenvalue in element 3720 (ADR 0688).
3. **The warm start of ADR 0660 and the backtracking of ADR 0655/0658 on the whole solve** (ADR 0682 S, W) fail with the same
   pivot at node 4768: neither changes the predictor's iterate (the driver's predictor is a linear solve at `u_c`), so they do not
   tell whether the lifted iterate has an admissible root.
4. **Cost** (ADR 0688): the observed `16^3` solve `74 s`; a force and tangent `~0.3 s`, a factor `~0.7 s`; four pairs of the
   pencil `~30 s`; the F-bar `16^3` solve `30 s` (ADR 0678). The local mode solve of one element is microseconds.

## Design

### D1. The corner record (test-only; c1, c2, c3)

**The path.** From the observed failing attempt (`lc = 0.9625`, `lt = 0.965625`, its iterates `u_0 = u_c`, `u_1`, ADR 0688
D1) and the driver's predictor `du` (`replicate_step` at `k = 0`, bit for bit the driver's): the **lift** `L(t)`, `t = i / 16`,
`i = 0..16`: the free values of `u_c`, the prescribed values `(lc + t (lt - lc)) v` (at `i = 0` exactly `lc v`, the committed
values; at `i = 16` exactly `lt v`), so `L(0) = u_c` and `L(1) = u_L`; then the **predictor line** `P(beta) = u_L + beta du` on
the free equations, `beta = j / 64`, `j = 0..64` (at `j = 64` the update `u_i + du_i` of the driver, bit for bit `u_1`). 81
points in this order; the probe's points `0, 1/32, 1/16, 1/8, 1/4, 1/2, 1` are on the grid.

**The elements.** `N3` = element 3959 (`(7, 7, 15)`, the top-layer element of the punch with the corner node 4768) and its
neighbours: the 18 elements `(i, j, k)` with `i, j in {6, 7, 8}`, `k in {14, 15}` (3959 and the 17 elements sharing a node
with it inside the block; 3720 = `(8, 8, 14)` is among them).

**At every point and element** (the committed state of the element, rule L, the default `Plain` local update):

- `det F_c,q`: the compatible gradient's determinant at the 8 points of the stiffness rule (`point_gradients` with zero modes).
- **cold**: the local iteration from `alpha = 0` (`mode_iteration_record`, the driver's default): classified **C** (converged,
  its root `a_cold`), **I0** (refused at the start: the compatible field itself inverted at a point), **Ij** (a local iterate
  `j >= 1` inverted), **N** (not converged in 25 iterations), **S** (a singular mode matrix); with the error text.
- **warm**: the local iteration from the root `a_c` of the element at `u_c` (`mode_iteration_record_from`; the start rule of
  ADR 0660 D1 without its cold fall-back), classified the same, its root `a_warm`.
- **continued**: the local iteration from the continued root of the previous point (at `L(0)` from `a_c`), its root `a_cont`;
  if it fails at a point, the next point starts from the last continued root found (reported). This is a numerical continuation
  of the root along the path: with a regular `K_aa` the root is a smooth function of the element displacement (implicit
  function theorem) and a start at the neighbouring point's root is inside Newton's quadratic basin for a fine enough grid.
- `det F_q` at the root found (cold, warm, continued) at the 8 points (`point_gradients` with the root), their minimum.

**Measures.** For each element: the points where the cold iteration fails, and how; whether the continued root exists at every
point; `min det F_q` of the continued root along the path; **the continuity ratio** `max |a_cont(p_{m+1}) - a_cont(p_m)|_2 /
|u_e(p_{m+1}) - u_e(p_m)|_2` over consecutive points with `u_e` changed (the measure of ADR 0688 D8, which was `0.049` over the
whole predictor of the corner elements); **root switches**: the points where the cold and the continued roots both exist and
`|a_cold - a_cont|_2 > 1e-6 max(|a_cont|_2, 1e-12)` (a local Newton iteration converging to another root). For element 3959: all
8 points' `det F_c`, the cold, warm and continued `det F` at `u_c`, `u_L`, `u_1` and the probe's points. For the 18 elements:
the summary per element.

**Reading JR (c3)** at each of the probe's inverted points `beta in {0, 1/16, 1/8}`, for element 3959:
- **"a jump of the local solution"** (the local iteration, not the root): the cold iteration of 3959 fails there, the continued
  root exists there, and the continuation is continuous (3959's continuity ratio `<= 10`, the JM limit of ADR 0688, and the
  continued root exists at every point of the path);
- **"an inversion of the root"**: no continued root of 3959 at that point (the continuation and the warm iteration both fail);
- **"between"** otherwise.
JR overall: "a jump" if all three points read "a jump", "an inversion" if all three read "an inversion", else "between".

**The whole model along the path (AS, WL).** At every point the whole-model evaluation `internal_force` (the default cold
update): success, or the error text (the probe's texts at the probe's points). And on the predictor line at `beta = j / 16`,
`j = 0..16`, the **warm model** (the same model with `set_incompatible_mode_warm_start(true)`, ADR 0660 D1, the states carrying
the cold roots `a_c` of every element at `u_c`: each evaluation starts from `a_c` and falls back to the cold start): the
evaluation's success, the slope `s(beta) = -du . r_F` of ADR 0688 D3, the inertia `nu(beta)` of the reduced symmetrised tangent
and whether the driver's factor passes. This tells whether the predictor line is admissible with the warm start (a measured
input to a follow-up; no limit).

### D2. A Newton iteration that accepts an indefinite tangent (library, opt-in `NewtonOptions::indefinite_tangent`; c4)

**Derivation.** Newton's method for `r(u) = 0` needs a regular tangent, not a definite one: with `K` Lipschitz and `K(u*)`
regular, `|u_{k+1} - u*| <= C |u_k - u*|^2` near `u*` (Taylor's theorem; Dennis and Schnabel 1983, cited from memory and not
used beyond this). The driver's refusal of a non-positive pivot (`d <= 1e-10 a_ii`) is a choice for the stable path, not a need
of the iteration. A symmetric `L D L^T` without pivoting exists when no pivot vanishes, and by Sylvester's law of inertia the
number of negative pivots is the number of negative eigenvalues (the inertia, as ADR 0480 and ADR 0688 use it). So the opt-in
factors the reduced tangent of every attempt without a load-factor control by the same `L D L^T` with the **`NonZero`** pivot
rule and refuses a pivot only when `|d| <= 1e-10 |a_ii|` or `d` is not finite: the test of the arc-length correction (ADR 0480,
`controlled_correction`), with the driver's failure texts. The supernodal factor is then made by
`SupernodalLdlt::factor_with` (nested dissection, `NonZero`, symmetry tolerance `1e-12`) without the analysis cache (the cache's
`factor_analysed` has positive pivots only; no library change of `rcfd-linalg`); the envelope factor by
`SkylineLdlt::factor_with` (`Rcm`, `NonZero`). The arc-length path of ADR 0480 is the other path that accepts an indefinite
tangent; it is not taken because it factors by the envelope only (ADR 0542 D2: about ten times the supernodal factor's time on
`16^3`) and because the load of the block is a prescribed displacement, held by the arc-length control at the load factor, which
there changes (the equilibrium at a given `0.965625` is the question).

**Scope.** `pub indefinite_tangent: bool`, **default `false`**, builder `indefinite_tangent(bool)`. Active in an attempt
without a load-factor control with the symmetric factor (every factor call of the attempt, including the fall-back of ADR 0570
D2); not in the trial tangent of the damping or the backtracking (their admissibility keeps the driver's positive test). Refused
(key `"indefinite_tangent"`): with `unsymmetric` (`NewtonOptions::check`), in `ArcLengthOptions::check` (the controlled
correction is indefinite already). With it off the code path is the default one, unchanged.

**The equilibrium at `0.965625` (c4).** `solve_increment` (ADR 0688 D2) from the observed `u_c` to `0.965625` with
`indefinite_tangent` (IN-A), the driver's other options (20 iterations, residual and energy `1e-10`), with the observer of ADR
0688 D1 (allowed: the observer adds nothing) to record every iterate: at each the inertia `nu_k`. If it converges, at `u*`: the
inertia `nu*`; the four lowest pairs of `(K_ff, I)` and of the pencil `(K_ff, L_ff)` by `lowest_any` (D7, 500 iterations), the
lowest located by element (ADR 0688); `|u* - u_1|_inf`, `|u* - u_c|_inf`; `R* = -sum_punch f_int,z / mu` and `w_c*` (the `z`
displacement of the far corner node, as ADR 0678). **Against F-bar at the same load (FB)**: the F-bar block (`SolidFormulation::
BBar`, the mean-dilatation element of ADR 0678) with the punch `-0.1 x 0.965625` and with `-0.1 x 0.9625`, ten increments each:
`R_Fb`, `w_Fb`; the gaps `g_R = R_L / R_Fb - 1`, `g_w = w_L / w_Fb - 1` at `u_c` (0.9625) and at `u*` (0.965625).

**Reading EQ (c4)**: **"a stable equilibrium past the predictor"** if IN-A converges with `nu* = 0` and the gaps are continuous
(`|g(u*) - g(u_c)| <= 0.01` for `R` and `w_c`: a load step of `0.3` per cent changes a smooth gap by `O(0.003)`); **"an
unstable equilibrium on the branch of u_c"** if it converges with `nu* >= 1` and the gaps are continuous; **"another stationary
point"** if it converges and a gap is not continuous; **"not reached"** if IN-A fails (the text).

### D3. The backtracking of the predictor (library, opt-in `NewtonOptions::predictor_backtracking`; c5, c6)

**Rule.** At iteration 0 of an attempt with a non-zero prescribed increment, without a load-factor control, with the symmetric
factor: the trial iterates `x(beta) = u_c + beta du` on the free equations with the prescribed values at `lt v`, `beta = 1, 1/2,
1/4, ..., 2^-10`, in this order; the first **admissible** one is taken (admissible as in ADR 0688 D3: the evaluation succeeds,
the slope is finite, and the reduced tangent at the trial passes the driver's positive pivot test). If none is admissible,
`beta = 1` (the default's full step). Then `u_F += beta du` (`beta = 1` by `u_i += du_i`, exactly as without the option), the
prescribed values `lt v`; the energy of iteration 0 stays that of the full correction. **The cap of 10 halvings** is not tuned:
the probe found the first admissible point at `1/2`; ten halvings take the trial within `1e-3` of the predictor's length of the
lifted iterate (between ADR 0607's 8 trials and ADR 0655's 20 halvings). **Why from the full step and not from the lifted
iterate**: the damping of ADR 0688 D3 needs the slope at the lifted iterate, which the probe found inadmissible (element 3959),
so it fell back to the full step; the backtracking never evaluates the lifted iterate (`beta = 0` is not a trial).

**Scope.** `pub predictor_backtracking: bool`, **default `false`**, builder `predictor_backtracking(bool)`. Refused (key
`"predictor_backtracking"`): with `unsymmetric`, with `predictor_damping` (both act on iteration 0), in `ArcLengthOptions::check`,
and in an attempt with a contribution (the trial tangent would need the contributions' tangents, as ADR 0688 D3). Records: the
`step_lengths` of a converged attempt get `beta` at iteration 0 (and `1.0`, or the line search's `alpha`, after), as with the
damping. **A failed attempt** whose predictor was backtracked (`beta < 1`) carries `"; the predictor backtracked to beta =
{beta:e} (ADR 0694 D3)"` at the end of its cause (the reason of a located inversion, or the solver's text), so that the step
length of a failing attempt is seen; with `beta = 1` or without the option the failure is unchanged. The attempt function is
split into the iteration and a wrapper that adds the note; the default path's arithmetic is the same operations in the same
order. **Combination with `indefinite_tangent`**: allowed (the backtracking keeps the positive test for its trial; the Newton
iterations after it accept an indefinite tangent).

**The runs (c5, c6).** On the failing attempt (`solve_increment` from the observed `u_c`): BT-A (backtracking), IB-A
(backtracking and the indefinite tangent). On the whole `16^3` solve (`solve_with`, ten increments, the options of ADR 0682,
the recorder of ADR 0688): BT-F (backtracking), IN-F (the indefinite tangent), IB-F (both); each "solves to full load" (`R`,
`w_c`, the inertia `nu` of the tangent at full load) or "fails" (the text, the last converged load factor).

### D4. The fast controls

- **The column (BTC, DC).** A derived case where the full predictor inverts and half of it does not: two Full Hex8 on `[0, 1]^2
  x [0, 2]` (`1 x 1 x 2`), the neo-Hookean material (`nu = 0.49`) with `mu = 1e6` in the lower element and `99e6` in the upper,
  every node held in `x` and `y` (uniaxial strain), the `zmin` nodes held in `z`, the `zmax` nodes at `u_z = -1.2`, one attempt
  from rest to the full value (`solve_increment`, `lc = 0`, `lt = 1`). **Arithmetic** (by hand, disclosed): at rest the reduced
  tangent is that of two springs in series of stiffness proportional to the uniaxial-strain moduli `kappa + 4 mu / 3`, which
  scale with `mu` at equal `nu`; by symmetry the four middle nodes move alike; the predictor moves them by `r Delta` with `r =
  99/100`: `-1.188`, so at `beta = 1` the lower element has the stretch `1 - 1.188 = -0.188` (inverted, inadmissible, certain);
  at `beta = 1/2` the stretches are `1 - 0.594 = 0.406` (lower) and `1 - 1.2 + 0.594 = 0.394` (upper), both positive; the
  reduced tangent (the `z` values of the middle nodes only) is then positive definite because the gradient of a `z`-only field is
  rank-one and the energy is polyconvex (the isochoric neo-Hookean term and the convex `(kappa/2)(J - 1)^2`; Ball 1977, cited
  from memory), so `beta = 1/2` is admissible. At the lifted iterate (`beta = 0`) the upper element has the stretch `1 - 1.2 =
  -0.2` (inverted): the damping of ADR 0688 falls back to the full step, the miniature of the 16^3 case. **BTC**: the test's own
  replica of the backtracked attempt (the trials by `internal_force`, `tangent`, `reduce` and the driver's factor test; then the
  driver's Newton iterations by `replicate_step` with the driver's criteria) against `solve_increment` with
  `predictor_backtracking`: `beta` of the replica equal to `1/2`; on convergence the library's `u` bit-equal to the replica's and
  `step_lengths[0] = 1/2`; on a failure the library's text contains the replica's failure (an inversion: `det F = {v:e} at point
  {q}` of the same element; a refused pivot: its value) and the note `beta = 5e-1`. **DC**: the default attempt and the damped one
  (ADR 0688 D3) fail with the same text (the full step's iterate inverted).
- **The saddle (INC, NC-I).** The ADR 0628 patch (`6^3`, `nu = 0.49`), the plain element (`OFF`) at the homogeneous state
  `lambda = 0.93` of case C49, below its zero `0.9507`, where ADR 0688 PC measured `nu = 21`: the homogeneous field is an
  equilibrium (the patch test; interior residual `<= 1e-13` measured there). `solve_from` (load factor 1, every boundary node
  prescribed at its homogeneous value) from the homogeneous field with the interior nodes moved by `1e-4 h (2U - 1)` (SplitMix64,
  seed `0xF774_0694`): the default refuses (`singular tangent`); with `indefinite_tangent` it converges, to the homogeneous field
  (`|u - u_hom|_inf <= 1e-6 h`), with the inertia `21` at the result. NC-I: rule L at the same state (`nu = 0`, ADR 0688 NC-P):
  the default and the indefinite iteration both converge, bit-equal.
- **The 8^3 block.** IN8, BT8, IB8: the ten-increment `8^3` solve with each opt-in against ADR 0678 (`R/mu 0.36659596`, `w_c
  0.06631664`); INX: `solve_increment` of the last observed `8^3` attempt with and without `indefinite_tangent` (a positive
  definite attempt: the same iterates; the `NonZero` rule does the same arithmetic when no pivot is refused); BT1: one increment
  with the backtracking (ADR 0688 PD1: the default's first attempt `0 -> 1` fails).
- **The corner record on 8^3 (CR8).** D1's helper on the last observed `8^3` attempt (`0.9 -> 1.0`, element `(3, 3, 7)` = 475,
  the punch-corner element of `8^3`, and its 17 neighbours `i, j in {2, 3, 4}`, `k in {6, 7}`): the path's end points bit-equal
  to the observed `u_c` and `u_1`; at `u_c` the cold iteration converges in all 18 elements; the warm iteration from the root at
  `u_c` returns that root bit for bit.
- **Refusals (RF2)**: `indefinite_tangent` with `unsymmetric`; `predictor_backtracking` with `unsymmetric`, with
  `predictor_damping`; each in `ArcLengthOptions`; `predictor_backtracking` with a contribution (`solve_with`): each refused
  naming its key.

### D5. The test file `crates/rcfd-fem/tests/si_eas_hg14.rs`

Helpers copied from `si_eas_hg13.rs` (existing tests are not edited), with `lowest` and `lowest_any` at 500 iterations in both
branches. New: the path and the corner record (D1), the warm states, the column and its replica (D4), the saddle (D4), the
controls of D7.

### D6. Not used

The share of the pencil mode is not a measure of this item; `share_l` is kept only for its control (D7).

### D7. Stronger controls for the surviving mutations of ADR 0688

- **NC-S3 (for MU1, `share` in the Euclidean product).** On the saddle state of D4 (the plain patch at 0.93): `phi` the lowest
  pencil vector, `x` a SplitMix64 vector (seed `0xF774_0694`), `d = x - (phi^T L x / phi^T L phi) phi` (L-orthogonal to `phi` by
  construction): `share_l(phi, d) <= 1e-20` (rounding: `phi^T L d` is `O(eps)` of its terms, squared) and `share_l(phi, phi) >= 1 -
  1e-10`. The Euclidean share of such a `d` is generically of order `1/n` (`n = 375`), far above the limit.
- **SS and DG (for MU2, the shift `1.05 s_lo`).** The registered shift of D5 of ADR 0688 is `1.05 s_hi`: SS checks the shift the
  eigen tool reports (`GeneralizedEigen::shift`, the value given by `Shift::Value`) against `1.05 s_hi` of the returned bracket
  (relative `<= 1e-15`), on the saddle's `(K_ff, I)` and on DG. DG: `K = diag(-2, -1, 0.5, 0.6, ..., 0.5 + 0.1 (n - 3))`, `n =
  40`, `M = I` (points on a line): `nu = 2`, the four lowest `(-2, -1, 0.5, 0.6)` to `1e-8` relative, SB holds. (With 10
  bisections the bracket is narrow enough that `1.05 s_lo` also exceeds `-lambda_1`, which is why MU2 survived; SS checks the
  registered shift itself.)

## Checks

Every measure is required finite before it is compared (a non-finite value or a missing step misses).

### The fast test `f774_fast` (default, not ignored)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| IN8 | c4, c7 | `8^3` ten increments, `indefinite_tangent`: `R`, `w_c` against ADR 0678 | `<= 1e-6` relative each | positive definite path, the same equilibrium |
| BT8 | c5, c7 | the same with `predictor_backtracking` | `<= 1e-6` each | as IN8 |
| IB8 | c4, c5 | the same with both | `<= 1e-6` each | as IN8 |
| INX | c4 | `solve_increment` of the last `8^3` attempt, default and `indefinite_tangent`: `max |du| / max |u|` | `<= 1e-12` (bit-equality reported) | the same arithmetic |
| BT1 | c5 | `8^3` one increment with the backtracking: outcome; if it converges, `R`, `w_c` against BT8 | reported; `<= 1e-6` | |
| RF2 | c4, c5 | the seven refusals of D4 | refused naming the key, each | D2, D3 |
| INC | c4 | the saddle: default refuses; indefinite converges; `|u - u_hom|_inf / h`; `nu` at the result | refuses; converges; `<= 1e-6`; `= 21` | ADR 0688 PC `nu = 21` |
| NC-I | c4 | rule L at the same state: default and indefinite converge; `u` | both converge; bit-equal | D2 |
| BTC | c5 | the column: replica `beta`; library against replica | `= 1/2`; bit-equal `u` and `step_lengths[0] = 0.5`, or the failure text contains the replica's failure and `beta = 5e-1` | D4 arithmetic |
| DC | c5 | the column: default and damped attempts | both fail, the same text | D4 |
| NC-S3 | | `share_l(phi, d)`; `share_l(phi, phi)` | `<= 1e-20`; `>= 1 - 1e-10` | D7 |
| SS | | reported shift against `1.05 s_hi` (saddle `(K, I)`, DG) | `<= 1e-15` relative each | D7 |
| DG | | `nu`; the four lowest; SB | `= 2`; `<= 1e-8` relative each; holds | exact |
| CR8 | c1, c2 | end points; cold converges at `u_c` in the 18 elements; warm from the root returns it | bit-equal; 18; bit-equal | D1 |

### The heavy record `f774_nh16_corner_heavy` (ignored, run once)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| OB | c1 | the observed `16^3` solve: the error text | equal to ADR 0678's | ADR 0688 OB16 |
| PL | c1 | `L(0)` and `P(1)` against the observed `u_c` and `u_1` | bit-equal each | D1 |
| RT | c2 | at `u_c`: the cold iteration converges in the 18 elements | 18 | the driver converged |
| WC | c2 | at `u_c`: the warm iteration from `a_c` returns `a_c` in the 18 elements | bit-equal | the start is the root |
| PB | c3 | 3959 cold at `beta = 0, 1/16, 1/8`: its failure text contains the probe's `det F = ... at point q`; cold converges at `1/32, 1/4, 1/2, 1` | contains, each; converges, each | the probe |
| AS | c1 | the whole-model evaluation at the 81 points; at the probe's points against the probe | reported; equal texts at `0, 1/16, 1/8` and success at the others of the probe | the probe |
| DF | c1 | `det F_c,q` and `det F_q` (cold, warm, continued) of 3959 at the listed points; the minima of the 18 elements | finite (reported) | |
| CT | c2 | the continued root at every point and element; continuity ratio; root switches; warm outcomes | finite (reported; reading JR) | |
| WL | c2 | the warm model along `beta = j / 16`: success, `s(beta)`, `nu(beta)`, the driver's factor | reported | |

### The heavy record `f774_nh16_paths_heavy` (ignored, run once)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| OB | c4 | as above | equal | |
| IN-A | c4 | `solve_increment` with `indefinite_tangent` from `u_c` to `0.965625`, observed: `nu_k` at every iterate; outcome | classified (reported; reading EQ) | |
| EI* | c4 | if IN-A converges: at `u*`, the four pairs of `(K_ff, I)` and of the pencil: the largest residual; negatives among them `= min(nu*, 4)`; SB | `<= 1e-8`; equal; holds | ADR 0688 EI |
| FB | c4 | the F-bar solves at `0.9625` and `0.965625` converge | both | ADR 0678: F-bar solves the block |
| EQ | c4 | if IN-A converges: `|R* / R_Fb - 1|`, `|w* / w_Fb - 1|` at `0.965625` | `<= 0.15` each | the `8^3` gap of rule L to F-bar is 6.6 per cent (ADR 0688 LS-F) |
| BT-A | c5 | `solve_increment` with the backtracking from `u_c`: `beta_0` (from `step_lengths` or the failure note); outcome | `beta_0 = 0.5`; classified | the probe: `1` refused, `1/2` admissible |
| IB-A | c4, c5 | the same with both: `beta_0`; outcome; if it and IN-A converge, `|u_IB - u*|_inf / |u*|_inf` | `= 0.5`; classified; reported | |
| BT-F, IN-F, IB-F | c6 | the full `16^3` solves | classified; if a solve passes: `|R / 0.32599497 - 1|`, `|w_c / 0.06060900 - 1|` `<= 0.15` each, `nu` at full load reported; two passing solves with `nu = 0` agree to `1e-6` relative | ADR 0688 LS-F |

### The default bits and the static checks

| Id | Clause | Measure | Limit |
|---|---|---|---|
| B6 | c7 | `si_eas_hg_default_bits`, `si_eas_hg3_bits`, `si_eas_default_bits`; `baseline_hashes` (rcfd-api) | pass |
| S7 | c7 | `cargo test -p rcfd-fem --release --lib`; `--test si_eas_hg13`, `si_eas_hg12`, `si_eas_hg11`, `si_eas_hg10`, `sa_contact_lifted_f608`, `sa_contact_line_search_f684` (the contact tests ADR 0688 ran), `sa_arclength` (the arc-length check touched) | pass |
| S6 | | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean |
| S8 | | `cargo test -p rcfd-cli --release --test docs_site` if `docs/` is touched | pass |
| S9 | | every commit of the branch builds (`cargo build -p rcfd-fem --release --tests` at each commit) | builds |

### Negative and positive controls

| Id | Control | Expected |
|---|---|---|
| IN8, BT8, IB8, INX, NC-I | the opt-ins on paths that pass | the same equilibrium (INX, NC-I: the same bits) |
| INC | an indefinite equilibrium (a saddle with `nu = 21`) | the default refuses, the indefinite iteration converges to it with its inertia |
| BTC | a derived predictor that inverts at the full step and not at half | `beta = 1/2` |
| DC | the damping of ADR 0688 on the same column | falls back to the full step (its lifted iterate inverts): the default's failure |
| NC-S3, SS, DG | the share of an L-orthogonal vector; the registered shift; a diagonal matrix | `0`; `1.05 s_hi`; the known values |
| CR8, RT, WC | the local solves at a converged state | converge, the warm start returns the root |

### Planned mutations (on a scratch copy of the test commit, each run once against `f774_fast`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `share_l` in the Euclidean inner product | NC-S3 |
| MU2 | `lowest_any`'s shift `1.05 s_lo` instead of `1.05 s_hi` | SS |
| MU3 | `indefinite_tangent` ignored (the positive factor taken) | INC |
| MU4 | the indefinite factor keeps the positive pivot test `d <= 1e-10 a_ii` | INC |
| MU5 | the backtracking always takes `beta = 1` | BTC |
| MU6 | the backtracked update also scales the prescribed values (`lc v + beta (lt - lc) v`) | BTC |
| MU7 | the default factor takes the `NonZero` rule (the opt-in always on) | INC (the default no longer refuses) |

The heavy records are not run under the mutations (they use the same functions, checked by the fast test).

## Decision rule

- **done**: every check of `f774_fast` met; OB, PL, RT, WC, PB, AS (its comparisons), FB, BT-A, IB-A's `beta_0`, B6, S6, S7, S8,
  S9 met in their single registered run; IN-A converges and EI*, EQ met (c4 answered with an equilibrium and its inertia);
  BT-F, IN-F, IB-F classified with their conditional limits met where they apply; JR reads "a jump" or "an inversion" (c3
  answered); every mutation caught; no amendment.
- **partial**: B6 met, but a check missed, a mutation not caught, JR "between", IN-A not converged (the equilibrium not
  reached), or a check met only after an amendment.
- **todo**: B6 fails (a default bit changed), or OB fails and no point of the path could be analysed.

c6 is answered whichever way the full solves end: a remedy that fails is a result, not a miss.

## Limits (not covered)

- One mesh, one material, one load; the corner record covers the 18 elements around 3959 on the lift and the predictor line of
  the failing attempt only, not the other attempts or the other increments (ADR 0682's 20 and 40 increment solves fail at
  `0.870` in the same element: not analysed here).
- The continuation is a numerical continuation on a fixed grid (81 points); a fold of the local equations between two grid
  points would read as a failed continuation (reported, not resolved).
- The indefinite Newton finds a stationary point near its start; that the one found is on the branch of `u_c` is read from the
  F-bar gaps (EQ), not proved. The stability of the branch between `0.9625` and `0.965625` (where `nu` changes) is not located.
- A change of rule L's coefficient is not in scope (ADR 0682 open question 2; the maintainer's call).
- The opt-ins are checked on this block, the column, the patch and `8^3` only; contact, the transient drivers and the arc-length
  path are not run with them (the arc-length path and the unsymmetric factor refuse them).
- Both opt-ins stay off by default.

## Open questions (taken or open)

1. **The design of the indefinite Newton** (the `NonZero` rule with the arc-length test, no analysis cache, the refusals),
   **the backtracking** (from the full step, 10 halvings, the positive test, the failure note, the refusals), **the corner
   record** (the 81-point path, the 18 elements, the continuation, JR's ratio `10` reused from ADR 0688, EQ's `0.01`), **the
   stronger controls** (D7), **the new test file** `si_eas_hg14.rs`: taken on the maintainer's standing instruction of
   2026-11-11 (take the recommendation).
2. **Whether a remedy may change rule L's coefficient**: not taken; open for the maintainer.
3. **Whether either opt-in should become a default**: not taken (it would change default results); open.

## Amendment 1 (2026-11-17, written and committed before the corrected run)

**The defect.** The registered run of `f774_fast` (on 5d958e8f, `f774-scratch/run1-fast.log`) missed BTC, and DC was met only
vacuously: the column's committed displacement was the zero vector, but `solve_increment` (ADR 0688 D2) requires the committed
displacement to carry `lc v` bit for bit at every prescribed equation, and `0 x (-1.2) = -0.0`, whose bits differ from `+0.0`.
All three column attempts (backtracked, default, damped) were refused at the input (`key 'displacement': the committed
displacement has 0e0 at the prescribed equation 26 (lc times its value -0e0)`); none ran. The replica itself ran: `beta = 1/2`
(as derived), then the Newton iteration from it fails at its next iterate with the lower element inverted (`det F =
-1.0056526950039935e-1 at point 0`). Every other row of the registered run was met.

**The correction.** The column's committed displacement is `0 x v` at every prescribed equation (that is `-0.0` where `v < 0`)
and `0.0` elsewhere, the value the driver itself commits at `lc = 0`. DC is made strict: both texts equal **and** they name an
inverted point (`inverted`), so that a refusal at the input cannot meet it. No limit changes. The registered test becomes the
ignored record `f774_fast_known_miss` (unchanged, the measured texts in its ignore text); the corrected test `f774_fast_amended`
(the same rows, the corrected column) is run once and is the target of the mutations.
