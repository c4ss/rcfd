# ADR 0724: F-843 registration: the fold of rule L's 16^3 punch branch by an extended system with warm-started local modes (at ADR 0723's fold bracket, a test-only Moore-Spence system on the warm model with committed element states: lambda_f, a_f, the critical vector's share on element 3720 and the null vector's residual)

Status: **REGISTERED 2026-11-17, written before any code of F-843 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no test code of F-843 was written and nothing of this item was run on the block (no probe). The
numbers quoted below come from the published log of ADR 0723 (`w49i-work/f841-scratch/run2-heavy.log`) and from the ADR
texts; the arithmetic on the toy was done beforehand in a scratch Python script (disclosed below). Results go in the addenda
at the end; nothing above the addenda is changed after a run.

Roadmap item F-843 (deps F-841), the follow-up proposed by
[ADR 0723](0723-f841-registration-the-subcritical-branch-of-rule-l-s-16-3-punch-path-past-amplitude-1-65e-4-with-warm-started-local-modes.md)
(its remaining question 2, option (a)). Number 0724 given by the lead. Branch `f843-work` from cea9dd0e (ADRs 0694, 0697,
0700, 0711, 0718, 0719 and 0723 merged), worktree `w49i-work/f843`, scratch `w49i-work/f843-scratch`. The block, the element,
the material and the branch are those of ADRs 0711, 0718 and 0723: the incompatible-modes Hex8 at finite strain with rule L
(`IncompatibleStabilisation::LinearStress`), the neo-Hookean material of ADR 0636 (`mu = 1e6`, `nu = 0.49`), the punch block
of ADR 0678 reading c1 on `16^3`, four threads, the supernodal factor, the model's opt-in warm start of the local mode
iteration (ADR 0660 D1, `SolidModel::set_incompatible_mode_warm_start(true)`) with the element states committed test-side as
in ADR 0723 D1.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would normally be asked about is
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-843 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. At ADR 0723's fold bracket, a test-only extended (Moore-Spence) system on the model with the warm start of the incompatible modes and committed element states, converged to a registered tolerance: lambda_f and a_f inside the bracket, the critical vector's share on element 3720, the null vector's residual; a fast control on a closed-form fold with a local unknown; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** at ADR 0723's fold bracket, a test-only extended (Moore-Spence) system on
the warm model with committed element states; **c2** converged to a registered tolerance; **c3** `lambda_f` and `a_f` inside
the bracket; **c4** the critical vector's share on element 3720; **c5** the null vector's residual; **c6** a fast control on a
closed-form fold with a local unknown; **c7** the default bits unchanged.

From the brief, also registered here: test-only (the extended system built in the test: a bordered solve with finite-difference
directional derivatives of the tangent, a registered step), no library change (another agent adds an arc-length opt-in in
`arclength.rs`, F-797, and another measures the local update options, F-844); heavy `16^3` runs capped, `#[ignore]`d and run
once, the fast toy always; rule L's coefficient and the defaults are not in scope. Reruns by name: `si_eas_hg20`,
`si_eas_hg18`, `si_eas_hg17`, `--lib`. The new test file is `crates/rcfd-fem/tests/si_eas_hg21.rs` (`hg19` and `hg20` are
taken).

## What was read

- `AGENT_RULES.md`; the brief; ADR 0723 (whole, and `f841-scratch/run2-heavy.log`); ADR 0711 D3 (the bordered Newton,
  `fold_between`); ADR 0660 D1 (the warm start).
- `crates/rcfd-fem/tests/si_eas_hg20.rs` (whole: the block, the observer, `FemCase`, `FemBranch`, `Bordered`, `bordered`,
  `amp_trace`, `fine_trace`, `commit_fem`, the toy `Pw`, the heavy record).
- `crates/rcfd-fem/src/nl_incompatible.rs`: `LOCAL_TOLERANCE = 1e-12` (the local test, relative to the mode scale),
  `converged` with its start, `mode_iteration_record_from`, `condensed_force_and_tangent_warm`.

**No library hook is needed.** As in ADR 0723: the warm evaluation reads the committed modes from the element states passed to
`internal_force` and `tangent`, both public; the commit is test-side (`commit_fem` of ADR 0723, copied). The directional
derivatives of the tangent are central differences of `tangent` at perturbed states. No library file is changed (check B);
`baseline_hashes` is not run.

## What is known before the run (from ADR 0723's log)

1. The bracket of ADR 0723 (its fine trace, `run2-heavy.log` lines 76 and 81): the lower end `t_8 = S5`, `a = 2.164988001e-4`
   (S5's `a = 2.164988000533e-4`), `lambda 0.958472223921`, `dlambda/da -5.382683e-1`, `nu 1`, the lowest pairs `-323.4006`,
   `2285.665`, `3057.084`, `3935.980`, share on element 3720 `0.5872`; the upper end (step 9, halved) `a = 2.204619895e-4`,
   `lambda 0.958478523962`, `dlambda/da +3.754511`, `nu 0`, the lowest pairs `+2108.03`, `2417.39`, share `0.3157`. Its
   target was `a(t_8) + (a_4 + 9 Da - a(t_8)) / 2` with `Da = (a_5 - a_4) / 8`.
2. ADR 0723 RS: the warm trace's state at `t_8` equals ADR 0711's rung 5 `S5` (plain, rebuilt) to `9.0e-12` in element 3720's
   modes and `1.2e-14` in `lambda`; the plain iteration from `alpha = 0` converges at `S5` in every element (S5 is a converged
   state of the plain model).
3. Cost (same log): rebuilding to `S5` about 50 s; a commit about 4 s; a bordered solve with its analysis 3 to 5 s (without the
   lowest pairs); the lowest pairs 3 to 12 s.

**Arithmetic beforehand (disclosed).** (i) `fold_between` of ADR 0711 on the printed bracket of item 1: the lower bound of
`lambda_f` `0.9584711481` (the tangent lines' intersection), the upper bound `min lambda = 0.958472223921`, the estimate `a
2.16996e-4`, `lambda 0.95847209`. (ii) In a scratch Python script (`f843-scratch/scripts/toy_ms.py`, the closed form and the
Newton of D2 only, no test code) on the toy PW of ADR 0723 D5 (`c = 2.84`, `d = 0.01`, the local unknown `z`), from the
closed-form branch states at `a = 0.90` and `0.95` with their roots committed, the step `h = 1e-8`: the closed-form fold
`a_f = 0.9415412906318721`, `p_f = 0.8928104857885694`, the null vector `(1, -a_f, 0, ...)` with the share `1 / (1 + a_f^2) =
0.5300821621831598` on `u_0`; `nu_r` (D2) at the starts `4.3e-3` and `9.5e-4`; with the tolerances of D4 the iteration stops
at iteration 4 from both ends, with `|a - a_f| = 3.4e-12` and `1.1e-14`, `|p - p_f| = 2.6e-14` and `7.8e-16`, the share to
`2e-12`. (iii) **The floor of the block elimination**: continued past convergence, the iteration from `0.90` loses the null
vector (`nu_r` rises from `4.3e-13` to `1e-2` within three more iterations, the normalisation to `4e-2`): at a fold computed
to rounding the matrix `K` of the four solves is singular to rounding, and `z_1`, `z_2` (D2) cancel. The stop rule of D4 (the
first iterate that meets all three tolerances) is therefore part of the method, and the tolerances sit above that floor.

## Design

### D1. The bracket on the warm model

The heavy record rebuilds ADR 0711's rungs 1 to 5 by the code of ADR 0723 D1 (copied: the observed `16^3` solve, the
symmetric chain, the switch, the step to 0.9645, `amp_trace` with 5 rungs) on the plain model, with ADR 0723's checks OB, R1
and S5 and their limits. **The lower end**: `S5` is committed on the warm model `M_W` (ADR 0723 D1; no modes yet: every
element from `alpha = 0`) to `S5_W`; by item 2 this is the state of ADR 0723's warm trace at `t_8`, and by ADR 0723 W0 the
warm evaluation at `S5_W` is the plain one bit for bit (check W5). This replaces the 8 warm steps from rung 4 (about 90 s) by
one commit. **The upper end**: one attempt of ADR 0711 D3 (the tangent predictor, the bordered Newton, the continuity test)
from `S5_W` to ADR 0723's halved target `a(S5) + (a_4 + 9 Da - a(S5)) / 2`, `Da = (a(S5) - a(S4)) / 8`, committed (check UB:
the bracket reproduced on this run). The extended system starts from the lower end only.

### D2. The extended system (c1)

Unknowns: the free displacement `u_F`, the load factor `lambda` (the prescribed values `lambda v`), the null vector `psi` (on
the free equations). Equations (Moore and Spence 1980, *The calculation of turning points of nonlinear equations*, SIAM J.
Numer. Anal. 17, cited from memory; the system and its elimination are derived here):

    R_F(u, lambda) = 0,    K(u, lambda) psi = 0,    l . psi = 1,

with `R_F` the free residual (`internal_force` on the free equations, evaluated with the committed states), `K` the
symmetrised reduced tangent of `FemBranch::system` (the tangent used by ADR 0711 D3 and for `nu` and `dlambda/da`), `f =
-dR_F/dlambda` its load vector, and `l = psi_0` the lowest-pair vector at the start (unit 2-norm, the sign of its largest
component positive). At a quadratic fold the system is regular (`psi . f != 0`, the second derivative along `psi` non-zero).

**Newton by block elimination with `K` (four solves, one factor).** With `G(d, t) = D_(u, lambda)[K psi] (d, t)`: `K y_1 =
-R`, `K y_2 = f` (so `du = y_1 + dl y_2`), `w_1 = G(y_1, 0)`, `w_2 = G(y_2, 1)`, `K z_1 = -w_1`, `K z_2 = -w_2`, `dl = (1 - l
. z_1) / (l . z_2)`, then `u_F += y_1 + dl y_2`, `lambda += dl`, `psi = z_1 + dl z_2` (from the second row, `K (psi + dpsi) =
-(w_1 + dl w_2)`, and the third). The factor is the supernodal `L D L^T` with the `NonZero` rule (`factor_nonzero`, as in
the bordered Newton); a refused factor or a non-finite `dl` is a failure.

**The directional derivatives** by central differences of the tangent: `G(d, t) = [K(u + e d, lambda + e t) psi - K(u - e d,
lambda - e t) psi] / (2 e)`, `e = h / |d|_2`, **`h = 1e-8`** (in the 2-norm of the free displacement, the units of the
amplitude `a`), the perturbed states with the prescribed values at the perturbed load (`at_load`) and the same committed
states. Estimate: on the block `mu_1` moves by `2.4e3` over `4e-6` of `a` in the bracket (scale `L ~ 1e-5`): truncation `(h /
L)^2 ~ 1e-6` relative; the evaluation noise of the local test (`1e-12` of the mode scale) over `2 h` about `1e-6` relative or
less. `G` only enters the Jacobian: its error slows the convergence, it does not move the solution. On the toy (scale 1) both
are below `1e-8`.

**The states within the iteration** are those of the start (committed), as within an increment of the driver of ADR 0660 D1.

### D3. The commit at the fold (c1)

After convergence the result `F` is committed (ADR 0723 D1: every element from its committed modes, from `alpha = 0` if that
fails) to `F_W`. Since every element's iteration from its own converged root passes the local test at its first iterate, the
evaluation at `F_W` is that at `F` bit for bit (check CF); no second cycle is needed, and none is run (a miss of CF is
recorded, not repaired by a second cycle).

### D4. The convergence (c2) and the measures (c3, c4, c5)

**The stop rule**: the first iterate `k >= 1` that meets the three tolerances is the result; at most **12 iterations**; a
failed evaluation, a refused factor or a non-finite value is a failure. Measures: the residual `m_R` (the bordered Newton's,
`|R_F|_2 / |f_int|_2` on the block, `max |r_i|` on the toy), the null residual **`nu_r = |K psi|_2 / (max_i |K_ii| |psi|_2)`**,
the normalisation `|l . psi - 1|`.

| | `m_R` | `nu_r` | `|l . psi - 1|` |
|---|---|---|---|
| the block | `<= 1e-10` (ADR 0711 D3) | `<= 1e-9` | `<= 1e-8` |
| the toy | `<= 1e-13` | `<= 1e-11` | `<= 1e-12` |

Estimates: on the block `max |K_ii|` about `1e7`, so `nu_r = 1e-9` puts the Rayleigh quotient within about `1e-2` of zero
against `mu_1 = -323` at `S5` and `d mu_1 / da` about `6e8`: `a_f` to about `2e-11`. The floor (iii) on the block is not
known; the local test's noise (`1e-12` relative) suggests a floor of `nu_r` near `1e-12`. On the toy (ii): the stop at
iteration 4 from both ends, the floor `4.3e-13`.

At the result: **`a_f`** `= phi . u_F` (the amplitude of ADR 0711 D3, `phi` the mode at 0.9647), **`lambda_f`**, **the share**
of `psi` on element 3720's free equations (`sum_{3720} psi_k^2 / |psi|^2`, ADR 0723's measure), `nu_r`; reported: the
iteration history (`m_R`, `nu_r`, `|l . psi - 1|`, `a`, `lambda` per iterate), the Rayleigh quotient `psi . K psi / psi .
psi`, `psi . f / (|psi| |f|)`, `nu_r` with the unsymmetrised reduced tangent (`nu_r^u`) and its asymmetry `max |K - K^T| / max
|K|`, the residual along `psi`, `|R_F(u + h psi^) - R_F(u - h psi^)|_2 / (2 h max |K_ii|)` (`psi^ = psi / |psi|_2`, a check of
the tangent by the residual, the noise about `1e-7`), element 3720's `K_aa` `l_min` at its committed modes, whether the plain
iteration from `alpha = 0` reaches element 3720's modes at `F`, the differences to ADR 0723's estimate.

### D5. The fast control (`f843_fast`): the toy PW (c6)

ADR 0723's toy PW (copied, unchanged: ADR 0711's PT with the local unknown `z`, `W_l = Phi(x) + (d/2) u_0^2 z`, `x = z - c
u_0^2`, `c = 2.84`, `d = 0.01`; the warm start with the commit): the closed-form fold `a_f` by bisection on `dp/da` (80
halvings, as ADR 0723), `p_f = p(a_f)`, `psi_f` parallel to `(1, -a_f, 0, ..., 0)` (rows 1 to 7 of the condensed `K` at `u_0
= a_f`: `2 a_f psi_0 + 2 psi_1 = 0`, `k_i psi_i = 0`), its share on `u_0` `1 / (1 + a_f^2)`, the committed root `z*(a_f) = c
a_f^2 - tan(d a_f^2 / 2)`.

- **The bracket**: ADR 0723's PW-W fine trace (from 0.5, `Da = 0.05`, the commit) to its first sign change of `sigma`: the
  states at `0.90` (`nu 1`) and `0.95` (`nu 0`), committed.
- **MS-L, MS-U**: the extended system of D2 from each end (`h = 1e-8`, the toy tolerances of D4), then the commit of D3.
- **NC1** (negative control, the local unknown): the extended system on the plain toy (no warm start) from the lower end: the
  local Newton from 0 fails above `a_c = 0.7005`, so the first evaluation fails with a local error.
- **NC2** (negative control, the measure): `nu_r` at the two bracket states, regular points, far above the tolerance.

## Checks

Every measure is required finite before it is compared (a non-finite value or a missing state misses).

### The fast test `f843_fast` (default, not ignored)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| TB | c6 | the PW-W trace: the sign change; the ends `0.90`, `0.95`; `nu` 1 then 0; `a_f` between them | holds | ADR 0723 run 1 |
| MS-L1 | c2, c6 | from the lower end: converged by D4 | holds; iterations `<= 6` | (ii): 4 |
| MS-L2 | c3, c6 | `|a - a_f|`; `|lambda - p_f|` | `<= 1e-10`; `<= 1e-12` | (ii): `3.4e-12`; `2.6e-14` |
| MS-L3 | c4, c6 | `|share_0 - 1 / (1 + a_f^2)|` | `<= 1e-9` | (ii): `2e-12` |
| MS-L4 | c5, c6 | `nu_r` at the result | `<= 1e-11` | the tolerance |
| MS-L5 | c1, c6 | the commit: fallbacks 0; `|z_committed - z*(a)|`; the residual after the commit bit-identical (CF) | `0`; `<= 1e-12`; holds | D3 |
| MS-U1 to MS-U5 | as MS-L | the same from the upper end | the same limits | (ii): 4; `1.1e-14`; `7.8e-16` |
| MS-LU | c3 | `|a_L - a_U|` | `<= 1e-10` | (ii) |
| NC1 | c6 | the extended system on the plain toy from the lower end | fails, the text starts with `local` | `a_c` |
| NC2 | c5 | `nu_r` at `0.90` and at `0.95` | `>= 1e-4` each | (ii): `4.3e-3`, `9.5e-4` |

### The heavy record `f843_nh16_fold_heavy` (ignored, run once)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| OB, R1, S5 | | as ADR 0723 | ADR 0723's limits | ADR 0723's run |
| W5 | c1 | `S5` committed on `M_W`: failed elements; CW (element 3720's committed state against `condensed_force_and_tangent_warm`); the free residual at `S5_W` against `S5`; `sigma`; `nu` | `0`; bit-identical; `0` entries differ; bit-identical; equal | ADR 0723 W0, CW |
| UB | c1 | the upper end from `S5_W`: accepted; `lambda` against `0.958478523962`; `a` against `2.204619895e-4`; `sigma` against `+3.754511`; `nu` | holds; `<= 1e-9`; `<= 1e-8` relative; `<= 1e-5` relative; `0` | ADR 0723 line 81 (from `t_8`, the same root to `9e-12`) |
| MS1 | c1, c2 | the extended system from `S5_W`: converged by D4 | holds (at most 12 iterations) | 4 to 6 |
| MS2 | c3 | `a_f` in ADR 0723's bracket `[2.164988e-4, 2.204620e-4]`; `lambda_f` in `[0.9584711481, 0.958472223921]` | inside, inside | (i): `2.170e-4`, `0.95847209` |
| MS3 | c5 | `nu_r` at the result; `m_R`; `nu_r^u` (unsymmetrised) | `<= 1e-9`; `<= 1e-10`; `<= 1e-8` | the tolerances; `K_aa` asymmetry `<= 1.7e-16` (ADR 0723) |
| MS4 | c4 | the share of `psi` on element 3720 | in `[0.30, 0.65]` | ADR 0723: `0.587` at `t_8`, `0.316` at the upper end, `0.645` to `0.587` from rung 4; at the fold the pair (`mu ~ 0`) stays separated from the next (`2285`): about `0.58` |
| LS | c5 | `K_f + 1000 I` inertia; `K_f - 1000 I` inertia | `0`; `1` | the critical value simple and lowest (next pair `2285`) |
| CF | c1 | the commit of `F`: failed elements; CW; the free residual at `F_W` against `F` | `0`; bit-identical; `0` entries differ | D3 |
| MSR | c3, c4, c5 | the history, the Rayleigh quotient, `psi . f`, the asymmetry, the residual along `psi`, `K_aa` `l_min`, the plain iteration from 0 at `F`, the differences to ADR 0723's estimate | reported | D4 |

### The default bits and the static checks

| Id | Clause | Measure | Limit |
|---|---|---|---|
| B | c7 | `git diff --stat cea9dd0e -- crates/*/src` empty at the tip | empty |
| S7 | | `cargo test -p rcfd-fem --release --lib`; `--test si_eas_hg20`, `si_eas_hg18`, `si_eas_hg17` (their default tests); `--test si_eas_hg21` (`f843_fast`) | pass |
| S6 | | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` | clean |
| S8 | | `cargo test -p rcfd-cli --release --test docs_site` (docs/ touched by this ADR) | pass |
| S9 | | every commit builds | builds |

`baseline_hashes` is not run: no shared solver file is touched (B checks it).

### Negative and positive controls

| Id | Control | Expected |
|---|---|---|
| NC1 | the extended system on the toy without the warm start | fails at the first evaluation, local |
| NC2 | `nu_r` at regular points (the bracket's ends) | `>= 1e-4`, far above the tolerance |
| MS-L, MS-U | the closed-form fold from both ends of the toy's bracket | the closed form to `1e-10` |
| W5 | the warm model at its own roots | the plain evaluation bit for bit |
| UB | the bracket's upper end against ADR 0723 | the same state |
| LS | the inertia on both sides of the critical value | `0` and `1` |

### Planned mutations (on a scratch copy of the test commit, each run once against `f843_fast`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `w_1` left out (the derivative of `K psi` along `y_1` set to 0) | MS-L1 or MS-U1 (the iterations, an inexact Jacobian) |
| MU2 | the load component of `G(y_2, 1)` left out (`G(y_2, 0)`) | MS-L1 or MS-U1 (the iterations) |
| MU3 | the share taken on `u_1` instead of `u_0` | MS-L3, MS-U3 |
| MU4 | the warm start off in the extended system (the plain toy) | MS-L1, MS-U1 (the first evaluation fails) |
| MU5 | the commit of the result from `z = 0` (the committed root ignored) | MS-L5, MS-U5 (the plain local Newton from 0 fails above `a_c`) |
| MU6 | the sign of `G` reversed | MS-L1, MS-U1 (no convergence) |

The heavy record is not run under the mutations.

## Decision rule

- **done**: every check of `f843_fast` and of the heavy record met in their single registered runs, B and S6 to S9 met, every
  mutation caught, no amendment.
- **partial**: B met and MS1 met, but another check missed, a mutation not caught, or a check met only after an amendment.
- **todo**: B fails, or MS1 fails (no fold computed), or OB fails and nothing is measured.

## Limits (not covered)

- One fold, one load path (the branch with `+phi_f`), one element watched; the other elements enter only through the commit's
  counts.
- The extended system uses the symmetrised tangent (the one of ADR 0711 D3); the unsymmetrised tangent enters only as the
  reported `nu_r^u`.
- The directional derivatives are finite differences with one registered step; their error is not measured beyond the
  convergence it allows.
- The block elimination is unstable once the fold is computed to rounding (iii); the stop rule handles that, a bordered
  solve with a non-singular extended matrix is not built.
- The lower end of the bracket is `S5` committed, the same state as ADR 0723's `t_8` by its RS (not rerun along the 8 warm
  steps).
- No library change; rule L's coefficient and the defaults are not in scope.

## Open questions (taken or open)

1. **The design** (test-only, the start at `S5` committed, the block elimination with central differences and `h = 1e-8`, the
   stop rule and tolerances, the measures, the toy PW and its bracket, the file `si_eas_hg21.rs`): taken on the maintainer's
   standing instruction of 2026-11-11 (take the recommendation).
2. **Whether the warm start becomes a default**: not taken (it would change default results); open (F-844 measures it).

## Addendum 1: results (2026-11-17)

Logs in `w49i-work/f843-scratch`: `run1-fast.log` (the registered fast run, on 4b5a2ef4), `run2-heavy.log` (the registered
heavy run, 63.9 s, on 4b5a2ef4), `mutations.log` and `mut-MU*.log`, `s7-*.log`, `b-src-diff.log`, `s6-*.log`,
`s8-docs.log`; the arithmetic beforehand `scripts/toy_ms.py`. Each registered test was run once; no limit changed after a
run; no amendment.

### The fast test (`f843_fast`, run 1)

The closed form: `a_f = 0.9415412906318720`, `p_f = 0.8928104857885693`, the share `1 / (1 + a_f^2) = 0.5300821621831598`.

| Id | Measured | Limit | Result |
|---|---|---|---|
| TB | the sign change between `a = 0.900000000000` (`nu 1`) and `0.950000000000` (`nu 0`), `a_f` between | holds | met |
| MS-L1 | converged in 4 iterations | holds; `<= 6` | met |
| MS-L2 | `2.7e-11`; `3.3e-15` | `<= 1e-10`; `<= 1e-12` | met |
| MS-L3 | `1.4e-11` | `<= 1e-9` | met |
| MS-L4 | `4.2e-12` | `<= 1e-11` | met |
| MS-L5 | 0 fallbacks; `0`; bit-identical | `0`; `<= 1e-12`; holds | met |
| MS-U1 | converged in 4 iterations | holds; `<= 6` | met |
| MS-U2 | `8.1e-14`; `7.8e-16` | `<= 1e-10`; `<= 1e-12` | met |
| MS-U3 | `4.2e-14` | `<= 1e-9` | met |
| MS-U4 | `1.2e-14` | `<= 1e-11` | met |
| MS-U5 | 0 fallbacks; `0`; bit-identical | `0`; `<= 1e-12`; holds | met |
| MS-LU | `2.7e-11` | `<= 1e-10` | met |
| NC1 | fails: `local: not finite at iteration 10` | fails, local | met |
| NC2 | `4.3e-3` at 0.90, `9.5e-4` at 0.95 | `>= 1e-4` each | met |

### The heavy record (`f843_nh16_fold_heavy`, 16^3, 63.9 s)

| Id | Measured | Limit | Result |
|---|---|---|---|
| OB, R1, S5 | equal; `9.95e-11`, `nu = 1`; rungs 1 to 5, `2.3e-13`, `2.2e-10`, `9.0e-8`, `mu_1` bit-identical (`0`) | ADR 0723's limits | met |
| W5 | 0 failed; CW bit-identical; the free residual: 0 entries differ; `sigma` bit-identical (`-5.382683486833375e-1`); `nu` 1 and 1 | as registered | met |
| UB | accepted (3 iterations); `5.6e-14`; `2.1e-10`; `8.1e-8`; `nu 0` | holds; `<= 1e-9`; `<= 1e-8`; `<= 1e-5`; `0` | met |
| MS1 | converged in **2 iterations** (`nu_r` `4.3e-5`, `7.8e-7`, `3.6e-11`) | holds, at most 12 | met |
| MS2 | **`a_f = 2.170071672061e-4`** in `[2.164988e-4, 2.204620e-4]`; **`lambda_f = 0.958472086948445`** in `[0.9584711481, 0.958472223921]` | inside, inside | met |
| MS3 | `nu_r = 3.6e-11`; `m_R = 3.4e-13`; `nu_r^u = 3.6e-11` | `<= 1e-9`; `<= 1e-10`; `<= 1e-8` | met |
| MS4 | **share on element 3720 `0.5856`** | in `[0.30, 0.65]` | met |
| LS | inertia `0` of `K_f + 1000 I`, `1` of `K_f - 1000 I` | `0`; `1` | met |
| CF | 0 failed (0 fallbacks); CW bit-identical; the free residual at `F_W`: bit-identical | as registered | met |
| MSR | below | reported | reported |

**The fold (MSR).** The extended system started at `S5_W` (`nu_r = 4.3e-5`, the share of `psi_0` `0.5872`) and converged in
two iterations (`m_R` `3.8e-9` then `3.4e-13`; `|l . psi - 1|` `5.5e-10` at the result, within its tolerance). At the fold:
the Rayleigh quotient `psi . K psi / psi . psi = -2.6e-5` (against `mu_1 = -323.4` at `S5` and the next pair `2285`), `max
|K_ii| = 7.50e6`, `psi . f / (|psi| |f|) = +1.96e-4` (non-zero: the fold is transversal in the load), the unsymmetrised
tangent's asymmetry `1.2e-16` and its `nu_r` equal to the symmetrised one's to the printed digits, the residual along `psi`
(the central difference of `R_F`) `6.8e-9` of `max |K_ii|` (the tangent and the residual agree to the noise estimated in
D4). Element 3720 at the committed fold: `K_aa` `l_min = 3.763e4` (positive definite, as along ADR 0723's trace), and the
plain iteration from `alpha = 0` reaches the same modes (`3.0e-11`). Against ADR 0723's `fold_between` estimate (`a
2.16996e-4`, `lambda 0.95847209`): `a_f` higher by `5.1e-5` relative, `lambda_f` lower by `3.1e-9`. The fold lies 0.23 per
cent of `a_5` past rung 5 and `1.4e-7` in `lambda` below it.

### Default bits and static checks

| Id | Result |
|---|---|
| B | `git diff --stat cea9dd0e -- crates/*/src` empty: met |
| S7 | all release: `--lib` 15 pass; `si_eas_hg20` 1 pass, 1 ignored; `si_eas_hg18` 1 pass, 2 ignored; `si_eas_hg17` 1 pass, 1 ignored; `si_eas_hg21` 1 pass (`f843_fast`), 1 ignored: met |
| S6 | `cargo fmt -p rcfd-fem --check` and `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` clean on the results tree: met |
| S8 | `cargo test -p rcfd-cli --release --test docs_site` on the results tree (2 pass): met |
| S9 | c75f4968 (docs), 4b5a2ef4 (built for the runs), the results commit (its only code change is the heavy record's ignore text, checked by S6): met |

### Mutations (each on a scratch copy of 4b5a2ef4, run once against `f843_fast`)

| Id | Caught by | Result |
|---|---|---|
| MU1 | MS-L1, MS-U1 (no convergence: the iterates leave the warm start's region, `local: not finite`), MS-LU | caught |
| MU2 | MS-L1 (the factor refused at iteration 7), MS-LU | caught |
| MU3 | MS-L3, MS-U3 (`6.0e-2`) | caught |
| MU4 | MS-L1, MS-U1 (`local: not finite`), MS-LU | caught |
| MU5 | MS-L5, MS-U5 (`z` NaN, the residual not bit-identical) | caught |
| MU6 | MS-L1, MS-U1 (`local: not finite`), MS-LU | caught |

### Deviations

- None in the registered procedure. The copied `pw_trace` of ADR 0723 also checks ADR 0723's PW-0 (the start at 0.5 within
  `1e-12`) with ADR 0723's limit: met; it is part of TB's procedure, not a new limit.
- The heavy run took 63.9 s, below the estimate of D1 (about 115 s): two extended iterations instead of four to six, at about
  1.2 s each.
- MS-L2's `|a - a_f| = 2.7e-11` is above the script's `3.4e-12` (a different rounding of the same last step); within the
  limit.

### Decision

**done** (by the registered rule). Every check of `f843_fast` and of the heavy record was met in their single registered
runs, B and S6 to S9 were met, six of six mutations were caught, no amendment.

### Remaining questions

1. **Whether the warm start becomes a default**: open (it would change default results); F-844 measures it.
2. **The library's arc-length driver at this fold.** The fold is now located to `nu_r = 3.6e-11`. Options: (a) once F-797's
   arc-length opt-in is merged, a registered check that it passes this fold with the warm start and brackets `a_f` and
   `lambda_f` of this ADR; (b) leave it. Recommendation: (a), proposed below.

## For the lead: roadmap entries

> Proposed status: done.

> F-843 (ADR 0724, 2026-11-17): the fold of rule L's 16^3 punch branch computed at ADR 0723's bracket by a test-only Moore-Spence system (R_F = 0, K psi = 0, l . psi = 1), Newton by block elimination with the tangent's factor and central differences of the tangent (step 1e-8), on the model with the warm start of the incompatible modes and committed element states. ADR 0711's rung 5 is rebuilt and committed on the warm model (bit-identical to the plain evaluation), and the bracket's upper end is reproduced (lambda to 6e-14). From rung 5 the system converges in 2 iterations to a_f 2.170072e-4, lambda_f 0.958472086948, both inside the bracket; the null vector's residual |K psi| / (max |K_ii| |psi|) is 3.6e-11 (registered tolerance 1e-9), the Rayleigh quotient -2.6e-5 against -323 at rung 5, the critical value simple and lowest (inertia 0 and 1 at +1000 and -1000), and the critical vector's share on element 3720 is 0.586. The fold is 0.23 per cent of a_5 past rung 5; ADR 0723's two-point estimate was within 5e-5 relative in a and 3e-9 in lambda. Committing the fold state changes the residual by no bit, and element 3720's mode tangent stays positive definite there. Fast control: ADR 0723's toy with an atan local unknown, the closed-form fold reproduced from both ends of its bracket (a to 3e-11, lambda to 3e-15, the null vector's share to 1e-11); the same system on the toy without the warm start fails at its first evaluation. Default bits unchanged (no library line); six of six mutations caught; no amendment.

> Follow-up proposed: The library arc-length driver through rule L's 16^3 fold with warm-started local modes. Deps F-797. Acceptance: Every limit not stated here is registered in the ADR before the run. From ADR 0711's rung 5, the arc-length opt-in of F-797 on the model with the warm start of the incompatible modes passes the fold computed by ADR 0724 and reads it: a sign change of dlambda/da with nu from 1 to 0 whose bracket contains a_f 2.170072e-4 and lambda_f 0.958472086948, the steps and the local fallbacks, to registered limits; a fast control on a closed-form fold with a local unknown; the default bits unchanged.
