# ADR 0718: F-796 registration: the end of the subcritical branch of rule L's 16^3 punch path at amplitude 2.2e-4 (at the last state of ADR 0711's amplitude-controlled trace, whether the trace ends at a fold of the branch or at a breakdown of the local mode problem of element 3720: that element's mode tangent, the iterates of the failing attempts, and the lowest pair and dlambda/da approached with smaller registered amplitude steps)

Status: **REGISTERED 2026-11-17, written before any code of F-796 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no test code of F-796 was written and nothing of this item was run on the block (no probe). The
numbers quoted below come from the published log of ADR 0711 (`w49i-work/f789-scratch/run2-heavy.log`) and from the ADR
texts; the arithmetic on the closed-form toys was done beforehand in a scratch Python script (disclosed below). Results go in
the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-796 (deps F-789), the follow-up proposed by
[ADR 0711](0711-f789-registration-the-subcritical-branch-of-rule-l-s-16-3-punch-path-past-0-9645.md) (its remaining question
1, option (a) with the smaller steps of option (b)). Number 0718 given by the lead. Branch `f796-work` from 07566125 (ADRs
0694, 0697, 0700 and 0711 merged), worktree `w49i-work/f796`, scratch `w49i-work/f796-scratch`. The block, the element, the
material and the branch are those of ADR 0711: the incompatible-modes Hex8 at finite strain with rule L
(`IncompatibleStabilisation::LinearStress`), the plain local update of the modes (the model's default, no warm start), the
neo-Hookean material of ADR 0636 (`mu = 1e6`, `nu = 0.49`), the punch block of ADR 0678 reading c1 on `16^3`, four threads,
the supernodal factor.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would normally be asked about is
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-796 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. At the last state of ADR 0711's amplitude-controlled trace, whether the trace ends at a fold of the branch or at a breakdown of the local mode problem of element 3720: the eigenvalues of that element's mode tangent and the iterates of the failing attempts, the lowest pair and dlambda/da of the branch approached with smaller registered amplitude steps; a fast control on a closed-form fold and on a toy whose local problem loses its solution before the fold; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** the reading at ADR 0711's last state: a fold of the branch or a breakdown
of the local mode problem of element 3720; **c2** the eigenvalues of element 3720's mode tangent; **c3** the iterates of the
failing attempts; **c4** the lowest pair and `dlambda/da` of the branch approached with smaller registered amplitude steps;
**c5** a fast control on a closed-form fold; **c6** a fast control on a toy whose local problem loses its solution before the
fold; **c7** the default bits unchanged.

From the brief, also registered here: the decision criterion (fold if `dlambda/da -> 0` with the element's mode tangent
staying regular; local breakdown if that tangent's smallest eigenvalue `-> 0` or the local iteration diverges first) is fixed
below (D4) before any run; test-only is expected, and if a hook into the element's local solve were needed it would be a
test-visible accessor that changes no computation, with `baseline_hashes`; heavy `16^3` runs capped in steps, progress
printed, `#[ignore]`d and run once, the fast toys always; rule L's coefficient and the defaults are not in scope. Reruns by
name: `si_eas_hg17`, `si_eas_hg16`, `si_eas_hg15`, `si_eas_hg14`, `--lib`.

## What was read

- `AGENT_RULES.md`; the brief; ADR 0711 (whole, and `run2-heavy.log`); ADR 0700 D3, D5; ADR 0694 D2.
- `crates/rcfd-fem/tests/si_eas_hg17.rs` (whole: the block, the observer, `FemCase`, `FemBranch`, `Pitch`, `Bordered`,
  `bordered`, `amp_trace`, `fold_between`, the fast and the heavy tests).
- `crates/rcfd-fem/src/nl_incompatible.rs`: `extended`, `ModeFactor`, `converged` (the plain local Newton of ADR 0608 D2 from
  `alpha = 0`: at most `LOCAL_ITERATIONS = 25` iterations, converged when `|r_a|_inf <= LOCAL_TOLERANCE (1e-12) x scale`,
  the step `alpha -= K_aa^{-1} r_a` by an LU with partial pivoting of the 9 x 9 mode block, an iterate with `det F <= 0` at a
  point refused with key `deformation gradient`), `evaluate`, and the public functions for checks:
  `mode_iteration_record` (the iterates of the same iteration as the driver's evaluation: `|r|_2`, `|r|_inf`, the scale, and
  its outcome), `mode_iteration_record_from` (the same from given modes), `modes_and_strains_with` (the converged modes),
  `enhanced_force_and_tangent_with` (the 33-value force and the 33 x 33 tangent of the extended element at given modes),
  `point_gradients` (`H_c`, `F_m` per point). In `nonlinear.rs`: `internal_force_at` calls `nl_incompatible::evaluate` with
  the model's rule, update and warm start, and wraps an element's error as `element <e>: ...`.

**No library hook is needed.** The mode tangent `K_aa` of an element at its converged modes is the mode block (rows and columns
24 to 32) of `enhanced_force_and_tangent_with` at the modes of `modes_and_strains_with`, both public, both on the same
`extended` evaluation and the same penalty as the driver's; the iterates are those of `mode_iteration_record`. No library file
is changed (check B); `baseline_hashes` is not run.

## What is known before the run (from ADR 0711's log)

1. The amplitude-controlled trace (ADR 0711 D3, from the B state at 0.9645, `a_b = 3.827194241e-5`), rung `k` at `a_b
   2^(k/2)`: `a`, `lambda`, `dlambda/da`, iterations: rung 4 `1.530877696e-4`, `0.960069738280`, `-4.163801e1`, 4; rung 5
   `2.164988001e-4`, `0.958472223921`, `-5.382683e-1`, 4 (continuity `1.222e-4 <= 3.127e-4`, the largest ratio of the trace,
   0.39); at rung 5 the four lowest pairs `-323.40057121476354`, `2285.67`, `3057.08`, `3935.98` (`nu = 1`).
2. The five failing attempts of rung 6 from rung 5 (targets `3.061755393e-4`, then halved toward it: `2.613371697e-4`,
   `2.389179849e-4`, `2.277083925e-4`, `2.221035963e-4`), with their texts (after `fails: `), times 0.7, 0.1, 0.1, 0.7, 0.1 s:
   - `element 3720: key 'internal modes': the modes of an Incompatible Hex8 did not converge in 25 local iterations
     (relative mode residual 3.602825838427964e-5, ADR 0608 D2)`;
   - the same with `2.333797178293884e-1`;
   - `element 3720: key 'deformation gradient': det F = -1.4423001685690635e0 at point 1 of an Incompatible Hex8 element
     (inverted)`;
   - the first form with `3.688331482870459e-4`;
   - the second form with `det F = -8.366015484721789e-1`.
3. Cost: a bordered iteration (residual, tangent, factor) about 0.6 s; an accepted rung 2.3 to 2.9 s; a lowest-pairs solve
   with the shift bracket of `lowest_any` about 1.2 s for the eigen solve plus the inertia factors; the rebuild of rung 5
   (the observed solve, the symmetric chain, the switch, five rungs) about 60 s.

**Arithmetic beforehand (disclosed).** (i) A time of 0.1 s is a failure at the first evaluation of the internal force (at the
predictor, before any factor): the predictor `u + dp v_t` with `dp = (t - a) dlambda/da` changes the load by at most `4.8e-5`
and the amplitude by at most `9e-5` (2.6 per cent for the last attempt): element 3720's local problem fails under a change of
its nodal displacement of order `1e-6` to `1e-5`. Either its local problem is near a loss of solution (a fold of the local
problem, where `K_aa` is singular at the root) or the plain iteration from `alpha = 0` no longer reaches the root; the
relative residuals after 25 iterations (`3.6e-5`, `3.7e-4`) are those of a slowly converging or stalled iteration. Which it
is, and whether the slope's fall from `-41.6` to `-0.54` comes from the continuum branch or from that element, is what D2 to
D4 measure. (ii) The finer steps: `Da = (a_5 - a_4) / 8 = 7.926e-6` (3.7 per cent of `a_5`); 16 steps reach `2.80e-4`,
beyond the second failing target of item 2; six halvings give `Da / 64 = 1.24e-7` (0.06 per cent of `a_5`). (iii) In a
scratch Python script (`f796-scratch/scripts/toys.py`, the closed form, the local Newton and the halving logic in the
amplitude only, no test code), on the toys of D5: PL-F (`c_l = 1/16`) has its fold at `a_f = 1.0090078598`, `p_f =
0.8239388459`, the local ratio there 0.942; the fine trace from `a = 0.8` with `Da = 0.05` accepts every step, brackets the
fold between 1.0 and 1.05 (`dp/da` `-2.0e-2` then `+9.8e-2`, `nu` 1 then 0, ratio 0.945 and 0.929). PL-B (`c_l = 0.390625`,
the local fold at `a* = 0.8`) from `a = 0.5` with `Da = 0.07`, the halvings reset on each acceptance and at most 6 in a row:
accepted up to `0.7999353027`, then 7 failures in a row (lost), the smallest failing target `0.8007175636`, the local ratio at
the last state 0.0163, `dp/da` from `-0.98` to `-43.0` (it grows: a local breakdown steepens the branch, it does not flatten
it), every failure in the local Newton and the local Newton from the last converged `z` failing as well at each failed target
(no root there). These fixed the toys' parameters below.

## Design

### D1. The last state rebuilt

The heavy record rebuilds rung 5 by the same code as ADR 0711 (the functions of `si_eas_hg17.rs` copied, existing tests not
edited): the observed `16^3` solve, the symmetric chain by `solve_increment` with `indefinite_tangent` through `0.963, 0.9635,
0.964, 0.9645, 0.9647`, `phi_f` the lowest pair at 0.9647, the switch with `eps = 2.7468713663317314e-5`, the fixed-load
Newton to 0.9645, and `amp_trace` with 5 rungs (the same arithmetic as ADR 0711's rungs 1 to 5; the loop bound is the only
difference). The load-controlled trace, F-bar and the comparisons of ADR 0711 are not repeated. Rung 4 is `S4`, rung 5 is
`S5`, "ADR 0711's last state".

### D2. The element's mode tangent (c2)

For an element `e` at a converged state: its nodal displacement `u_e` (24 values, node-major), its state, `PointContext`
default; `alpha` from `modes_and_strains_with(.., LinearStress)`; `K` the 33 x 33 tangent of `enhanced_force_and_tangent_with`
at `alpha`; `K_aa` its rows and columns 24 to 32; the asymmetry `max |K_aa - K_aa^T| / max |K_aa|`; the nine eigenvalues of
`(K_aa + K_aa^T) / 2` (`symmetric_eigen`, ascending), `l_min`, `l_max`; the number of local iterations (the record's length
minus one); the smallest `det F` over the points at `alpha` (`point_gradients`). At `S5` for element 3720, and for every
element the ratio `l_min / l_max` (its rank: is 3720 the element with the smallest ratio?).

### D3. The failing attempts and their iterates (c3)

**The re-run.** From `S5`, the five attempts of rung 6 with ADR 0711's arithmetic (the target `a_b 2^3`, halved toward it by
`t = a + 0.5 (t - a)`, the tangent predictor) by a copy of the bordered Newton that also keeps every global iterate and, on a
failure, the iterate whose evaluation failed (the arithmetic of `bordered` unchanged). The failing element `E` is read from
the error text (`element <E>:`).

**The local iterates.** At the failing iterate, element `E`'s `u_e`: (a) the library's record (`mode_iteration_record`, plain
update): `|r|_2`, `|r|_inf / scale` per iterate, the outcome; (b) a test-side replica of the plain local Newton over the
public `enhanced_force_and_tangent_with` (the residual its entries 24 to 32, the step by a copy of the LU with partial
pivoting of `ModeFactor`, the same order of operations and the same singular-pivot test), stepped as many times as the
record has iterates: per iterate `alpha`, `|alpha|_inf`, `|r|_2` (against the record), the smallest `det F`, `l_min` and
`l_max` of `K_aa`; where the library's next evaluation was refused, the replica's evaluation at the same `alpha` is refused
with the same text; (c) the local iteration from `S5`'s converged modes of `E` (`mode_iteration_record_from`, the start
`alpha_5`): converged or not, and if converged `|alpha - alpha_5|_inf / |alpha_5|_inf` and `l_min` there.

**The segment.** On `u_e(t) = u_e(S5) + t (u_e(fail) - u_e(S5))`: (d) the first `t*` where the plain local iteration from
`alpha = 0` fails (bisection on `[0, 1]`, 30 halvings, if it converges at 0 and fails at 1), and `l_min` at the converged
side; (e) the continuation in `t` in 64 equal steps, each local iteration started from the previous converged modes
(`mode_iteration_record_from`): the last `t` reached and `l_min` along it (its smallest value and the value at the last
`t`). (d) and (e) tell an iteration that no longer reaches an existing root (the continuation passes `t*`) from a local
problem that loses its root (the continuation also stops, `l_min -> 0` there).

### D4. The fine trace and the decision criterion (c1, c4)

**The fine trace.** From `S4`, under amplitude control by the bordered Newton of ADR 0711 D3 (the same predictor, the same
convergence tests, the same continuity test `|u - u_pred|_inf <= |u_pred - u_from|_inf`), the targets `t_j = a_4 + j Da`,
**`Da = (a_5 - a_4) / 8`**, `j = 1 .. 16`. A failed attempt (not converged, discontinuous, or an element refused) is halved
toward its target (`t = a + 0.5 (t - a)` from the current state); the count of halvings is **reset on each acceptance** (so
the trace closes in on its end geometrically), at most **6 halvings in a row** (the 7th failure in a row loses the trace),
at most **40 solves**. Stops: two accepted states after a bracketed fold ("fold + 2"); `j = 16` reached ("steps"); the cap or
the halvings ("lost"). The rung trace of ADR 0711 kept its halvings per rung and stopped after 5 failures; the reset is the
change that makes "approached with smaller steps" close in on where the trace ends.

At each accepted state: `a`, `lambda`, `sigma = dlambda/da = 1 / (phi_f . v_t)`, `nu`, the four lowest pairs (`lowest_any`),
the share of the lowest vector's squared 2-norm on element `E`'s free equations, element `E`'s `l_min`, `l_max` and local
iterations (D2), and the local iteration from the previous accepted state's modes of `E` (`|alpha_warm - alpha|_inf /
|alpha|_inf`: the same root or another one). At each failed attempt: the text, the global iteration of the failure, and
whether it is **local**: the failing element's record has at least one iterate (the evaluation at `alpha = 0` passed, so
the failure came from the local iteration, not from an inverted compatible field).

**The decision criterion (registered now, the brief's criterion made operational).** With `rho_j = l_min,j / l_min,0` (`l_min`
of element `E` at the accepted states, `0` the start `S4`; `rho` undefined, all comparisons with it false, if `l_min,0 <= 0`)
and **`rho_reg = 0.1`** (an order of magnitude below the start: "staying regular" is `rho >= 0.1`, "`-> 0`" is `rho <
0.1`):

- **"fold"**: the first sign change `sigma_{j-1} < 0 <= sigma_j` between consecutive accepted states has `nu` changing by
  exactly one, `l_min > 0` and `rho >= 0.1` at both states, and `l_min` changed sign on no earlier segment. (A sign change
  that fails these reads "undecided".)
- **"local breakdown: the mode tangent"**: no sign change of `sigma`, and either `l_min` changes sign between two consecutive
  accepted states or the trace is lost with `rho < 0.1` at its last accepted state.
- **"local breakdown: the local iteration"**: no sign change of `sigma`, no sign change of `l_min`, the trace lost with `rho >=
  0.1` at its last accepted state, and every failed attempt after that state local.
- **"undecided"**: otherwise (a lost trace with a non-local failure among the final ones, or a trace that reaches `j = 16`
  without a sign change: "no end up to a = ...").

Reported with the reading, not deciding it: the linear extrapolations from the last two accepted states of the amplitude
where `sigma` and where `l_min` would vanish, `|sigma_last| / max |sigma|` along the fine trace, the interval `[a_last,
t_fail,min]` in which the trace ends, and D3 (d), (e).

**Consistency with ADR 0711.** If the fine trace accepts `t_8` (`= a_5` up to rounding), its state there is `S5` again:
`|lambda - lambda_5| <= 1e-8` (the Newton tolerance on a regular bordered system) and `mu_1` against `-323.40057121476354`
within `1e-6` relative. If it is lost before `t_8`, `S5` is not reached by continuation in the finer steps (a reading in
itself: `S5` would lie beyond a jump of the element's local root); that is reported, not a miss.

### D5. The fast controls on the same code (`f796_fast`)

The fine trace, the classification of failures, the reading and the local diagnostics are written once over the branch case
of ADR 0711 extended with a closure for the watched element's `l_min` and one for the classification; the heavy record and the
toys call the same functions.

**The toy PL** (ADR 0711's PT generalised by a condensed local unknown): on the free unknowns `u_0 .. u_7` of PT (`b3`, `b5`,
`sb = 1`) and a local unknown `z` with the energy `W_l = (k_l/2) z^2 - z^3/3 - c_l u_0^2 z` (`k_l = 1`), the local equation
`g = k_l z - z^2 - c_l u_0^2 = 0`, solved per evaluation by Newton from `z = 0` (25 iterations, `|g| <= 1e-14 (k_l |z| + z^2 +
c_l u_0^2)`, the element's rule in miniature; a failure is an error `local: ...`), and the term `-2 c_l u_0 z` added to `r_0`
(`dW_l/du_0`). The condensed tangent adds `-2 c_l z - 2 c_l u_0 z'`, `z' = 2 c_l u_0 / (k_l - 2 z)`, to `K_00`; the local
tangent is `g_z = k_l - 2 z` (its `l_min`). On the branch (`u_1 = (0.1 p - a^2) / 2`): `z(a) = (k_l - sqrt(k_l^2 - 4 c_l
a^2)) / 2`, `p(a) = (1 + (b3 - 1) a^2 + b5 a^4 - 2 c_l z(a)) / 0.9`, closed form; the local problem loses its root at `a* =
k_l / (2 sqrt(c_l))`, where `g_z = 0` and `dp/da -> -inf`. `phi = e_0`, `a = u_0`.

- **PL-F, a closed-form fold with a regular local problem (c5)**: `b3 = 0.5`, `b5 = 0.25`, `c_l = 1/16` (`a* = 2`); the fold
  where `dp/da = 0`, `a_f = 1.0090078598`, computed in the test by bisection on the closed-form `dp/da`. The fine trace from
  `a = 0.8` (the closed form polished by the bordered Newton), `Da = 0.05`, 16 steps, 6 halvings, cap 60.
- **PL-B, the local problem loses its root before the fold (c6)**: `b3 = 0.5`, `b5 = 0.25` (without the local term, the fold
  of ADR 0711's PT-U5 at `a = 1`), `c_l = 0.390625` (`a* = 0.8 < 1`); the fine trace from `a = 0.5`, `Da = 0.07`, 16 steps, 6
  halvings, cap 60. At each failed attempt, the local Newton from the last accepted `z` at the failed target.
- **E1, the element plumbing (negative control of the replica)**: one Hex8 `[0, 1]^3` with rule L and the material, the
  displacement `u = (0.02 (2y - 1)(2z - 1), 0.02 (2x - 1)(2z - 1), -0.15 z)` at its nodes, the initial state: the library
  record converges; the replica (D3 b) reproduces its iterates and the converged modes; `K_aa` symmetric; the eigenvalues
  sum to its trace.

## Checks

Every measure is required finite before it is compared (a non-finite value or a missing state misses).

### The fast test `f796_fast` (default, not ignored)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| PF-0 | c5 | the start at 0.8: `|p - p(0.8)|` | `<= 1e-12` | closed form |
| PF-1 | c5 | at every accepted state: `|p - p(a)|`; `|sigma - dp/da(a)| / |dp/da(a)|`; `|g_z - (1 - 2 z(a))|` | `<= 1e-12`; `<= 1e-8`; `<= 1e-12` | closed form, Newton tolerance |
| PF-2 | c5 | no failed attempt; the stop | holds; "fold + 2" | arithmetic (iii) |
| PF-3 | c1, c5 | the reading; `a_f` in the bracket `[a_{j-1}, a_j]`; `nu` before and after; `rho` at both | "fold"; holds; `1`, `0`; `>= 0.1` | arithmetic (iii): `0.945`, `0.929` |
| PB-0 | c6 | the start at 0.5: `|p - p(0.5)|` | `<= 1e-12` | closed form |
| PB-1 | c6 | at every accepted state: `|p - p(a)|`; `|sigma - dp/da(a)| / |dp/da(a)|`; `|g_z - (1 - 2 z(a))|`; `nu` | `<= 1e-10`; `<= 1e-6`; `<= 1e-8`; `= 1` | closed form near a double root (`z` error `~ g / g_z`) |
| PB-2 | c6 | the stop; `a_last < a* = 0.8 < t_fail,min`; `t_fail,min - a_last` | "lost"; holds; `<= 0.07 / 2^6 = 1.09e-3` | arithmetic (iii): `7.8e-4` |
| PB-3 | c1, c6 | the reading; `rho` at the last state; `|sigma_last| / |sigma_0|` | "local breakdown: the mode tangent"; `< 0.1`; `> 10` | arithmetic (iii): `0.0163`, `~44` |
| PB-4 | c3, c6 | every failed attempt local (`local: ...`); at every failed target above `a*` the local Newton from the last accepted `z` fails too | holds; holds | no root above `a*` |
| E1 | c2, c3 | the record converges; replica `|r|_2` per iterate against the record (relative); `|alpha_replica - alpha|_inf / |alpha|_inf`; `K_aa` asymmetry; `|sum l_i - tr K_aa| / max |l_i|` | holds; `<= 1e-14` each; `<= 1e-14`; `<= 1e-12`; `<= 1e-12` | the same arithmetic |

### The heavy record `f796_nh16_end_of_branch_heavy` (ignored, run once)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| OB | | the observed `16^3` solve: the error text | equal to ADR 0678's | ADR 0711 OB |
| R1 | | the step to 0.9645: `a` against `3.827194476e-5`; `nu` | `<= 1e-8` rel; `= 1` | ADR 0711 R1 |
| S5 | | rungs 1 to 5 accepted; at rung 5 `|lambda - 0.958472223921|`; `a` against `2.164988001e-4`; `sigma` against `-5.382683e-1`; `mu_1` against `-323.40057121476354` | holds; `<= 1e-12`; `<= 1e-9` rel; `<= 1e-6` rel; `<= 1e-9` rel | the same arithmetic, the log's printed digits |
| X1 | c3 | the re-run of the five attempts: each text against ADR 0711's (item 2); `E` from the first | equal each; `E = 3720` | the same arithmetic |
| X2 | c3 | per attempt, at the failing iterate: the record's outcome text contained in the attempt's text; replica `|r|_2` against the record per iterate (relative); the replica's refusal (if the library's next evaluation was refused) with the same text | holds; `<= 1e-14` each; holds | the same arithmetic |
| X3 | c2 | at `S5`, element `E`: the asymmetry of `K_aa`; the nine eigenvalues; `l_min / l_max`; local iterations; smallest `det F`; the rank of `E` by `l_min / l_max` among the 4096 elements | `<= 1e-12`; reported; reported; reported; reported; reported | D2 |
| X4 | c3 | per attempt: (c) the local iteration from `alpha_5`; (d) `t*` and `l_min` there; (e) the last `t`, the smallest `l_min` | reported | D3 |
| FT0 | c4 | if `t_8` accepted: `|lambda - lambda_5|`; `mu_1` against `-323.40057121476354` | `<= 1e-8`; `<= 1e-6` rel | the same root |
| FT1 | c4 | at every accepted state: the constraint (relative); the continuity test | `<= 1e-9`; holds | ADR 0711 AT2 |
| FT2 | c1 | the reading | not "undecided" | D4 |
| FT3 | c2, c4 | the table: `a`, `lambda`, `sigma`, `nu`, the four lowest pairs, the share on `E`, `l_min`, `l_max`, `rho`, local iterations, warm-start agreement; the failed attempts | reported | D4 |

The answer of c1 is the reading FT2 with D3 (d), (e) beside it; c2 is X3 and the `l_min`, `l_max` column of FT3; c3 is X1,
X2, X4 and the failed attempts of FT3; c4 is FT3.

### The default bits and the static checks

| Id | Clause | Measure | Limit |
|---|---|---|---|
| B | c7 | `git diff --stat 07566125 -- crates/*/src` empty at the tip | empty |
| S7 | | `cargo test -p rcfd-fem --release --lib`; `--test si_eas_hg17`, `si_eas_hg16`, `si_eas_hg15`, `si_eas_hg14` (their default tests); `--test si_eas_hg18` (`f796_fast`) | pass |
| S6 | | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` | clean |
| S8 | | `cargo test -p rcfd-cli --release --test docs_site` (docs/ touched by this ADR) | pass |
| S9 | | every commit builds (`cargo build -p rcfd-fem --release --tests` or the test target it adds) | builds |

`baseline_hashes` is not run: no shared solver file is touched (B checks it).

### Negative and positive controls

| Id | Control | Expected |
|---|---|---|
| PL-F | a closed-form fold with a regular local problem | "fold", bracketed at the closed-form `a_f`, `rho` stays above 0.1 |
| PL-B | a local problem that loses its root before the fold | "local breakdown: the mode tangent", lost within `1.09e-3` of `a*`, `|dlambda/da|` growing |
| E1 | the replica of the local iteration on one element | the library's iterates bit for bit |
| X1 | ADR 0711's five failures re-run | the same texts |

### Planned mutations (on a scratch copy of the test commit, each run once against `f796_fast`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the fold read on `sigma_{j-1} > 0 >= sigma_j` (reversed) | PF-2, PF-3 (no fold) |
| MU2 | `rho = l_min,0 / l_min,j` (inverted) | PB-3 (reading "the local iteration", `rho > 0.1`) |
| MU3 | `sigma = phi . v_t` in place of its inverse in the fine trace | PF-1, PB-1 (against `dp/da`) |
| MU4 | the replica's local step `alpha += delta` | E1 |
| MU5 | the halvings not reset on acceptance (ADR 0711's rule) | PB-2 (`t_fail,min - a_last`), PB-3 |
| MU6 | the toy's local Newton with `g_z = k_l - z` | PF-1, PB-1 (`g_z` against the closed form, or not converged) |

The heavy record is not run under the mutations.

## Decision rule

- **done**: every check of `f796_fast` and of the heavy record met in their single registered runs, B, S6 to S9 met, every
  mutation caught, no amendment, and the reading FT2 is "fold" or one of the two "local breakdown" readings (c1 answered).
- **partial**: B met, but the reading is "undecided", or a check missed, a mutation not caught, or a check met only after an
  amendment.
- **todo**: B fails, or OB fails and nothing is measured.

## Limits (not covered)

- One state, one element of interest, one load path; the branch with `+phi_f` only. The rank of element 3720 (X3) is the
  only look at the other elements.
- The reading is about the trace with the plain local update from `alpha = 0` (the model's default). The opt-ins of the local
  update (the backtracking of ADR 0655, the decrease test of ADR 0658, the trust region and the warm start of ADR 0660) are not
  run on the block; D3 (c), (e) look at the warm start on the element only.
- `rho_reg = 0.1` is a threshold, not a proof: a mode tangent that falls below a tenth of its start value is read as tending
  to zero; the extrapolations are reported beside it.
- The arc-length driver of the library is not used (ADR 0711 D1, its follow-up F-797).
- Rule L's coefficient is not in scope; no opt-in becomes a default.

## Open questions (taken or open)

1. **The design** (no library hook, the replica of the local iteration in the test, the fine steps `(a_5 - a_4) / 8` from
   `S4` with the reset halvings, `rho_reg = 0.1`, the toys PL-F and PL-B, the new test file `si_eas_hg18.rs`): taken on the
   maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. **Whether either opt-in of ADR 0694 or of the local update becomes a default**: not taken (it would change default
   results); open.
