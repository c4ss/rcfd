# ADR 0688: F-767 registration: the failing Newton iterate of rule L at the punch corner (the failing attempt of ADR 0682 recorded iterate by iterate through a bit-neutral observer, the predictor's share in the isolated pencil mode, the lowest eigenvalue of the free tangent and of the ADR 0628 pencil at every iterate located by element, the ADR 0628 sweep at the state of the punch-corner element, the line search of ADR 0607 and an opt-in damping of the predictor on the same attempt and on the whole 16^3 solve, a positive control on the ADR 0628 patch below its zero)

Status: **REGISTERED 2026-11-17, written before any code of F-767 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-767 was written and nothing of this item was run (no probe). The
numbers quoted below come from the published logs of ADR 0682 and ADR 0678 (`w49i-work/f763-scratch/run3-heavy-tangent.log`,
`run4-heavy-variants.log`, `w49i-work/f747-scratch/run3-heavy.log`) and from the ADR texts. Results go in the addenda at the
end; nothing above the addenda is changed after a run.

Roadmap item F-767 (deps F-763), the follow-up proposed by
[ADR 0682](0682-f763-registration-rule-l-loses-positive-definiteness-on-the-refined-neo-hookean-punch-block.md) (its remaining
questions 1, 2 and 4, each option (a)/(b) recommended there). Number 0688 given by the lead. Branch `f767-work` from 1c991cb7
(F-763 / ADR 0682, F-762 / ADR 0683 and F-747 / ADR 0678 merged), worktree `w49i-work/f767`, scratch `w49i-work/f767-scratch`.
The element is the incompatible-modes Hex8 at finite strain of ADR 0608 with rule L (`IncompatibleStabilisation::LinearStress`,
ADR 0630 D1), the neo-Hookean material of ADR 0636 (`mu = 1e6`, `nu = 0.49`), the punch block of ADR 0678 reading c1
(`[0, 1]^3` on `16^3`, the die constraints, `u_z = -0.1` on the `zmax` nodes with `x, y <= 0.5`, ten increments, four threads,
the supernodal factor), as `si_eas_hg12.rs` builds it.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the
open questions at the end.

## What F-767 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The failing attempt of ADR 0682 (rule L, neo-Hookean, the punch block on 16^3, from the converged state at load factor 0.9625) recorded iterate by iterate: the predictor's share in the isolated pencil mode of ADR 0682 and the lowest eigenvalue of the free tangent and of the ADR 0628 pencil at every iterate, located by element; the ADR 0628 sweep at the state of the punch-corner element; the line search of ADR 0607 and a registered damping of the predictor on the same attempt, and whether the 16^3 block then solves to full load; a positive control on the ADR 0628 patch below its zero; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** the failing attempt from `0.9625` recorded iterate by iterate; **c2** the
predictor's share in the isolated pencil mode; **c3** the lowest eigenvalue of the free tangent at every iterate, located by
element; **c4** the same for the ADR 0628 pencil; **c5** the ADR 0628 sweep at the state of the punch-corner element; **c6** the
line search of ADR 0607 on the same attempt; **c7** a registered damping of the predictor on the same attempt; **c8** whether
the `16^3` block then solves to full load; **c9** a positive control on the ADR 0628 patch below its zero; **c10** the default
bits unchanged.

## What was read

- `AGENT_RULES.md`; the brief; ADR 0682 (whole; its logs `run3-heavy-tangent.log` and `run4-heavy-variants.log`); ADR 0607
  (D1 the line search, its limits: "an iteration whose prescribed increment is not zero ... takes the full step"); ADR 0628 (the
  sweep, the patch, the plain element's zero `lambda_s`, table Z); ADR 0678 (the F-bar `16^3` neo-Hookean values in
  `f747-scratch/run3-heavy.log`, line 20).
- `crates/rcfd-fem/tests/si_eas_hg12.rs` (whole), `si_eas_hg3.rs` (`patch`, `homogeneous`, `interior`, `dense_free`,
  `laplacian`, `lowest10`, `kaa_min`, the cases).
- `crates/rcfd-fem/src/nonlinear.rs`: `NewtonOptions` and `check`, `Problem::attempt_with` (lines 1756 to 2002 at 1c991cb7),
  `trial_slope`, `step_length`, `factor_solve`, `solve_with`, `solve_from` and `solve_from_field_reusing`, `contribute_with`,
  the traits `Contribution` and `StepControl`, `initial_states`; `arclength.rs` (`ArcLengthOptions::check`); `contact.rs` (the
  line-search refusal); `nl_incompatible.rs` (the public element functions, `LOCAL_TOLERANCE = 1e-12`, the cold start from
  zero modes); `crates/rcfd-linalg/src/eigen.rs` (`lowest_eigenpairs`, `Shift::Auto` and `Shift::Value`, the `Positive` pivot
  rule of the shifted factor, the Sturm check).

## What is known before the run (from the published logs)

1. **The attempts from `u_c`** (ADR 0682 RP1): after `0.95625 -> 0.9625` converged, `0.9625 -> 0.96875` failed and `0.9625 ->
   0.965625` failed (the fifth halving), the latter with the text `singular tangent: pivot -4.651826504388123e6 at node 4768
   component 0 (diagonal 2.8253460477227545e6)`. Node 4768 is `(0.5, 0.5, 1)`, the punch corner. **The failing attempt** of
   this item is the last one, `0.9625 -> 0.965625`, whose failure is the published error text. The driver reports the failure
   of the last attempt only, and a pivot refusal happens after the force and tangent of that iterate were evaluated.
2. **At `u_c`** (ADR 0682, at the state of its replicated path, equal to the driver's within the Newton tolerance): `nu = 0`,
   the driver's factor passes, `lambda_1 / (mu h) = 3.633098e-2`, the pencil's four lowest `6.286570e-2, 0.615, 0.649, 0.674`
   (over `mu`), the lowest pencil mode in `|A| = 2` elements, the first **3720 at `(8, 8, 14)`**, `H = 0.88`; KA smallest in
   3720 (`37.3 mu h^3`).
3. **The size of an increment.** `0.003125 x 0.1 = 3.1e-4` of punch displacement, `0.5` per cent of `h = 1/16`. A tangent that
   is positive definite at `u_c` with the lowest eigenvalue `0.036 mu h` changes by `O(3e-4 / h)` relatively over such a step if
   the force is smooth; a pivot of `-1.65` times its diagonal one or two iterates later needs either a large component of the
   iterate in a soft mode (the candidate of ADR 0682), a discontinuity of the element force (a jump of the locally solved modes,
   which are solved from zero at every evaluation), or a diverging Newton sequence. The measures below distinguish these.
4. **The ADR 0628 zero** used by the positive control: the plain element on the `6^3` patch in confined compression at `nu =
   0.49` (case C49) has `lambda_s = 0.9507467` (ADR 0628 table Z); rule L stays at `m_L >= 0.977` there.
5. **Cost** (ADR 0682 logs): the `16^3` solve `45` to `60 s`; per state: tangent and reduction `0.2 s`, the supernodal factor
   `0.7 s`, four pairs of `(K_ff, I)` `5.5 s`, of the pencil `28.7 s` (167 iterations), the element tangents `0.3 s`. Expected
   iterates of the failing attempt: unknown (at most 21); per iterate about `40 s` with `nu = 0`, more with the bisection below
   (about 30 factors, `20 s`). The heavy record of the iterates: about `5` to `15` minutes; of the remedies (three full solves
   with trial evaluations, three single attempts): about `5` to `10` minutes. Each is `#[ignore]`d and run once.

## Design

### D1. The observer (test-only; no library change)

The driver keeps no iterate. A **contribution** (`Contribution`, ADR 0440 D7) that adds nothing is called by the driver at
every iterate with the iterate `u` and the attempt's load factor (`contribute_with`, after `force_and_tangent`), and a
step-control hook (the `Recorder` of ADR 0682, returning `proposed`) is called before every attempt. Together (sharing one
`RefCell` log) they record every iterate of every attempt: the attempt `(committed, attempted)` and the list of iterates `u_0 =
u_c, u_1, ..., u_K` whose force was evaluated. **Bit-neutrality, derived**: with one contribution the driver (i) keeps a clone
of `f_int` (the converged iterate's force, the same values); (ii) adds the contribution's force `0.0` to every entry
(`x + 0.0 = x` bit for bit, except `-0.0 + 0.0 = +0.0`, and every later use of the entry is a sum, a product or a
comparison, where the two zeros agree; the residual `0 * f - g` of the two zeros is the same `+0.0` or `-0.0`, its square the
same); (iii) adds no triplet (`add_triplets` with an empty list changes nothing); (iv) does not enter the residual reference
(`in_residual_reference` false); (v) takes no fallback tangent (`has_fallback_tangent` false). Its `add_trial_force` is
overridden to record nothing (a line-search trial is not an iterate). The checks OB8 and OB16 test the neutrality. The
contribution is not given to the runs of the predictor damping (D3 refuses contributions).

### D2. A single attempt of the driver from a committed state (library, new public function)

`rcfd_fem::nonlinear::solve_increment(model, prescribed, load, committed, states, lc, lt, options, contributions) ->
Result<Equilibrium>`: **one attempt of `solve_with`** (`Problem::attempt`, ADR 0404 D5 step 4) from the committed displacement
`committed` (which must carry `lc v` at the prescribed equations, bit for bit, `v` their values; refused otherwise, key
`"displacement"`), the committed `states`, the committed load factor `lc` to the target `lt` (`0 <= lc < lt`, finite; key
`"load factor"`), the load vector `load` at factor 1 (one value per equation): the driver's predictor with the prescribed
increment `(lt - lc) v`, then the Newton iteration, with the driver's options and criteria and a fresh factor cache (ADR 0566
D2: the cache does not change bits). It is what `solve_with` runs for each attempt, made callable so that the same attempt can
be run with other options (c6, c7). A new function: no existing path calls it (defaults unchanged by construction). Checked
by BX8 and BX16 (it reproduces the driver's attempt bit for bit).

### D3. The damping of the predictor (library, opt-in `NewtonOptions::predictor_damping`)

**Why the line search of ADR 0607 does not reach the predictor.** Iteration 0 of an attempt with prescribed values carries the
increment `(lt - lc) v` and takes the full step (ADR 0607, limits). Its correction is `du = K_ff^{-1} (r_F(u_c) - K_FP dg)`,
`dg = (lt - lc) v`, followed by the prescribed values set to `lt v`.

**Derivation.** Let `u_L` be the *lifted* iterate: the free values of `u_c` and the prescribed values `lt v`. To first order
`r_F(u_L) = r_F(u_c) - K_FP dg`, so `du = K_ff(u_c)^{-1} r_F(u_L) + O(|dg|^2)`: the predictor's free part is a Newton
correction from `u_L` with the tangent at `u_c`. With the prescribed values held at `lt v` the total potential `Pi(u_L + beta
du)` of a hyperelastic body under dead loads has the derivative `s(beta) = -du . r_F(u_L + beta du)`, and `s(0) < 0` when
`K_ff(u_c)` is positive definite and `du` is (up to the `O(|dg|^2)` term) its Newton direction at `u_L`. So the predictor is
damped by **the step-length rule of ADR 0607 D1 applied from `u_L`**: `s_0 = s(0)` by a trial evaluation at `u_L`; `beta = 1`
when the trial at `1` is admissible and `s(1) <= eta |s_0|` (`eta = 0.5`), else the Illinois regula falsi on `(0, 1)` with up to
8 further trials, accepted when admissible and `|s(beta)| <= eta |s_0|`. **Admissible** (the one addition to ADR 0607): the
evaluation succeeds, `s` is finite, **and the reduced tangent at the trial passes the driver's pivot test** (the factor of
`factor_solve`, the same test `d <= 1e-10 a_ii`); an inadmissible trial counts as an overshoot (it replaces `b`), as a failed
evaluation does in ADR 0607. The reason for the addition: the failure of ADR 0678 is a refused pivot, which the slope alone does
not see. When the 8 trials are spent, `beta` is the bracket's lower end `a` if `a > 0` (admissible, `s(a) < 0`), else the last
trial's value. If `s_0` is not `< 0` or the evaluation at `u_L` fails, `beta = 1`. Then `u_F += beta du` (with `beta = 1` by `u_i
+= du_i`, exactly as without the option) and the prescribed values `lt v`; the energy of iteration 0 stays `|du . b|` of the full
correction. `eta` and the 8 trials are those of ADR 0607, not chosen here.

**Scope.** `pub predictor_damping: bool`, **default `false`**, builder `predictor_damping(bool)`. Active only in an attempt
without a load-factor control, with the symmetric factor, at iteration 0, when some value of the prescribed increment is not
`0.0`. Refusals (key `"predictor_damping"`, `FemError::InvalidValue` or the driver's failure text): `NewtonOptions::check` with
`unsymmetric`; `ArcLengthOptions::check` with `newton.predictor_damping`; an attempt with any contribution (the trial tangent
would need the contributions' tangents, whose `add` may record; refused rather than approximated). Records: `step_lengths`
(`LoadStep`, `Equilibrium`) gets one value per correction when `line_search` or `predictor_damping` is on (`beta` at iteration
0 with the damping, else `1.0`; `alpha` of ADR 0607 at iterations `>= 1` with the line search, else `1.0`); with `line_search`
alone the record is what it was (`1.0` at iteration 0).

**The code of the line search**: `step_length` is split into the computation of `s_0` and the search from a given `s_0`
(`search_from`), with the admissibility test a parameter (`false` for ADR 0607: no pivot test); the arithmetic of the line
search is unchanged (the same operations in the same order). The default path (`line_search` and `predictor_damping` false)
executes no new code; `baseline_hashes` is run (rule 3: a shared solver file).

### D4. The test file `crates/rcfd-fem/tests/si_eas_hg13.rs`

Helpers copied from `si_eas_hg12.rs` and `si_eas_hg3.rs` (existing tests are not edited). New: `Log`, `Observer`, `Watch`
(D1); `replicate_step` (the driver's correction of iteration `k` rebuilt from the observed iterate: `internal_force`,
`tangent` with the committed states, `reduce` with the increment `(lt - lc) v` at `k = 0` and `0` after, the driver's factor
and solve, the update; BI); `lowest_any` (D5); `share_l` (D6); `tc` (D7); `mode_jump` (D8); `corner_sweep` (D9).

### D5. The lowest eigenvalue at an iterate that may be indefinite

At `u_c` ADR 0682 used `lowest_eigenpairs` with `Shift::Auto`, whose candidates are `>= 0` and whose factor must be positive
definite (`PivotRule::Positive`). At an indefinite iterate the shift must exceed `-lambda_1`. `lowest_any(K, M)`:

- `nu` = the inertia of `K` (the supernodal `L D L^T`, `NonZero` pivots, nested dissection; ADR 0682).
- `nu = 0`: the tool of ADR 0682 unchanged (four pairs, `Shift::Auto`, tolerance `1e-8`, the supernodal factorisation with the
  coordinates, at most 300 iterations).
- `nu >= 1`: `rho = sum |k_ii| / sum m_ii`; `s_j = 1e-6 rho 2^j`, `j = 0, 1, ..., 60`, `s_hi` the first with inertia of `K + s_j
  M` zero, `s_lo = s_{j - 1}` (`0` for `j = 0`); 10 bisections of `(s_lo, s_hi]` keeping inertia zero at `s_hi` (a refused
  count counts as non-zero). By Sylvester's law `-lambda_1` lies in `(s_lo, s_hi]` (SB checks the computed `lambda_1` against
  it). Then four pairs with `Shift::Value(1.05 s_hi)` (positive definite: `lambda_1 + 1.05 s_hi >= 0.05 s_hi > 0`), the same
  tolerance and factorisation, at most 500 iterations.
- Reported: the values, the residuals, the iterations, the shift, the bracket.

### D6. The predictor's share in the isolated pencil mode (c2)

`phi`: the lowest eigenvector of the pencil `(K_ff, L_ff)` at the driver's `u_c` (`L_ff` the vector Laplacian of the Full Hex8
on the free block, ADR 0682; `phi^T L phi = 1` as returned). For a free vector `d`: **`share(d) = (phi^T L d)^2 / (d^T L
d)`** in `[0, 1]` (the squared cosine in the `L` inner product, the inner product in which the mode is isolated: ADR 0682 found
it at `0.063 mu` against `0.615 mu` next). Reported for the predictor `d_0` (**`S_P = share(d_0)`**, c2), for every correction
`d_k` and for the total change `u_k - u_c`; beside it the Euclidean share `(phi . d)^2 / (|phi|^2 |d|^2)`. The isolated mode
at the driver's `u_c` is checked to be the one of ADR 0682 (I0).

### D7. Tangent consistency at an iterate

`TC(u, d) = |(f_int(u + eps dh) - f_int(u - eps dh)) / (2 eps) - K dh|_2 / |K dh|_2` on the free equations, `dh = d / |d|_inf`
(free part), `eps = 1e-5 h`, `h = 1/n`. The condensed tangent is exact (ADR 0614: consistent tangent `2.6e-11` to `2.0e-8`
measured); the central difference has a truncation error `O((eps / h)^2) = O(1e-10)` and a local-solve error of order
`LOCAL_TOLERANCE / (eps / h) = 1e-7` at most. Limit at `u_c` along `d_0`: `<= 1e-6`; at the other iterates reported (a value
above `1e-2` would mean the force is not smooth along the correction: a jump).

### D8. The jump of the modes

At every iterate, every element: the modes `a_e(u_k)` (`modes_and_strains_with`, rule L, the committed state, the default
cold local update), `Delta a_e = |a_e(u_k) - a_e(u_c)|_2`, `Delta u_e = |u_k,e - u_c,e|_2`. Reported: `max_e Delta a_e` and its
element; `max_e Delta a_e / Delta u_e` over the elements with `Delta u_e > 0` and its element.

### D9. The ADR 0628 sweep at the state of the punch-corner element (c5)

The patch of ADR 0628 (`6^3` Hex8 on the unit cube, the interior nodes moved by `0.1 h (2U - 1)` with the seed `0xF685_0614`,
every boundary node fixed, `nu = 0.49`), the homogeneous field `u = (F - I) X` at every node, the measure `m` the lowest
eigenvalue of `(K_ff, L_ff)` over `mu` on the interior equations (dense, `generalized_eigen_dense`, as `lowest10` of
`si_eas_hg3.rs`). `F` is the **compatible centre gradient** of an element of the block (the gradient of the trilinear field at
the element centre, `centre_stretch`'s `F` of ADR 0682). The elements and states: **E_c = 3959** (`(7, 7, 15)`, the top-layer
element of the punch with the corner node 4768) and **E_p** = the first element of the pencil mode at `u_c` (expected 3720), each
at `u_c` and at the last iterate `u_K`; and **E_K** = the first element of the lowest free-tangent mode at `u_K` if it is
neither. At each `(element, state)`: `m_L` (rule L), `m_Inc` (the plain element), `m_Full`, `m_Fbar`, the interior equilibrium
residual `max |f_free| / max |f|`, and along the ray `F(t) = I + t (F - I)`, `t = 0, 0.1, ..., 1.5`: `m_L(t)`, `m_Inc(t)` and
the first `t` where each is `<= 0`. **Reading SH**: "the corner element's homogeneous state is unstable for rule L" if `m_L <=
0` at `(E_c or E_p, u_K)`; "not a homogeneous-state property" if `m_L >= 0.5` at all four states of E_c and E_p (the floor of
ADR 0628's sweep); "between" otherwise.

### D10. The positive control (c9)

The ADR 0628 patch, C49 (`F = diag(1, 1, lambda)`, `nu = 0.49`), the **plain element** at **`lambda = 0.93`**, below its zero
`0.9507` (two grid points of ADR 0628's sweep into the unstable side): the tools of this item must find the instability, and the
same tools find none above the zero (`lambda = 0.97`) and none for rule L at `0.93` (negative controls). The free tangent is
the sparse reduced tangent on the interior equations, `M = I` and `M = L_ff` (sparse); dense references: `symmetric_eigen` of
`K_ff`, `generalized_eigen_dense` of `(K_ff, L_ff)`.

## Readings, registered

- **The failing iterate** `K`: the last observed iterate of the failing attempt. "**The predictor's iterate**" if `K = 1`; "**a
  later iterate**" if `K >= 2`, with the residuals `r_0 .. r_K` read as **converging** (`r_k <= 0.5 r_{k-1}` for every `2 <= k <=
  K`), **diverging** (`r_K > r_1`) or **stalling** (neither).
- **CP, the candidate of ADR 0682** (the soft local pencil mode carried by the predictor past admissibility):
  - **confirmed**: `S_P >= 0.5` and the first element of the lowest free-tangent mode at `u_K` lies in the **corner
    neighbourhood** `N` (the 48 elements `(i, j, k)` with `i, j in 6..=9`, `k in 13..=15`, around node 4768 and containing 3720
    and 3959);
  - **refuted**: `S_P <= 0.1`;
  - **undetermined** otherwise (reported with `S_P`, the element and the form `H`).
- **JM, a jump of the force**: `TC > 1e-2` at some iterate `k < K` along `d_k`, or `max Delta a_e / Delta u_e > 10` at `u_K`:
  reported as "a jump of the element force at element ..." (the element of the largest ratio); otherwise "smooth".
- **c6, c7 on the same attempt**: `solve_increment` from the observed `u_c` (bit for bit the driver's) to `0.965625` with
  `line_search` (LS-A), `predictor_damping` (PD-A) and both (LP-A); each "solves the attempt" (an `Equilibrium`) or "fails" (its
  text). **c8**: `solve_with` of the `16^3` block (ten increments, the options of ADR 0682, the recorder) with `line_search`
  (LS-F), `predictor_damping` (PD-F) and both (LP-F): "solves to full load" (with `R`, `w_c`) or "fails" (the text, the last
  converged factor).

## Checks

Every measure is required finite before it is compared (a non-finite value or a missing step misses).

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| OB8 | c1 | `8^3` rule L, default: the solve with the recorder and the observer against the plain solve: `R`, `w_c` | bit-equal | D1 |
| BI8 | c1 | `8^3`: every observed iterate `u_{k+1}` of every attempt equals `replicate_step(u_k)` | bit-equal, every pair; at least 20 pairs | the driver's arithmetic |
| BX8 | c1 | `8^3`: `solve_increment` (default) from the observed start of the last attempt (`0.9 -> 1.0`): `u` against the observed last iterate; its residual count against the observed iterate count | bit-equal; equal | D2 |
| PD8 | c7, c10 | `8^3` rule L, ten increments, `predictor_damping`: `R`, `w_c` against ADR 0678 (`0.36659596`, `0.06631664`) | `<= 1e-6` relative each | the same equilibrium |
| LS8 | c6 | the same with `line_search` | `<= 1e-6` each | as PD8 |
| PD1 | c7 | `8^3` rule L, **one** increment, default and `predictor_damping`: outcome, `beta_0`; if the damped solve converges, `R`, `w_c` against PD8's | reported; `<= 1e-6` if it converges | |
| RF | c7 | `predictor_damping` with `unsymmetric` (`NewtonOptions` check), in `ArcLengthOptions`, with a contribution (`solve_with`), each refused naming `predictor_damping` | refused, each | D3 |
| NC-S | c2 | `8^3` at load factor 1 (observed converged state): pencil pairs; `share(phi_1)` and `share(phi_2)` with `phi = phi_1` | `>= 1 - 1e-10`; `<= 1e-8` | L-orthonormal Ritz vectors |
| TC8 | c1 | `8^3`: `TC` at the start of the last attempt along its observed predictor | `<= 1e-6` | D7 |
| PC | c9 | patch C49, plain, `lambda = 0.93`: inertia `nu`; `lowest_any` of `(K_ff, L_ff)` and of `(K_ff, I)`: `lambda_1 < 0`, against the dense values; SB; LC1; the driver's factor refuses | `nu >= 1`; `< 0`, `<= 1e-6` relative each; holds; `<= 1e-10`; refuses | ADR 0628: plain zero at `0.9507` |
| NC-P | c9 | the same at `lambda = 0.97` (plain) and at `0.93` (rule L) | `nu = 0`; `lambda_1 > 0`, `<= 1e-6` relative each; the factor passes | ADR 0628: `m_L >= 0.977` |
| OB16 | c1 | `16^3` rule L, default, the recorder and the observer: the error text | equal to ADR 0678's, character for character | D1 |
| BI16 | c1 | every observed `u_{k+1}` of the failing attempt equals `replicate_step(u_k)`; the driver's factor at `u_K` refuses with the pivot `-4.651826504388123e6` at node 4768 component 0 | bit-equal; the pivot's bits equal | D1 |
| BX16 | c1, c6 | `solve_increment` (default) from the observed `u_c` to `0.965625`: its error text contains the failure text of ADR 0678 (from `singular tangent`) | contains | D2 |
| I0 | c2 | at the observed `u_c`: `nu`, the driver's factor, `m_1` against ADR 0682's `6.286570e-2`, the first element of the pencil mode | `0`; passes; `<= 1e-4` relative; 3720 | the same state within the Newton tolerance |
| EI | c3, c4 | at every iterate `u_0 .. u_K`: the four pairs of `(K_ff, I)` and of the pencil by `lowest_any`: the largest residual; negatives among them `= min(nu, 4)`; SB when `nu >= 1` | `<= 1e-8`; equal; holds | ADR 0682 E1, E2 |
| LCI | c3, c4 | at every iterate, LC1 of ADR 0682 for both lowest modes | `<= 1e-10` | rounding |
| LOCI | c3, c4 | at every iterate: `nu`, the driver's factor, `lambda_1 / (mu h)`, `m_1`, the located set, first element and grid position, `s_E`, `H`, the stretch; KA; the residual `r_k`, `|d_k|_inf` and its node; `share(d_k)`, `share(u_k - u_c)`; D8 | finite (reported) | |
| TC0 | c1 | `TC` at `u_c` along `d_0` | `<= 1e-6` | D7 |
| TCI | c1 | `TC` at `u_k` along `d_k`, `1 <= k < K`, and at `u_K` along `d_{K-1}` | finite (reported; JM) | |
| SP | c2 | `S_P` | finite (reported; CP) | |
| SH | c5 | D9 at the listed elements and states | finite (reported; reading SH) | |
| OT | c1 | the other attempts from `0.95` and from `0.9625`: their iterate counts and residuals | finite (reported) | |
| LS-A, PD-A, LP-A | c6, c7 | D2 from `u_c` to `0.965625` | classified (reported) | |
| LS-F, PD-F, LP-F | c8 | the full `16^3` solves | classified (reported); if a solve passes: `|R / 0.32599497 - 1|` and `|w_c / 0.06060900 - 1|` (F-bar `16^3`, ADR 0678) `<= 0.15` each, and two passing solves agree to `1e-6` relative | the `8^3` gap of rule L to F-bar is `6.6` per cent |
| B6 | c10 | `si_eas_hg_default_bits`, `si_eas_hg3_bits`, `si_eas_default_bits`; `baseline_hashes` (rcfd-api) | pass | D3 |
| S6 | | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |
| S7 | | `cargo test -p rcfd-fem --release --lib`; `--test si_eas_hg10`, `si_eas_hg11`, `si_eas_hg12` (the brief), `sa_contact_lifted_f608`, `sa_contact_line_search_f684` (the line search touched) | pass | |
| S8 | | `cargo test -p rcfd-cli --release --test docs_site` | pass | |

### Negative and positive controls

| Id | Control | Expected |
|---|---|---|
| OB8, BI8, BX8 | the observer and the replica on a solve that passes | bit-equal |
| NC-S | the share of a mode in itself and of the next mode | `1`, `0` |
| NC-P | the plain element above its zero, rule L below it | no negative eigenvalue, the factor passes |
| PC | the plain element below its zero on the ADR 0628 patch | a negative eigenvalue found by the sparse tools, equal to the dense one; the driver's factor refuses |
| PD8, LS8 | the opt-ins on a solve that passes | the same equilibrium |

### Planned mutations (on a scratch copy of the test commit, each run once against the fast test `f767_fast`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `share` in the Euclidean inner product instead of `L` | NC-S (`share(phi_2)`) |
| MU2 | `lowest_any`'s shift `1.05 s_lo` instead of `1.05 s_hi` | PC (the shifted factor refuses, or the value differs) |
| MU3 | `replicate_step` without the prescribed increment at `k = 0` | BI8 |
| MU4 | `solve_increment` called with `lc = lt` | BX8 |
| MU5 | the damped predictor also scales the prescribed values (`lc v + beta (lt - lc) v`) | PD8, PD1 if some `beta < 1` occurs there (conditional, stated) |
| MU6 | the inertia counting positive pivots | PC, NC-P |
| MU7 | the observer adds `1e-9` to its force at one free equation | OB8 |

The heavy records are not run under the mutations (they use the same functions, checked by the fast test).

## Decision rule

- **done**: OB8, BI8, BX8, PD8, LS8, RF, NC-S, TC8, PC, NC-P, OB16, BI16, BX16, I0, EI, LCI, TC0, B6, S6, S7, S8 met in their
  single registered run, and the conditional limits of PD1 and LS-F, PD-F, LP-F where they apply; LOCI, TCI, SP, SH, OT and the
  classifications reported; every mutation caught (MU5 counted only if its condition holds); CP **confirmed or refuted**; no
  amendment.
- **partial**: B6 met, but a check missed, a mutation not caught, CP undetermined, or a check met only after an amendment.
- **todo**: B6 fails (a default bit changed), or OB16 or BI16 fails and no iterate of the failing attempt could be analysed.

c8 is answered whichever way the full solves end: a remedy that fails is a result, not a miss.

## Limits (not covered)

- One mesh, one material, one load; the failing attempt analysed in full, the other attempts by their residuals only.
- A remedy that changes rule L's coefficient is not in scope (ADR 0682 open question 2; the maintainer's call).
- The damping is checked on this block only; contact, the arc-length path, the time-stepping drivers and the unsymmetric
  factor are not run with it (the last two refuse it).
- The eigenvector of a repeated lowest eigenvalue is any vector of its space (reported when the two lowest are within `1e-6`
  relative, as ADR 0682).
- The corner sweep reads a homogeneous state taken from a non-homogeneous element (its centre gradient).
- Both opt-ins stay off by default.

## Open questions (taken or open)

1. **The design of the damping** (the ADR 0607 rule from the lifted iterate, with the pivot test in the admissibility; the
   refusal with contributions), **the indefinite eigen tool** (the bisection, `1.05 s_hi`), **the readings** (CP's `0.5`,
   `0.1` and the neighbourhood `N`; JM's `1e-2` and `10`; SH's `0.5`), **the new public function** `solve_increment`: taken on the
   maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. **Whether a remedy may change rule L's coefficient**: not taken; open for the maintainer.
3. **Whether the damping should become a default**: not taken (it would change default results); open.
