# ADR 0719: F-780 registration: a driver retry with the indefinite tangent after a refused factor (an opt-in of the load-stepping Newton driver that repeats an increment refused by the positive pivot test with `NewtonOptions::indefinite_tangent`, and the inertia of every accepted state in the per-increment record; the ADR 0628 saddle patch, the 8^3 block and the 16^3 punch block of ADR 0694)

Status: **REGISTERED 2026-11-17, written before any code of F-780 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-780 was written and nothing of this item was run (no probe). The
numbers quoted below come from the published logs of ADR 0694 (`w49i-work/f774-scratch/run3-heavy-paths.log`,
`run4-fast-amended.log`), from ADR 0697, ADR 0700, ADR 0711 and ADR 0628 and from the ADR texts. Results go in the addenda at
the end; nothing above the addenda is changed after a run.

Roadmap item F-780 (deps F-774, F-779, F-783), the follow-up proposed by
[ADR 0694](0694-f774-registration-the-corner-element-of-rule-l-at-the-punch-corner-and-the-equilibrium-past-the-predictor.md)
(its remaining question 1, option (c)). Number 0719 given by the lead. Branch `f780-work` from 5c20a8df (ADRs 0694, 0697, 0700
and 0711 merged), worktree `w49i-work/f780`, scratch `w49i-work/f780-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would normally be asked about is
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-780 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. An opt-in driver retry with NewtonOptions::indefinite_tangent after a refused positive factor, reporting the inertia of every accepted state; the 16^3 block of ADR 0694 solves to full load with it; off by default, the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** an opt-in retry of the driver with `indefinite_tangent` after a refused
positive factor; **c2** the inertia of every accepted state reported; **c3** the `16^3` block of ADR 0694 solves to full load
with it; **c4** off by default, the default bits unchanged.

From the brief, also registered here: the field name, where the inertia is reported (a per-step record), and what happens when
the retry also fails; `baseline_hashes` run and the default path's bits shown; the change local to the load-stepping driver and
documented (F-797, the arc-length trace from a given state, may later touch the same driver); the heavy `16^3` run `#[ignore]`d
and run once, the fast tests on a small block where a positive factor is refused. Defaults and rule L's coefficient are not in
scope. Reruns by name: `si_eas_hg14`, `si_eas_hg15`, `si_eas_hg16`, `si_eas_hg17`, the driver's nonlinear and arc-length test
files, `--lib`, `baseline_hashes`, `docs_site`.

## What was read

- `AGENT_RULES.md`; the brief; ADR 0694 (D2, D3, D4, addendum 1 and its remaining question 1), the "For the lead" sections of
  ADRs 0697, 0700 and 0711; ADR 0697 D4 (PB: the patch's zero) and ADR 0628 table Z.
- `crates/rcfd-fem/src/nonlinear.rs`: `NewtonOptions` and its `check`, `LoadStep`, `NonlinearResult`, `Failure`, `classify`,
  `Problem::attempt`, `attempt_with`, `iterate`, `trial_slope`, `predictor_backtrack`, `factor_solve`,
  `factor_solve_indefinite`, `backtracked_failure`, `controlled_correction`, `solve`, `solve_with`, `solve_increment`,
  `solve_from_field_reusing`, `TimeSteppingOptions::check`; `crates/rcfd-fem/src/arclength.rs` (`ArcLengthOptions::check`);
  `crates/rcfd-fem/src/contact.rs` (`ContactOptions::check`, `solve_with`); `rcfd_linalg` `SkylineLdlt::inertia`,
  `SupernodalLdlt::inertia` (the count of negative pivots).
- `crates/rcfd-fem/tests/si_eas_hg14.rs` (the block, `solve_hooked`, `Recorder`, `inertia_at`, `patch`, `homogeneous_f`,
  `boundary_nodes`, the column, the fast test's RF2 and INC rows).

## What is known before the run (from the published logs)

1. **The default 16^3 path** (ADR 0694 `run3-heavy-paths.log` line 6): the observed attempts `0.7 -> 0.8 (2 iterates) x`,
   `0.9 -> 1.0 (3 iterates) x`, `0.95 -> 1.0 x`, `0.95 -> 0.975 x`, `0.95 -> 0.9625 x`, `0.9625 -> 0.96875 x`, `0.9625 ->
   0.965625 x` (the last fails after 5 halvings with ADR 0678's text, a refused pivot `-4.65e6` at node 4768). Every other
   attempt converged.
2. **IN-F** (`indefinite_tangent` on every factor, ten increments, the supernodal factor, four threads): ten attempts, every one
   converged at full steps, `R/mu 0.33937627`, `w_c +0.06245308`, 23.3 s, `nu = 1` at full load (the test's own inertia,
   `inertia_at`, of the symmetrised free tangent).
3. **INX** (ADR 0694 fast, `8^3`): on an attempt whose factors all pass the positive test, the indefinite factor gives the same
   iterates bit for bit. IN8 and the default `8^3` solve both give `R/mu 0.36659596`, `w_c 0.06631664`; the default `8^3` solve
   has no failed attempt (`run4-fast-amended.log` line 19).
4. **The stability of the 16^3 path** (ADR 0697): `nu` changes from 0 to 1 at `0.96490` and stays 1 to full load on the
   symmetric branch (ADR 0700: no nearby stable branch). The `8^3` path is stable at 0.9 and 1.0 (ADR 0697 NC8).
5. **The saddle patch** (ADR 0628 patch, `6^3`, interior nodes moved by `0.1 h (2U - 1)`, seed `0xF685_0614`, the plain
   incompatible element `Off`, `nu = 0.49`), confined compression `F = diag(1, 1, 1 - p)` with every boundary node prescribed:
   the homogeneous field is an equilibrium at every `p` (the patch test, ADR 0628 H3b); the inertia changes from 0 at `p_c =
   0.0492533` (ADR 0697 PB: the patch's zero 0.9507467 found to `6e-8`); at `p = 0.07` (`F_33 = 0.93`) `nu = 21` (ADR 0694 INC,
   by the test's `inertia` on the symmetrised supernodal factor). The default refuses at `0.93` (`singular tangent`), the
   indefinite iteration converges to the homogeneous field (`5.1e-15 h`). Rule L at the same state: `nu = 0` (ADR 0688 NC-P).
6. **The column** (ADR 0694 D4): two Full Hex8, one attempt from rest to `u_z = -1.2`: the full predictor inverts the lower
   element (`det F = -0.188`); the default fails with the located inversion, not with a refused factor.

No other number of this item is known before the run.

## Design

### D1. The retry (library, opt-in `NewtonOptions::indefinite_retry`; c1)

**Why a retry and not the indefinite tangent throughout.** ADR 0694 D2 derives that Newton's method needs a regular tangent, not
a definite one; the driver's positive test is a choice for the stable path. ADR 0694 remaining question 1 and ADR 0700 add why
the indefinite tangent should not be a silent default: on the `16^3` block it walks onto an unstable state with no nearby stable
branch. The retry keeps the positive factor first (so a stable path is solved exactly as by the default) and takes the
indefinite tangent only for an increment the positive test refuses, with the inertia of every accepted state in the record so
that a user sees `nu > 0` (c2).

**Rule.** `pub indefinite_retry: bool`, **default `false`**, builder `indefinite_retry(bool)`, read only by the load-stepping
driver ([`solve`], [`solve_with`]). With it, each attempt of the driver from the committed `(u_c, lambda_c, states_c)` to `lt`
is first made exactly as without it (the options as given, the positive pivot test). If that attempt fails **with a refused
factor** (the driver's own pivot refusal: a failure whose cause starts with `"singular tangent"`, the text of every pivot
refusal of `factor_solve`, positive or not, the test that ADR 0570 D2 uses for its fall-back), the same attempt (the same
committed state and target `lt`) is made once more with the same options and `indefinite_tangent = true` (ADR 0694 D2: every
factor of the attempt `L D L^T` with `NonZero` pivots, refused only at `|d| <= 1e-10 |a_ii|` or not finite). A failure of the
first attempt by anything else (a located inversion, a non-finite value, a non-convergence) is not retried. The retry is part of
the one attempt: the step-control hook sees one attempt (one `converged` or one `failed`), and the halving of the driver is
unchanged.

**What happens when the retry also fails (registered).** The attempt has failed: its failure is the retry's failure with
`"; retried with the indefinite tangent (ADR 0719 D1) after: {first cause}"` appended to its cause (the reason of a located
inversion, or the solver's text), and the driver halves the increment as without the opt-in (each halved attempt again first
positive, then retried if refused). After `max_halvings` halvings the error is the one of the default driver with that cause, so
both causes are seen.

**Why retry the same increment before halving.** A refused factor is a property of the tangent along the path (a lost
definiteness), not of the step length: in the default `16^3` path the halvings from 0.9625 end at the same refusal (known 1).
Retrying at the same increment first gives, on that block, the path of IN-F (D4, prediction P1).

**Combinations.** Refused (key `"indefinite_retry"`): with `indefinite_tangent` (every factor is indefinite already; nothing to
retry), with `unsymmetric` (the indefinite tangent is refused there, ADR 0694 D2), in `solve_with` with a contribution (the
inertia of D2 would need the contributions' tangents), and by every other entry point that takes `NewtonOptions` and does not
read it, so that the flag is never silently ignored: `ArcLengthOptions::check`, `solve_increment`, `solve_from` and its
variants (through `solve_from_field_reusing`, which also covers the transient driver and the total-Lagrangian contact body),
`TimeSteppingOptions::check` (`solve_in_time`), `ContactOptions::check`. Allowed with `line_search`, `predictor_damping`,
`predictor_backtracking`, `stagnation`, either factor, the step-control hook.

### D2. The inertia of every accepted state (library; c2)

**Where it is reported.** Two new fields of the per-increment record [`LoadStep`] (in `NonlinearResult::steps`, and given to
the hook's `converged`): `pub inertia: Option<usize>` and `pub retried: bool`. Without the opt-in: `None` and `false` (no
arithmetic is added). With it: `retried` is `true` when the increment was accepted by the retry; `inertia` is the inertia of the
accepted state.

**Definition (derivation).** The accepted state is the converged iterate `u*` of the attempt with the trial states the attempt
returns. Its inertia is the number of negative eigenvalues of the reduced tangent `K_ff(u*)` (the tangent of the driver at `u*`
from the committed states, `force_and_tangent` as in the attempt, reduced on the free equations by `reduce` with zero
increments). By Sylvester's law of inertia this is the number of negative pivots of any `L D L^T` of `K_ff` that exists (no
pivot zero). The driver's last factor of the attempt is at the iterate before `u*` and is not used: the record is of the state.
So the driver assembles the tangent at `u*` once more and factors it with the `NonZero` rule and the options' factor
(`SkylineLdlt` with reverse Cuthill-McKee for `Envelope`, `SupernodalLdlt::factor_with` with nested dissection and symmetry
tolerance `1e-12` for the supernodal factor, the factors of `factor_solve_indefinite`). `inertia = Some(count of negative
pivots)`; `Some(0)` with no free equation; **`None`** when the assembly fails or a pivot is refused (`|d| <= 1e-10 |a_ii|` or
not finite: a state numerically at a critical point, whose inertia the factor does not decide). Cost: one assembly and one
factor per accepted increment, only with the opt-in. With an increment accepted by the positive factor, `inertia` may still be
positive (the last factor was at the previous iterate); it is reported, not refused.

### D3. Not changed

The iteration (`Problem::iterate`), the factors, the halving, the error texts of the default path, the other entry points
(except the refusal of D1), the arc-length driver. The new code is in `solve_with` (the retry and the record) and two new
private functions (the inertia and the retry's failure note); `iterate` is not touched, so F-797 (the arc-length trace from a
given state) does not meet this change except at `ArcLengthOptions::check`'s one refusal.

### D4. The checks' cases and the predictions

- **The saddle path (SP).** The ADR 0628 patch with the plain element (`Off`), `solve` with every boundary node prescribed by
  the `Prescribed` list at the homogeneous field of `F = diag(1, 1, 0.93)` (scaled by the load factor: `p = 0.07 lambda`), ten
  increments, the options of `si_eas_hg14` (`options(10)`: four threads, the record of the active set, the supernodal factor).
  **Arithmetic (disclosed).** The homogeneous path `u_hom(lambda) = lambda (F - I) X` is linear in `lambda` and an equilibrium
  at every `lambda` (known 5), so `K(u_hom) d u_hom / d lambda = 0` on the free equations and the predictor from a homogeneous
  committed state lands on `u_hom(lt)` to rounding. Iteration 1 then has a residual at rounding but the energy test fails
  (`e_last = e_first`), so the driver factors the tangent at `u_hom(lt)`; it converges at iteration 2. The inertia changes at
  `lambda_c = 0.0492533 / 0.07 = 0.70362`. So, **predicted**: the increments to 0.1 .. 0.7 (`p <= 0.049`) are accepted by the
  positive factor with `nu = 0`; the attempt `0.7 -> 0.8` is refused at iteration 1. **SP-D (default)**: the halvings from 0.7
  (0.75, 0.725, 0.7125, 0.70625 refused; `0.703125`, `p = 0.04921875 < p_c`, accepted; then `0.70625` refused at 5 halvings):
  the default fails, `FemError::Solver` with `"load step 8 of 10"` and `"singular tangent"`. **SP-R (retry)**: ten increments,
  none halved; `retried` exactly at increments 8, 9, 10 (at 9 and 10 the committed state's tangent is indefinite, so the
  positive factor refuses at iteration 0); `inertia = Some(0)` at 1 .. 7, `Some(>= 1)` at 8, 9, `Some(21)` at 10 (known 5);
  `|u - u_hom|_inf / h <= 1e-6` at full load (known 5 measured `5.1e-15`); the test's own inertia (`inertia_at` of si_eas_hg14,
  the symmetrised supernodal factor) at the result equal to the record's. **SP-E**: SP-R with the envelope factor: the same
  `retried` and the same `inertia` at every increment (Sylvester: the count does not depend on the order), `|u - u_hom|_inf / h
  <= 1e-6`.
- **NC-L (negative control, no refusal).** The same path with rule L (`nu = 0` at 0.93, known 5; rule L's margin grows with the
  compression, ADR 0628 table Z, `m_Stab`): with the retry, no increment retried, `inertia = Some(0)` at all ten, `u` bit-equal
  to the default solve's.
- **RT8 (the 8^3 block, no refusal).** The `8^3` punch block of ADR 0694, rule L, `options(10)` with the retry, by
  `solve_hooked` (the hook path): ten attempts, all converged; `u` bit-equal to the default solve's (known 3: no attempt failed,
  so none is retried and the iterates are the default's); no increment retried; `inertia = Some(0)` at all ten (known 4; the
  lower loads are less compressed); `R`, `w_c` against `0.36659596`, `0.06631664` within `1e-6` relative.
- **DF0 (the default record).** The default `8^3` solve: `inertia = None`, `retried = false` at every increment.
- **NR (a failure that is not a refused factor is not retried).** The column (known 6), `solve` with one increment and
  `max_halvings = 0`: the default fails with the located inversion (`FemError::Element`, text with `"det F"`); with the retry the
  error is equal (`Display` text equal, no retry note).
- **RFF (the retry also fails).** The saddle patch, one increment to 0.93, `max_halvings = 0`, `max_iterations = 2`,
  `residual_tolerance = 1e-300` (positive and finite, so `check` accepts it; no residual meets it, so no attempt converges).
  **Arithmetic**: the positive attempt factors at rest (passes), lands on `u_hom(1)` (`nu = 21`), fails the convergence test at
  iteration 1 and is refused at its factor; the retry passes that factor and fails at iteration 2 = `max_iterations` with `not
  converged in 2 iterations`. Checks: the default fails with a cause `C` starting with `"singular tangent"` (the text after
  `"after 0 halvings: "`); the retry's error contains `"not converged in 2 iterations"` and ends with `"; retried with the
  indefinite tangent (ADR 0719 D1) after: " + C`.
- **RF (refusals).** `indefinite_retry` with `indefinite_tangent`; with `unsymmetric`; `solve_with` with a contribution (the
  observer of si_eas_hg14, which adds nothing); `trace` (`ArcLengthOptions`); `solve_increment`; `solve_from`; `solve_in_time`;
  `contact::solve_with` (no pairs: the options are checked first). Eight rows, each `FemError::InvalidValue` with key
  `"indefinite_retry"`.
- **The 16^3 block (heavy, c3).** The block of ADR 0694 (rule L, neo-Hookean, `16^3`, `u_z = -0.1 lambda` on the punch),
  `options(10)`, by `solve_hooked`: **OB16** the default solve fails with ADR 0678's text (`ADR0678_TEXT` of si_eas_hg14, equal);
  **IN16** `indefinite_tangent` (IN-F of ADR 0694 again): full load, `R/mu`, `w_c`; **RT16** the retry: **prediction P1**
  (derivation): each attempt of the retry path either passes the positive test at every factor, and then has the iterates of the
  indefinite attempt bit for bit (known 3), or is refused and retried, and then is the indefinite attempt; IN-F's attempts all
  converge (known 2); so the retry path is IN-F's, bit for bit, with ten attempts and no halving. Checks: full load; ten
  attempts, all converged (the hook); `u` at full load bit-equal to IN16's; `|R/mu / 0.33937627 - 1| <= 3e-8` and `|w_c /
  0.06245308 - 1| <= 1.6e-7` (twice the rounding of the eight printed decimals: `0.5e-8 / 0.339`, `0.5e-8 / 0.0625`);
  `retried` at increment 8 (`0.7 -> 0.8`: the default's attempt failed at its second iterate, an evaluation IN-F made without
  failing, so the failure was its factor) and at increment 10 (the state at 1.0 has `nu = 1` with a lowest eigenvalue far from
  zero, ADR 0711 lowest pair `-323` on the branch near it, so the positive factor near `u*` refuses); increment 9 reported;
  `inertia = Some(0)` at 0.1 .. 0.9 and `Some(1)` at 1.0 (known 4); the test's own inertia at the result `= 1` and equal to the
  record's. The run's time is reported (expected about 60 to 120 s: OB16 35 s, IN16 23 s, RT16 IN16's time plus ten assemblies
  and factors).

### D5. The test file `crates/rcfd-fem/tests/si_eas_hg18.rs`

Helpers copied from `si_eas_hg14.rs` (existing tests are not edited): `Checks`, the block and its sets, `options`, `measures`,
`ADR0678_TEXT`, `Recorder`, `solve_hooked` (returning also the records), `free_tangent`, `symmetrise`, `inertia`, `inertia_at`,
`Rng`, `rebuild`, `boundary_nodes`, `patch`, `homogeneous_f`, the column, `Observer`. Tests: `f780_fast` (default, every row of
D4 except the `16^3` ones) and `f780_nh16_retry_heavy` (`#[ignore]`, run once with `--ignored`).

## Checks

### The fast test `f780_fast` (default, not ignored)

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| RF | c1 | eight refusals | each key `"indefinite_retry"` | D1 |
| DF0 | c4 | default `8^3`: `inertia`, `retried` of every record | `None`, `false` | D2 |
| SP-D | c1 | default saddle path | fails: `Solver`, `"load step 8 of 10"`, `"singular tangent"` | D4 |
| SP-R | c1, c2 | retry: full load; records; halvings; `retried`; `inertia`; `|u - u_hom|_inf / h`; own inertia | 10; all 0; exactly {8, 9, 10}; `0` at 1..7, `>= 1` at 8, 9, `21` at 10; `<= 1e-6`; `=` record's | D4 |
| SP-E | c2 | the envelope factor | `retried`, `inertia` equal to SP-R's at every increment; `<= 1e-6` | Sylvester |
| NC-L | c1, c4 | rule L with the retry | none retried; `Some(0)` x 10; `u` bit-equal to default | known 5 |
| RT8 | c1, c4 | `8^3` with the retry (hook) | 10 attempts converged; bit-equal to default; none retried; `Some(0)` x 10; `R`, `w_c` `<= 1e-6` | known 3, 4 |
| NR | c1 | the column, one increment, no halving | default inverted (`"det F"`); retry text equal | known 6 |
| RFF | c1 | the retry also fails | default cause `C` starts `"singular tangent"`; retry text has `"not converged in 2 iterations"` and ends with the note + `C` | D4 |

### The heavy record `f780_nh16_retry_heavy` (ignored, run once)

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| OB16 | c4 | default `16^3` text | equal to `ADR0678_TEXT` | ADR 0678 |
| IN16 | | `indefinite_tangent` throughout | full load | known 2 |
| RT16 | c3 | the retry | full load; 10 attempts all converged; `u` bit-equal to IN16; `R/mu` `<= 3e-8`, `w_c` `<= 1.6e-7` relative to ADR 0694; retried at 8 and 10; `inertia` `Some(0)` at 1..9, `Some(1)` at 10; own inertia `= 1 =` record's | P1 |

### The default bits and the static checks

| Id | Check | Limit |
|---|---|---|
| B1 | `cargo test -p rcfd-api --release --test baseline_hashes` | passes |
| B2 | `si_eas_hg_default_bits`, `si_eas_hg3_bits`, `si_eas_default_bits` (release) | pass |
| S1 | `cargo test -p rcfd-fem --release --lib`; the fast tests of `si_eas_hg14`, `si_eas_hg15`, `si_eas_hg16`, `si_eas_hg17`; `sa_arclength`, `sa_contact_lifted_f608`, `sa_contact_line_search_f684` (the driver's other users with `solve_with`, the contact options and the arc-length options) | pass |
| S2 | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean |
| S3 | `docs_site` (`rcfd-cli`) | passes |
| S4 | every commit builds (`cargo build -p rcfd-fem --release --tests` on an archive copy of each) | builds |

### Negative and positive controls

| Control | What it shows |
|---|---|
| SP-D | the case needs the retry: the default refuses |
| NC-L, RT8 | with no refusal the opt-in leaves the path's bits as they are |
| NR | a failure that is not a refused factor is not retried |
| RFF | the retry's own failure, with both causes |
| OB16 | the default `16^3` path still refused with its text |
| DF0 | the default record has no inertia |

### Planned mutations (on a scratch copy of the test commit, each run once against `f780_fast`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the retry never fires | SP-R |
| MU2 | every failure retried (no `"singular tangent"` test) | NR |
| MU3 | the retry without `indefinite_tangent` (the positive factor again) | SP-R |
| MU4 | the inertia counts the positive pivots | SP-R, NC-L |
| MU5 | the inertia of the committed state before the increment | SP-R (increment 8 would read 0) |
| MU6 | `retried` never set | SP-R |
| MU7 | the retry's failure without the first cause | RFF |
| MU8 | the inertia recorded also without the opt-in | DF0 |

## Decision rule

- **done**: every row of `f780_fast` and of the heavy record met in its single registered run, B1, B2, S1 to S4 met, every
  mutation caught.
- **partial**: c3 met (RT16 reaches full load) and the default bits unchanged (B1, B2, OB16), but a row missed, an amendment was
  needed, or a mutation was not caught.
- **todo**: RT16 does not reach full load, or a default bit changed.

## Limits (not covered)

- The retry is in the load-stepping driver only; `solve_increment`, `solve_from`, the time stepping, the transient driver, the
  contact driver and the arc-length driver refuse the flag (D1) and do not report an inertia.
- A refused factor is recognised by its text (`"singular tangent"`), as ADR 0570 D2 does; a future change of that text would
  need this test changed with it.
- No contribution: the inertia would need the contributions' tangents (refused, D1).
- `inertia = None` at a state whose factor refuses a pivot: the record does not decide the inertia there.
- The inertia is of the symmetric `L D L^T` of the reduced tangent as assembled (symmetric to `1e-12`); the full unsymmetric
  tangent is refused (D1).
- No default changes; whether the retry should become a default is open question 1.

## Open questions (taken or open)

1. **Whether the retry becomes a default** (open, not taken: it would change default results). Recommendation: keep it off; a
   default that solves past a loss of stability should at least report the inertia in every user-facing output first (the CLI
   and the case file do not show `LoadStep` yet).
2. **Retry before or after the halvings** (taken on the maintainer's standing instruction of 2026-11-11, take the
   recommendation): before, at the same increment (D1), because the refusal is a property of the path, not of the step length.
3. **Whether `inertia` should be reported without the retry** (taken: no; with the opt-in only, so that the default path does no
   extra assembly and keeps its bits).

## Addendum 1: results (2026-11-17)

Logs in `w49i-work/f780-scratch`: the registered fast run `run1-fast.log` and the heavy record `run2-heavy.log` (both on
df2ae6d2, `run-commit.txt`), `fmt.log`, `clippy.log`, `b1-baseline.log`, `b2-bits.log`, `s1-lib.log`, `s1-tests.log`,
`s3-docs.log`, `s4-8a285601.log`, `mutations.log` and `mut-MU*.log`. Each registered test was run once; no limit changed after
a run; no amendment.

### The fast test (2.9 s)

| Id | Measured | Limit | Result |
|---|---|---|---|
| RF | eight refusals, each `InvalidValue` key `"indefinite_retry"` | key | met |
| DF0 | `inertia None`, `retried false` at all ten records | as registered | met |
| SP-D | fails: `load step 8 of 10 (load factor 7.03125e-1 to 7.0625e-1) after 5 halvings: singular tangent: pivot -1.0708e5 at node 228` (the halving chain of D4 exactly) | `Solver`, step 8, singular tangent | met |
| SP-R | full load, 10 records, none halved, 2 iterations each; retried at `[8, 9, 10]`; inertia `0` x 7, `5`, `11`, `21`; `|u - u_hom|_inf / h` 7.3e-15; own inertia 21 | as registered | met |
| SP-E | retried and inertia equal to SP-R's at every increment; 1.7e-15 | equal; `<= 1e-6` | met |
| NC-L | none retried; `Some(0)` x 10; `u` bit-equal to the default | as registered | met |
| RT8 | 10 attempts converged; bit-equal to the default; none retried; `Some(0)` x 10; `R` 2.3e-9, `w_c` 6.4e-8 | as registered; `<= 1e-6` | met |
| NR | default: `det F = -1.88e-1 at point 0` (inverted); retry: the same text | equal | met |
| RFF | default cause `singular tangent: pivot -7.2012e6 at node 106 ...`; retry: `not converged in 2 iterations (relative residual 2.9e-16, ...); retried with the indefinite tangent (ADR 0719 D1) after: ` + that cause | as registered | met |

### The heavy record (70.8 s)

| Id | Measured | Limit | Result |
|---|---|---|---|
| OB16 | ADR 0678's text, equal (24.2 s) | equal | met |
| IN16 | full load, `R/mu 0.33937627`, `w_c 0.06245308` (21.1 s) | full load | met |
| RT16 | full load (25.0 s); 10 attempts, all converged; `u` bit-equal to IN16's; `R/mu` 1.2e-8, `w_c` 1.8e-8 relative; retried at `[8, 10]` (9 not); inertia `0` at 0.1 .. 0.9, `1` at 1.0; own inertia 1 | as registered | met |

Prediction P1 held: the retry path is IN-F's bit for bit. The hook saw ten attempts; the default path's seven failed attempts
are replaced by two retries (increments 8 and 10). The extra assembly and factor per increment cost about 4 s on `16^3`.

### Default bits and static checks

| Id | Result |
|---|---|
| B1 | `baseline_hashes`: 3 pass, 1 ignored: met |
| B2 | `si_eas_hg_default_bits`, `si_eas_hg3_bits`, `si_eas_default_bits` pass: met |
| S1 | `--lib` 15 pass; the fast tests of `si_eas_hg14`, `si_eas_hg15`, `si_eas_hg16`, `si_eas_hg17` pass; `sa_arclength` 2, `sa_contact_lifted_f608` 7, `sa_contact_line_search_f684` 1 pass (release): met |
| S2 | `cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean: met |
| S3 | `docs_site` 2 pass (on df2ae6d2 and again on the results commit): met |
| S4 | `cargo build -p rcfd-fem --release --tests` passes at 8a285601 (archive copy) and at df2ae6d2 (the runs); 9167bd8d changes only the ADR: met |

### Mutations (each on a scratch copy of df2ae6d2, run once against `f780_fast`)

| Id | Caught by | Result |
|---|---|---|
| MU1 (the retry never fires) | SP-R, SP-E (the default's refusal at step 8), RFF | caught |
| MU2 (every failure retried) | NR (the inversion retried, its text carries the note) | caught |
| MU3 (the retry with the positive factor) | SP-R, SP-E, RFF | caught |
| MU4 (positive pivots counted) | SP-R (375 .. 354), NC-L, RT8 | caught |
| MU5 (the committed state's inertia) | SP-R (increment 8 reads 0) | caught |
| MU6 (`retried` never set) | SP-R | caught |
| MU7 (the first cause dropped) | RFF | caught |
| MU8 (the inertia also without the opt-in) | DF0 | caught |

### Deviations

None. The registration's D4 expected the inertia at increments 8 and 9 only as `>= 1`; the measured `5` and `11` are reported,
not checked.

### What was not run

The other users of `NewtonOptions` (the time stepping, the transient driver, the contact driver) were run only through their
refusal (RF) and the named contact files; the whole `rcfd-fem` integration suite was not run (rule 7; the daily CI gate runs it).

### Decision

**done**: every row of the fast test and of the heavy record met in its single registered run, B1, B2, S1 to S4 met, eight of
eight mutations caught, no amendment.

### Remaining questions

1. **Whether the retry becomes a default** (open question 1, open). Options: (a) keep it off; (b) make it the default with the
   inertia reported; (c) make it the default only for models with incompatible modes. Recommendation: (a), until the CLI, the
   case file and the result writers show `LoadStep::inertia` (now only in the library's record), since a default that solves
   past a loss of stability without showing `nu > 0` changes what a user reads as a stable result.
2. **Reporting the inertia outside the library** (CLI summary, result file). Recommendation: a follow-up that writes `inertia`
   and `retried` per increment where the solid case reports its increments, behind the same opt-in, without touching the hash
   tables.

## For the lead: roadmap entries

> Proposed status: done.

> F-780 (ADR 0719, 2026-11-17): the opt-in `NewtonOptions::indefinite_retry` (off by default) makes the load-stepping driver (`solve`, `solve_with`) repeat an increment whose attempt is refused by the positive pivot test once more, from the same committed state, with `indefinite_tangent`; a retry that also fails is the attempt's failure with both causes, and the driver then halves as before. Every accepted state's inertia (the negative pivots of the non-zero-pivot `L D L^T` of the reduced tangent at the converged iterate) and whether it was retried are in the new `LoadStep::inertia` and `LoadStep::retried`. The 16^3 punch block of ADR 0694 solves to full load with it in ten increments (R/mu 0.33937627, w_c 0.06245308, bit-equal to the indefinite-tangent solve; increments 8 and 10 retried; nu = 0 to 0.9 and nu = 1 at full load). Fast controls: the ADR 0628 saddle patch (the default refuses at step 8 exactly as derived; the retry reaches F_33 = 0.93 with nu 0, ..., 5, 11, 21, equal with both factors), rule L and the 8^3 block (no refusal: bits unchanged), an inversion not retried, a retry that fails. Every other entry point that takes NewtonOptions refuses the flag. Default bits unchanged (baseline_hashes, the default-bits tests); eight of eight mutations caught; no amendment.

> Follow-up proposed: The inertia and the retry of the nonlinear driver in the solid case's output. Deps F-780. Acceptance: Every limit not stated here is registered in the ADR before the run. With the solid case's opt-in for NewtonOptions::indefinite_retry, the inertia and the retry flag of every accepted increment written where the case reports its increments and shown by the CLI summary, with a warning when an accepted state has nu > 0; the 16^3 block of ADR 0694 run through the case file reports nu = 1 at full load; off by default, the default bits, the schema golden and the hash tables unchanged.

## Lead note at merge (2026-11-17)

The test file of D5 was written on a branch made before ADR 0718 (F-796) merged, and both chose the name
`crates/rcfd-fem/tests/si_eas_hg18.rs`. At the merge the lead kept ADR 0718's file under that name and moved this ADR's file,
unchanged in content, to `crates/rcfd-fem/tests/si_eas_hg19.rs`. Every test name,
check and result above refers to that file. Nothing above this heading was edited.

## Answers of the maintainer (2026-11-18), written into this ADR by the lead on 2026-11-19

The maintainer decided these on 2026-11-18 as sets of options with a recommendation; until now the decision stood in the roadmap note of the item only. Nothing above this section is changed.

- **F-842, Addendum 1, remaining question 2 (reporting the inertia outside the library).** Asked: `inertia` and `retried` per increment in the solid case's output, behind the opt-in. Answer: Approved under the class rule for additive golden edits (agent rules, rule 3, from version 2): an edit that only adds entries and removes or changes none is reported by the agent as an "additive golden edit" with its diff, re-recorded by the lead and written in the ledger line of the merge. `baseline_hashes.rs` is never in this class, and an edit that changes or removes an entry still needs the maintainer's word for that item. The item may be issued. The lead's reading: this ADR and the item's acceptance say that the schema golden and the hash tables stay unchanged, so the approval may not be needed; if an output golden gains entries, the class rule covers it.
