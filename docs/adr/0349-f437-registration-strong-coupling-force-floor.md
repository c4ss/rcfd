# ADR 0349: F-437 registration: a strong-coupling case whose force floor decides the convergence test

Status: **REGISTERED 2026-11-12**, written before any code of F-437 and before any run of its checks. Not yet accepted by
the maintainer. Before this text nothing was compiled and no test was run: the code and documents under "What was read"
were read, and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in the addendum at
the end; nothing above the addendum is changed after a run.

Parents: [ADR 0266](0266-f350-registration-strong-body-flow-coupling-case-key.md) (F-350: the optional table
`[dynamic_mesh.six_dof.coupling]`, its tree-against-library tests E1, E2, its mutation MU5 "force_scale not read", which
passed its named test E1, and its question Q1, from which this item comes), [ADR 0250](0250-f251-registration-strong-fluid-body-coupling.md)
(F-251: `strong_step`, the convergence test D7, the spring-mounted piston SPm, the sub-iteration gain C1),
[ADR 0346](0346-defaults-registration-wall-pressure-flip.md) (the default wall pressure of the body force is
`prescribed_gradient`; E1 and E2 compare the tree with `strong_step_with(PrescribedGradient)` and a carried gradient
vector). Roadmap item F-437 (depends on F-350).

Number 0349, branch `f437-strong-coupling-force-floor` from main 48ab0ba3, worktree `w49i-work/f437`, given by the lead.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here and not changed after a run; a miss becomes an ignored
`*_known_miss`; a defective set-up is corrected only by an amendment committed before the corrected run; each registered
run is made once; no existing test, hash table or golden is edited. This item is test-only: no library or driver line is
planned to change, so defaults are bit-identical by construction. Clean-room: no algorithm is added; the convergence test
is ADR 0250 D7's formula, read in `crates/rcfd-fv/src/fsi.rs`.

## What F-437 asks

The acceptance of `docs/roadmap.json`, binding in full:

> A tree-against-library equality test of the strong coupling whose force_scale decides the convergence test of every
> step (for instance force_scale = 100 k X on the piston of ADR 0266) is bit-identical, and fails when the driver does
> not read force_scale; registered before the run.

Clauses: (a1) a tree-against-library equality test, bit-identical; (a2) its `force_scale` decides the convergence test of
every step; (a3) it fails when the driver does not read `force_scale`; (a4) registered before the run (this file). The
lead's brief adds: derive a `force_scale` for which the floor binds in every step; register that the mutation "driver
ignores force_scale" makes the test fail (scratch copy); a negative control at a `force_scale` where the floor does not
bind gives equal results with and without the mutation; compare like with like (the default wall pressure is
`prescribed_gradient` since ADR 0346, so the library side is `strong_step_with(PrescribedGradient)` with a carried
vector).

## What was read (main 48ab0ba3, read only)

1. ADR 0266 in full (D1, D2, E1, E2, the mutations, the addendum: MU5 passed E1; E2 fails under it; Q1 (a)).
2. `crates/rcfd-api/tests/strong_coupling_case_f350.rs` in full: `piston_with`, `registered_table`, `tree_run`,
   `library_run` (since ADR 0346: `strong_step_with(.., WallPressure::PrescribedGradient, &mut carried, ..)` with
   `carried` empty before the first step), `Bits`, `first_difference`. E1 compares the bits after every step and the
   iteration counts; it does **not** compare the recorded test quantity.
3. `crates/rcfd-fv/src/fsi.rs`, `StrongCoupling`, `ratio` and `strong_step_with` (lines 97 to 433): the test quantity of
   iteration `k` is `e_k = max(ratio(r_F, F_t, force_scale), ratio(r_M, M_t, moment_scale))` with `ratio(r, v, s) =
   |r| / max(|v|, s)` (0 when `|r| = 0`), `r_F = F_t - F`, `r_M = M_t - M`, `F_t, M_t` the force and moment after the
   flow step of the iterate, `F, M` the (relaxed) values the body was advanced with; converged when `e_k <= tolerance`.
   `CouplingStep.residuals` holds every `e_k`, `force_residuals` every `r_F`; the moment is taken about the body's
   centre of mass `body.state.x`; the moment has its own relaxation sequence.
4. `crates/rcfd-api/src/dynmesh.rs`: the table's schema (`force_scale`, `moment_scale`, default 0), `from_case` builds
   `StrongCoupling { .., force_scale: get_f(case, &key("force_scale")), moment_scale: get_f(case, &key("moment_scale")),
   .. }` (lines 1100 to 1103), `step_strong` calls `strong_step_with` with the driver's `wall_pressure` (default
   `PrescribedGradient`) and its kept `wall_dpdn`; `CouplingRecord { iterations, converged, residual }`, where
   `residual` is the test quantity of the last iteration.
5. ADR 0346's summary and its addendum line: E1, E2 compare with `strong_step_with(PrescribedGradient)`.

## Arithmetic before this registration (disclosed)

**Why MU5 passed E1 (diagnosis by reading, no run).** The piston is the face `xmin` of `box_3d` with `y, z` in `[0, 0.2]`;
the body's centre of mass is at `(x, 0, 0)` (`position = [X, 0, 0]`, translation only). The face's resultant force is
along `x` (uniform pressure of the column; the sides are slip), applied at face centres whose offset from the centre of
mass is `(-X, y_f, z_f)` with the centroid of the face at `(.., 0.1, 0.1)`. So `M_t = r x F_t` with `r = (-X, 0.1, 0.1)`
and `F_t = (F, 0, 0)`: `M_t = (0, 0.1 F, -0.1 F)`, `|M_t| = 0.1414 |F_t|`, and the same linear map takes `r_F` to `r_M`
(the face centres do not move in `y, z`; the Aitken factor of a sequence is invariant under a fixed scaling, so the two
relaxation sequences agree). With E1's `moment_scale = 0` the moment term is `|r_M|/|M_t| = |r_F|/|F_t|`, the
**unfloored** force ratio, and `e_k = max(|r_F|/max(|F_t|, kX), |r_F|/|F_t|) = |r_F|/|F_t|` whatever the force floor:
the floor `k X` cannot change any `e_k` of E1 (up to rounding of the two ratios). That is ADR 0266's MU5 result, and it
holds for **any** `force_scale` while `moment_scale = 0`. A binding force floor needs a moment floor as well.

**The settings of the new case (F1).** SPm at `mu = 4` (`M = 0.01`, `M_A = rho A L0 = 0.04`, `k = (2 pi)^2 (M + M_A) =
1.9739208802178716`, `X = 0.01`, `k X = 0.019739208802178716` N), `T/100`, 60 steps, Aitken from
`relaxation_from_added_mass(M, M_A) = 1/3`, cap 2, `tolerance = 1e-8`, `max_iterations = 20`, `wall_pressure` not written
(the default `prescribed_gradient`), and

- `force_scale = s_F = 100 k X = 1.9739208802178716` N;
- `moment_scale = s_M = 100 k X x (1 m) = 1.9739208802178716` N m (the same number).

**The floor binds at every iteration of every step.** Bound on `|F_t|`: on the column the fluid's force is the added mass
times the wall acceleration, `|F_t| ~ M_A |a|`, and the body advanced with a held force `F` and the spring has `|a| <=
(|F| + k|x|)/M`. ADR 0250's linear model: the converged force is `F* ~ -(M_A/(M + M_A)) k x`, `|F*| <= 0.8 k X` (S3: `|x|
<= X`, no growth); with `e_k = F_k - F*` the flow's answer is `F_t,k - F* = -g e_k` with `1 + g = 2.952` (ADR 0250 C1),
and the factor `1/3` gives `e_{k+1} = (1 - 2.952/3) e_k = 0.016 e_k`. The first iterate has `F_1 = F(t_n)`: at step 1
`F_1 = 0` (fluid at rest), `|e_1| <= 0.8 k X`, `|F_t,1| <= 0.8 k X + 1.952 x 0.8 k X = 2.4 k X`; at later steps `|e_1| ~
omega dt |F*| = 0.063 x 0.8 k X`, so `|F_t,1| <= 0.9 k X`; later iterates are closer to `F*`. Estimate: `|F_t| <= 2.4 k X
= 0.024 s_F` at every iteration of every step, a margin of 40. Then `max(|F_t|, s_F) = s_F` and the force term is
`|r_F|/s_F`. The moment: `|M_t| = 0.1414 |F_t| <= 0.34 k X < s_M`, so the moment term is `|r_M|/s_M = 0.1414 |r_F|/s_F`,
below the force term by a factor 7 (even with the crude arm bound `|r| <= sqrt(0.01^2 + 0.2^2 + 0.2^2) = 0.283`, a factor
3.5). Hence at every iteration `e_k = |r_F|/s_F` **exactly** (the same division as `ratio`), the floor and not the force
sets the test: the test reads `|r_F| <= 1e-8 s_F = 1.97e-8` N, an effective relative tolerance of `1.97e-8/|F_t| >=
8.3e-7` against E1's `1e-8`.

**Why the mutation is caught (by construction, not by luck).** Under "the driver ignores force_scale" the tree's
`StrongCoupling` has `force_scale = 0` and its test quantity is `max(|r_F|/|F_t|, 0.1414 |r_F|/s_M) = |r_F|/|F_t|`,
which is at least `s_F/|F_t| >= 40` times the library's `|r_F|/s_F` wherever `r_F != 0`. At step 1 the first residual is
`r_F = F_t,1 - 0 != 0`. So the recorded test quantity of the last iteration of step 1 (`CouplingRecord.residual` against
`CouplingStep.residuals.last()`) differs, unless the last iteration of step 1 has `r_F = 0` exactly in both (both 0), which
the F1 run excludes by the record B2 below. The comparison of the test quantity is what E1 lacked; it catches the
mutation even where the iteration counts happen to agree. The iteration counts are expected to differ as well: with the
floor, iteration 2 needs `|r_F,2| <= 8.3e-7 |F_t|` at most (more at small force) against `1e-8 |F_t|` without; E1 took 3
iterations a step (4 at steps 1 and 25), so the floored case is predicted to take 2 to 3 a step and the mutated tree 3
to 4. Iteration 1 never passes: `|r_F,1| ~ 0.2 |F*|` gives `e_1 ~ 1.6e-3 >> 1e-8`, so the floored case is not the loose
step (no growth over 60 steps; the loose run of ADR 0266 L passes `10 X` only at step 28 with one iteration every step).

**The negative control (G).** The same case with `force_scale = s_F' = 1e-9 k X = 1.97e-11` N and `moment_scale = s_M`
unchanged. The floor `s_F'` binds only where `|F_t| < 1.97e-11` N, `1.2e-9` of the force amplitude `0.8 k X`; over 60
steps of `T/100` the force passes zero near `t = 0.25` and `0.75` where the step's sample has `|cos(omega t)| ~ 8e-3`
(`omega_m/omega = 1.005`, so `x(0.25) ~ -0.008 X`); the smallest `|F_t|` expected is about `1e-4` N, `5e6` times `s_F'`.
So `max(|F_t|, s_F') = |F_t| = max(|F_t|, 0)` and the control's every test quantity is the same number with and without
the mutation; the moment term is `0.1414 |r_F|/s_M`, far below `|r_F|/|F_t|`. Hence under the mutation the control's
tree run equals its library run bit for bit, as without it. This shows that F1 fails under the mutation because its
floor binds, not because the harness compares something else.

**Cost.** E1's 60 steps ran in a few seconds in release in ADR 0266's file; F1 and G are two tree runs and two library
runs of 60 steps each (below 30 s in release; they also run in debug, slower, no ignore).

## Design

Test-only. A new file `crates/rcfd-api/tests/strong_coupling_force_floor_f437.rs` with the SPm case text, the tree run and
the library run copied from `strong_coupling_case_f350.rs` (that file is not edited), extended to return the per-step
test quantity (tree: `CouplingRecord.residual`; library: `CouplingStep.residuals.last()`), and for the library also every
`residuals` and `force_residuals` entry and the last iterate's `force`. No library, driver, schema or document line
changes.

## Registered checks

All release, in the new file; each run once.

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| F1 (a1, a2) | the F1 case (`s_F = s_M = 100 k X`), 60 steps: tree against `strong_step_with(PrescribedGradient)` with a carried vector on the flow solver of a second `Simulation` of the same case: `u`, `p`, `phi`, body `x`, `v` after every step; iteration counts; the test quantity of the last iteration of every step (bits); every step converged; iterations `<= 20` | bit-identical; equal; equal bits; all; all | the same calls in the same order; 2 to 3 iterations |
| B1 (a2, the floor binds) | in F1's library run, at **every** iteration of every step: `e_k` equals `norm(r_F,k)/s_F` bit for bit | all iterations of all 60 steps | arithmetic: `|F_t| <= 0.024 s_F`, moment term 0.14 of the force term |
| B2 (a2, margin) | in F1's library run: the largest `norm(F + r_F)/s_F` at the last iterate of each step (the force of the flow's last answer, `F + r_F` recomputed); `norm(r_F)` at the last iterate of step 1 | `<= 0.1`; `> 0` | `0.024`; step 1 starts from `F = 0` |
| G (control) | the G case (`s_F' = 1e-9 k X`, `s_M = 100 k X`), 60 steps: as F1 (bits, iterations, test quantities, converged, `<= 20`) | bit-identical; equal; equal bits; all; all | as F1 |
| GB (control, the floor does not bind) | in G's library run: the smallest `norm(F + r_F)` at the last iterate of each step over `s_F'` | `>= 1e3` | `5e6` |

Printed, no limit: F1's and G's iteration counts (tree and library), the test quantities, the step of the smallest force.

### Mutations (scratch copy, not committed)

Each applied alone to `crates/rcfd-api/src/dynmesh.rs` of a `git archive` copy of the final tree in
`w49i-work/f437-scratch` by an exact text replacement that must occur once; the named tests are run once; the file is
restored and compared byte for byte with a kept copy.

| Id | Mutation | Must fail | Must pass |
| --- | --- | --- | --- |
| MF1 (a3) | the driver ignores `force_scale`: `force_scale: get_f(case, &key("force_scale")),` becomes `force_scale: 0.0,` | F1 | G (the negative control: equal with and without the mutation) |
| MF2 | the driver ignores `moment_scale` (0 always) | F1 (the moment term becomes the unfloored ratio, arithmetic above) | G is not registered under MF2 (its moment floor also matters there: the moment term would equal the unfloored force ratio, which is G's own force term; G is predicted to pass, printed only) |

## Predictions (mine, before any run)

F1 bit-identical, 2 to 3 iterations a step (fewer than E1's 3 to 4), all converged; B1 holds at every iteration; B2 about
`0.024` at step 1, `<= 0.01` after; G bit-identical with E1-like counts (3, 4 at step 1 and 25); GB about `5e6`. MF1:
F1 fails at step 1 (the test quantity, and the iteration counts); G passes. MF2: F1 fails. The check I trust least is B1's
exactness of the moment term's inferiority at the first iteration of step 1, where the predictor force is the default
extrapolation (the carried vector is empty): the arm is the same, so it should hold.

## Decision rule

- **done**: F1, B1, B2, G and GB are met, MF1 fails F1 and passes G, MF2 fails F1. The acceptance stays the maintainer's.
- **partial**: F1 or G is met but a mutation is not caught as registered (MF1 passes F1, or G fails under MF1), or B1,
  B2 or GB is missed (the floor's binding not shown as registered; kept as `*_known_miss`, diagnosed).
- **todo**: F1 is not bit-identical (the tree and the library differ with the default wall pressure; the cause recorded).

## Open choices taken

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none changes a default.

- **T1. A moment floor with the force floor.** The acceptance names only `force_scale = 100 k X`; by the arithmetic a
  force floor alone cannot decide the test on this case while `moment_scale = 0`, so `moment_scale = 100 k X x 1 m` is
  written too. Options: (a) both floors, the test compares the test quantity as well; (b) force floor only, which cannot
  meet (a2). Taken (a).
- **T2. A new test file**, the F-350 file unedited (house rule: no existing test edited).

## Limits of this registration

One geometry, translation only; the moment floor is exercised only as the companion of the force floor; no Newmark, no
rest patches, no extrapolated wall pressure in the new tests (ADR 0266's R1 and R2 keep it). Nothing here changes the
library's or the driver's lines; whether the driver should warn when a force floor is written with `moment_scale = 0` on
a body whose moment is proportional to its force (where the force floor is inert) is a question, not done here.

## Addendum 1: results (2026-11-12)

### What was built

- `crates/rcfd-api/tests/strong_coupling_force_floor_f437.rs` (new, 2 tests: F1 with B1, B2; G with GB). Test-only:
  no line of `rcfd-fv`, `dynmesh.rs`, `sim.rs`, the schema, the documents of the key, `baseline_hashes.rs`, the hash
  tables or the goldens changed, so every default is bit-identical by construction.

### Checks

Run once, `cargo test -p rcfd-api --release --test strong_coupling_force_floor_f437 -- --nocapture --test-threads=1`
(1.8 s), on commit 71951e16.

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| F1 (a1, a2) | bit-identical; equal; equal bits; all converged; `<= 20` | 0 of 60 steps differ in bits, iteration counts or the test quantity of the last iteration; 3 iterations every step (tree and library); all converged | met |
| B1 (a2) | `e_k = norm(r_F,k)/s_F` bit for bit at every iteration | 0 of 180 iterations differ | met |
| B2 | largest `norm(F + r_F)/s_F` `<= 0.1`; `norm(r_F)` of step 1's last iterate `> 0` | `8.83e-3` (step 2); `5.75e-10` N | met |
| G (control) | as F1 | 0 of 60 steps differ in bits, counts or test quantities; 4 iterations at step 1, 3 after; all converged | met |
| GB (control) | smallest `norm(F + r_F)/s_F'` `>= 1e3` | `1.56e7` (step 25) | met |

Against the predictions: F1's counts are 3 every step, not 2 to 3; the floor removes only step 1's fourth iteration
(G, the unfloored control, has 4 at step 1 and 3 after; ADR 0266 E1 had 4 at steps 1 and 25 with the old wall
pressure). So with this tolerance the floor sets *the value* of every test quantity (B1) and decides the outcome of
step 1's third test, not the iteration count of every step; the tree-against-library test catches an unread floor
through the test quantity it now compares, and through step 1's count and bits. B2 is `8.8e-3`, below the estimate
`0.024` (the bound `2.4 k X` at step 1 was loose: the largest `|F_t|` at a last iterate is `0.88 k X`, about the column's
`0.8 k X`). GB `1.56e7` against the estimate `5e6`.

### Mutations

Each applied alone to `crates/rcfd-api/src/dynmesh.rs` of a `git archive` copy of 71951e16 in
`w49i-work/f437-scratch/copy` by an exact replacement that had to occur once (`f437-scratch/mutate.py`), the new file's
two tests run once (release), the file restored and compared byte for byte with the kept copy and with the worktree's
file (equal both times).

| Id | Must fail | Must pass | Result |
| --- | --- | --- | --- |
| MF1 `force_scale: 0.0` in the driver (a3) | F1 | G | F1 failed at step 1 (bits, iterations 4 against 3, test quantity); G passed (the negative control: equal with and without the mutation) |
| MF2 `moment_scale: 0.0` in the driver | F1 | (G printed only) | F1 failed at step 1 (bits, counts, test quantity). G also failed, on the test quantity only (from step 1; bits and counts equal): the prediction that G passes under MF2 was wrong. With `moment_scale = 0` the moment term `norm(r_M)/norm(M_t)` is the unfloored force ratio up to rounding, and `max` picks it when its last bits are larger, so the recorded number differs while every decision is the same. This is the diagnosis of ADR 0266 MU5 seen directly; not a registered check of G |

### Deviations and disclosures

- No registered limit was missed; nothing was re-run. The MF2 outcome of G was a prediction, not a limit.
- The diagnosis of ADR 0266 MU5 in the arithmetic (by reading) was not run on ADR 0266's settings; MF2's G result
  supports it.

### Not run

The whole-workspace suite and the gate (the lead's). No existing test was run: no existing line changed. No run of the
binding floor over the full 11.5 periods (not registered; the 60-step test covers the code path).

### After the tests

`cargo fmt -p rcfd-api --check`: clean. `cargo clippy -p rcfd-api --all-targets --release -- -D warnings`: clean.

### Decision by the registered rule

F1, B1, B2, G and GB are met, MF1 fails F1 and passes G, MF2 fails F1: **done**. Every clause of the acceptance is met:
(a1) F1 bit-identical; (a2) the floor sets every test quantity of every step (B1: 180 of 180 iterations are
`|r_F|/s_F`, with `|F_t| <= 0.0088 s_F`); (a3) F1 fails when the driver does not read `force_scale` (MF1), while the
non-binding control does not; (a4) registered in commit 316417b7 before the test existed. The acceptance stays the
maintainer's.

### Open questions (each with options and a recommendation)

- **Q1. F-350's status.** ADR 0266 was partial only because MU5 passed its named test; F1 is now a named test that
  catches it. Options: (a) F-350 done with this item; (b) leave F-350 partial and count the closure here only.
  Recommendation (a).
- **Q2. A force floor with no moment floor is inert on a body whose moment is proportional to its force** (a force on a
  fixed arm from the centre of mass: the moment ratio equals the unfloored force ratio). Options: (a) a located warning
  when `force_scale > 0` and `moment_scale = 0`; (b) a sentence in `docs/case-schema.md` beside the keys; (c) leave.
  Recommendation (b) now (no default changes), (a) only if a case is seen to be confused by it.

### For the lead: roadmap entries

Proposed status of F-437: **done** (pending the maintainer's acceptance).

Note text for F-437:

> From ADR 0266 question 1, recommendation taken on the maintainer's standing instruction of 2026-11-11. DONE (ADR 0349, 2026-11-12; registered before the code, test-only, no library or driver line changed): crates/rcfd-api/tests/strong_coupling_force_floor_f437.rs runs the spring-mounted piston of ADR 0266 at mass ratio 4 through the case driver with force_scale = moment_scale = 100 k X and the default wall pressure (prescribed_gradient), against strong_step_with(PrescribedGradient) with a carried vector: 60 steps bit-identical in the flow, the body, the iteration counts and, new against ADR 0266 E1, the test quantity of every step; at all 180 iterations the test quantity is |r_F|/force_scale bit for bit (the largest force is 0.0088 of the floor). The driver mutated to ignore force_scale fails it at step 1 (bits, 4 iterations against 3, test quantity); the same case with force_scale = 1e-9 k X (the floor never binds; the smallest force is 1.6e7 times it) is equal with and without that mutation. Why ADR 0266 E1 could not catch it: with moment_scale = 0 the moment term of the test is the unfloored force ratio (the piston's moment is a fixed arm times its force), so a force floor alone is inert on that case; the moment floor is written with it here. Open: ADR 0349 Q1 (F-350's status), Q2 (a force floor without a moment floor).

Note text for F-350 (replacing its sentence that begins "Partial by the registered rule"):

> The one mutation of ADR 0266 that passed its named test (force_scale not read; the floor k X was inert in E1 because moment_scale = 0 makes the moment term the unfloored force ratio) is caught by the binding-floor test of ADR 0349 (F-437, 2026-11-12); proposed done by ADR 0349 Q1.

No follow-up items.
