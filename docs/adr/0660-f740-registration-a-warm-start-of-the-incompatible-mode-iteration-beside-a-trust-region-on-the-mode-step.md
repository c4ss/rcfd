# ADR 0660: F-740 registration: a warm start of the incompatible-mode iteration from the committed modes as the registered remedy, a trust-region (Levenberg-Marquardt) mode step measured beside it, on the eleven set-ups of ADR 0658 and three more seeds of the 8^3 block at 0.25 h

Status: **REGISTERED 2026-11-16, written before any code of F-740 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-740 was written, compiled or run: the files listed under "What was
read" and the logs of ADR 0658 (`w49i-work/f739-scratch/run1-default.log`, `a1-diagnosis.log`) were read, and the arithmetic
below was done by hand (disclosed). No probe was run for this item. Results go in the addenda at the end; nothing above the
addenda is changed after a run.

Roadmap item F-740 (deps F-739), the follow-up proposed by
[ADR 0658](0658-f739-registration-the-line-search-with-the-backtracking-of-the-incompatible-mode-update-on-distorted-meshes.md)
("For the lead", follow-up proposed; its remaining question 1, option (a) measured beside (b)). The element is the
incompatible-modes Hex8 at finite strain of [ADR 0608](0608-f596-registration-incompatible-modes-hex8-at-finite-strain.md)
with the stabilisation of ADR 0614 D1 and the rule `IncompatibleStabilisation::LinearStress` of ADR 0630 D1 (rule L); the
reference is the F-bar Hex8 of ADR 0506; the meshes are those of
[ADR 0649](0649-f719-registration-the-stabilised-incompatible-hex8-under-load-on-distorted-meshes.md) D2 and the seeds of ADR
0658 reading c3. Number 0660 given by the lead. Branch `f740-work` from main 14b155cd (F-732 / ADR 0655 and F-739 / ADR 0658
merged), worktree `w49i-work/f740`, scratch `w49i-work/f740-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical (the new behaviour opt-in); no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden
file; no existing test edited; no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that
the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation), except the open questions at the end.

## What F-740 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. Rule L on the eleven set-ups of ADR 0658 (the eight distorted meshes of ADR 0649 and the 8^3 block at 0.25 h with seeds 0xF739_0001 to _0003) and three more registered seeds: an opt-in warm start of the local mode iteration from the modes of the committed state (element state added only behind the opt-in), and a trust-region (Levenberg-Marquardt) mode step measured beside it on the same set-ups; punch reaction, corner bulge, tip reaction and shortening within the ADR 0636 tolerances of the F-bar Hex8 and the loaded state positive definite for the registered remedy; the mutations of the new update caught by the item's own checks; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** the eleven set-ups of ADR 0658 (the eight meshes of ADR 0649 D2 and the
`8^3` block at `0.25 h` with seeds `0xF739_0001` to `_0003`); **c2** three more registered seeds; **c3** an opt-in warm start
of the local mode iteration from the modes of the committed state, element state added only behind the opt-in; **c4** a
trust-region (Levenberg-Marquardt) mode step measured beside it on the same fourteen set-ups; **c5** punch reaction and corner
bulge (block), tip reaction and shortening (cantilever) of the registered remedy within the ADR 0636 tolerances of the F-bar
Hex8 on the same mesh; **c6** the loaded state of the registered remedy positive definite; **c7** the mutations of the new
update caught by this item's own checks; **c8** the default bits unchanged. The brief adds: everything opt-in;
`baseline_hashes` (`nonlinear.rs` is a shared solver file); the named existing tests rerun; negative controls and mutations
registered before the run; a run over about two minutes in release an ignored heavy record with a fast test on the same path;
Levenberg 1944, Marquardt 1963 and Nocedal and Wright cited from memory, said so, and what is used derived.

### Readings, registered

- **c1, c2: the fourteen set-ups.** The eight meshes exactly as ADR 0658 reading c2 (ADR 0649 D2: SplitMix64, seeds
  `0xF719_0004` block `4^3`, `0xF719_0008` block `8^3`, `0xF719_1010` cantilever `10 x 2 x 2`, `0xF719_1020` cantilever `20 x
  4 x 4`, three draws per interior node in node order, every component moved by `a h (2U - 1)`, `a = 0.1` and `0.25`); the three
  seeds of ADR 0658 (`0xF739_0001` to `_0003`, all three valid there, M0 `0.0680`, `0.0957`, `0.1788`); **the three more seeds**
  `0xF740_0001`, `0xF740_0002`, `0xF740_0003` on the `8^3` block at `a = 0.25` with the generator of c1, and the rule of ADR
  0658 reading c3: a seed whose mesh fails M0 is replaced by the next (`0xF740_0004`, ...) until three valid meshes are found,
  the replacement printed and recorded; fixed now, before any mesh of these seeds was built; no seed is chosen by a solve
  result. Load cases, measures and material as ADR 0658 reading c2 (block: confined punch `x, y <= 0.5` on `zmax` at `u_z =
  -0.1`, `R = -sum_punch r_z / mu`, `w_c = u_z(1, 1, 1)`; cantilever `[0, 1] x [0, 0.2]^2`, `xmin` clamped, `xmax` `u_y = 0.3`,
  `P = sum_tip r_y / mu`, `d = -mean_tip u_x`; `neo_hookean(mu = 1e6, kappa(nu = 0.49))`; 10 increments, 4 threads). The
  helpers of `si_eas_hg7.rs` are copied (existing tests are not edited).
- **c3: the registered remedy W.** Rule L with `SolidModel::set_incompatible_mode_warm_start(true)` (D1),
  `set_incompatible_mode_backtracking(true)` and `set_incompatible_mode_decrease(true)` (the local update S of ADR 0658 D1),
  **without** the driver's line search, `NewtonOptions::new(10).threads(4)`. Why S's update and not R's: ADR 0658 Addendum 1
  found S passing every set-up R passed, more simply (DB4 in 10 increments without halving), and recommended it (its remaining
  question 3); the warm start changes only where the local iteration starts, so W differs from S in nothing else, and S is the
  control that isolates the warm start (NCS2 below).
- **c3: "the modes of the committed state", "element state added only behind the opt-in".** With the opt-in, the trial state
  an Incompatible Hex8 returns to the driver is its material state followed by its nine converged modes (D1); the driver commits
  the trial states of the converged iterate of an attempt and keeps the committed ones on a failed attempt (ADR 0475 D3,
  `nonlinear.rs` `attempt_with`, unchanged), so the modes in the committed state are those of the element at the committed
  displacement. Without the opt-in the element returns its material state alone (nothing added), as today.
- **c4: the variant T, "measured beside it".** Rule L with the opt-in trust-region mode step (D2,
  `set_incompatible_mode_trust_region(true)`) from `alpha = 0` (no warm start), without the line search, on the same fourteen
  set-ups. A cold start isolates the question ADR 0658 remaining question 1 (b) asked: does a trust region on the mode step alone
  handle the indefinite, near-singular mode block of ADR 0658 E2. T is measured: per set-up its outcome, its two errors against
  F-bar and SL are printed and set against W's limits, "passes" recorded; no limit binds T's values. What binds are the checks of
  D2's arithmetic (TR1, TR2) and the control NCT.
- **c5: the tolerances.** ADR 0636 (and ADR 0649 reading c4): `0.15` on the coarse meshes (block `4^3`, cantilever `10 x 2 x
  2`), `0.10` on the refined ones (block `8^3`, cantilever `20 x 4 x 4`, the six seeds), for each of `e_R = |R / R_Fbar - 1|`,
  `e_w`, `e_P`, `e_d`; F-bar solved on the same mesh in the same test with the driver's defaults.
- **c6: positive definite.** SL of ADR 0649 as in ADR 0658 reading c5: at the converged loaded state of W, no negative
  eigenvalue of the free block `K_ff` of the driver's tangent at W's committed states (`count_below`, shift 0), and its lowest
  eigenvalue over its largest diagonal `> 0` (`lowest_eigenpairs`). The tangent is evaluated by the model of W (warm start from
  the committed modes, which at the converged state are the root: LW0).
- **c7: the mutations of the new update.** The mutations of the warm start (MW) and of the trust-region step (MT) below, each
  run once on a scratch copy against the default tests of the new file `si_eas_hg8.rs`; "caught" is a default test of that file
  that fails under the mutation (a check missed).
- **c8: default bits.** Without the two new opt-ins the library performs the same operations in the same order as at 14b155cd
  (D1, D2: every new branch is behind a flag that is off by default, and the material state returned is the same vector); the
  records: the bit tests `si_eas_hg3_bits.rs`, `si_eas_hg_default_bits.rs`, `si_eas_default_bits.rs`, `si_fbar_default_bits.rs`;
  `cargo test -p rcfd-api --release --test baseline_hashes`; the tests of ADR 0655 and 0658 (`si_eas_hg6.rs`, `si_eas_hg7.rs`,
  whose NC4 and NC2 records are bitwise); and NCW0 (a default model commits no modes).

## What was read

- `AGENT_RULES.md`; the brief; ADR 0658 (whole: registration, amendments 1 and 2, Addendum 1, the roadmap section); ADR 0655
  (D1, E1, E2, its remaining question 1); ADR 0649 (D2, readings, the results); ADR 0636 (its tolerances, through ADR 0649
  reading c4); ADR 0572 and 0573 (the form of the roadmap section, through ADR 0658).
- `crates/rcfd-fem/src/nl_incompatible.rs` (whole), `model.rs` (the opt-in fields, setters, `incompatible_mode_update`),
  `nonlinear.rs` (`initial_states`, `internal_force_field`, `force_and_tangent`, `attempt_with` and the commit of
  `states: trial`, `yielded_points`, `StateLayout`), `nl_transient.rs` (the energy record's call of `modes_and_strains_opt` with
  an empty state), `rotating.rs` (`unit_centrifugal_model`).
- `crates/rcfd-fem/tests/si_eas_hg7.rs` (whole); the ADR 0658 logs named above (the F-bar values on the three seeds; the E1, E3
  records of the diagnosis).

## Arithmetic done before the code (disclosed)

1. **Reference values** (ADR 0649 and ADR 0658 `run1-default.log`, 8 printed digits). The eight meshes: as ADR 0658 arithmetic
   1 (block `4^3` `0.1 h` `R = 0.38349042`, `w_c = 0.07137650`; `0.25 h` `0.38316202`, `0.07044819`; block `8^3` `0.34263132`,
   `0.06370837`; `0.34220247`, `0.06389947`; cantilever `10 x 2 x 2` `P = 6.17271412e-4`, `d = 5.24022571e-2`; `6.19924741e-4`,
   `5.23693020e-2`; `20 x 4 x 4` `5.72841645e-4`, `5.23430902e-2`; `5.75783694e-4`, `5.23261081e-2`). The three seeds of ADR
   0658: F-bar `R = 0.34387169`, `0.34915749`, `0.34360196`, `w_c = 0.06336271`, `0.06430598`, `0.06344073`. The F-bar code is
   unchanged, so FR3 (the same values to `1e-6`) is a reproduction of recorded runs on eleven meshes; on the three new seeds
   F-bar is reported.
2. **The warm start, by first principles.** The condensed element at `u` is defined by a root `alpha*(u)` of the mode equations
   `r(u, alpha) = 0` (ADR 0608 D2). Where the mode block `K_aa = dr / d alpha` is nonsingular at a root, the implicit function
   theorem gives a smooth branch `alpha*(u)` with `d alpha* / du = -K_aa^{-1} K_au`, so `alpha*(u) - alpha*(u_c) = O(|u -
   u_c|)`. Newton's method converges quadratically from any start in a ball about a regular root whose radius is fixed by `|K_aa^
   {-1}|` and the Lipschitz constant of `K_aa` (the Newton-Kantorovich theorem; cited from memory, only the qualitative statement
   is used). The cold start `alpha = 0` is at distance `|alpha*(u)|`; the warm start `alpha*(u_c)` at distance `|alpha*(u) -
   alpha*(u_c)|`, of the order of the change of the modes over one increment. ADR 0658 E3 measured the committed roots before the
   failing increments of seeds 2 and 3 as regular (smallest eigenvalue of the symmetric part of `K_aa` `1.2e5` and `1.95e5`
   against `5.2e7`), and E2 showed the cold iteration's trouble on the way from `alpha = 0` (`|delta| / |alpha|` up to `195`, an
   indefinite or near-singular `K_aa` along the path). Expected (not a limit): W passes seeds 2 and 3, where the warm start is
   inside the basin of the branch the committed state is on. The driver's first iterate of an attempt is the committed state
   itself, `u = u_c`, where the warm start is the root: no local step at all (LW0's reasoning).
3. **Same root, same results.** Where the cold iteration converges to the branch root (the eight meshes and seed 1, where S
   passed in ADR 0658), the warm start converges to the same root within the local tolerance `1e-12` of the scale, so W's
   measures are expected within `1e-6` relative of S's (not bitwise: the start changes the last bits of the modes) and the
   errors those of ADR 0658: `e_R`, `e_w` `0.1202`, `0.0406`; `0.1278`, `0.0870`; `0.0664`, `0.0349`; `0.0750`, `0.0287`;
   `e_P`, `e_d` `0.0580`, `0.0058`; `0.0694`, `0.0045`; `0.0203`, `0.0027`; `0.0297`, `0.0028`; seed 1 `0.0743`, `0.0495`. The
   largest error against its limit is `0.1278 / 0.15`. Seeds 2 and 3 and the three new seeds: as ADR 0658 arithmetic 3, `e_R` in
   `0.06` to `0.09`, `e_w` in `0.02` to `0.05`, against `0.10` (an estimate; the default fails on all three seeds of ADR 0658,
   so the new seeds are expected to need the remedy).
4. **LW0, exact.** The driver commits the trial states of the evaluation at the converged iterate `u` of the attempt (D1 reading
   c3); the modes stored there are the modes at which that evaluation's residual passed `|r|_inf <= 1e-12 scale`. A warm record
   at the same `u` from the same modes evaluates the same extended element at the same bits, so it passes the test at its first
   iterate: **exactly one iterate (no step) for every element**, at the result's displacement and states. Any other count says
   the stored modes are not the converged modes at the committed displacement.
5. **The trust-region step (D2) by first principles.** At the modes `alpha_j`, with the residual `r` and `J = K_aa` (exact,
   ADR 0658 E2: consistency `4e-10` to `1.1e-9`), the Levenberg-Marquardt step for the nonlinear equations `r(alpha) = 0`
   minimises the linear model of the merit `f = |r|_2^2 / 2`, `m(d) = |r - J d|_2^2 / 2` (the trial `alpha_j - d`), plus `mu
   |d|_2^2 / 2`: `d = argmin |[J; sqrt(mu) I] d - [r; 0]|_2`, i.e. `(J^T J + mu I) d = J^T r` (Levenberg, Quart. Appl. Math. 2
   (1944) 164; Marquardt, J. SIAM 11 (1963) 431; Nocedal and Wright, Numerical Optimization, 2nd ed. 2006, chapters 4 and 10;
   all cited from memory; what is used is derived here). This is the trust-region step of radius `|d(mu)|`: `mu` is the
   multiplier of the constraint `|d| <= Delta` (N&W theorem 4.1, cited from memory; not needed for what follows). It is solved
   by Householder QR of the `18 x 9` stacked matrix (no normal equations, so no squared condition number). Properties used:
   for `mu > 0` the stacked matrix has full rank whatever `J` (indefinite or singular), so the step exists; for `mu -> 0` and
   `J` nonsingular it is the Newton step `J^{-1} r`; the predicted reduction `pred = f(0) - m(d) = (|r|^2 - |r - J d|^2) / 2 =
   (d^T J^T r + mu |d|^2) / 2 > 0` for `d != 0` (from `J^T J d = J^T r - mu d`). The gain ratio `rho = (f(0) - f(alpha_j -
   d)) / pred`; the trial is accepted when `rho > eta = 1e-4` (N&W algorithm 4.1 takes `eta` in `[0, 1/4)`; `1e-4` is the
   constant of the decrease test of ADR 0658, so an accepted step satisfies `f(trial) < f(0)`); a trial that inverts a point or
   has a non-finite residual is rejected (`rho = -inf`). The multiplier is `mu = lambda max_i (J^T J)_ii` (the largest squared
   column norm of `J`, scale-free `lambda`); after a rejection or `rho < 1/4`, `lambda <- 4 lambda`; after `rho > 3/4`, `lambda
   <- max(lambda / 4, 1e-16)` (the thresholds of N&W algorithm 4.1 applied to the multiplier, the factor 4 its radius factor;
   cited from memory and fixed here, not tuned). **The initial `lambda_0 = 1e-8`, derived**: in a direction of singular value
   `sigma` the step is the Newton step times `sigma^2 / (sigma^2 + mu)`; `mu <= sigma^2 / 99` keeps a direction within one per
   cent of Newton, and asking that for every `sigma >= 1e-3 sigma_max` gives `lambda <= 1e-6 / 99 = 1.0e-8`; a direction with
   `sigma = 1e-6 sigma_max` (ADR 0658 E2's near-singular block: `48 / 5.2e7 = 9e-7`) is then damped by `1e-12 / (1e-12 + 1e-8)
   = 1e-4`. At most `MODE_HALVINGS = 20` rejections per step (then `lambda` has grown by `4^20 = 1.1e12`, the step a gradient
   step of length `|J^T r| / mu`); then the evaluation is refused with the iteration error. Accepted steps count against the cap
   `LOCAL_ITERATIONS = 25` and the convergence test is that of ADR 0608 D2 (`|r|_inf <= 1e-12 scale`), as for every update.
6. **What T is expected to do** (not a limit). LM is a descent method for `f`; its limit points are stationary points of `f`,
   `J^T r = 0`, which with `r != 0` needs a singular `J`. ADR 0658 E2 found the decrease iteration stalling at `|r|_2 = 223` with
   the smallest eigenvalue of `K_aa` at `+48` against `5.2e7`, close to such a point. T may therefore stall there too, or the
   damping may keep its path away from the near-singular region (the damped steps are short along the near-singular direction).
   Not estimated; that is what T measures.
7. **TR2 at a regular state.** At `u_b` of seed 3 (ADR 0658 E1, the state of R at the punch `0.0855`), where the backtracked
   cold record converged and `K_aa` at the root is positive definite (E3, smallest `1.95e5`, condition about `270`), the T record
   from `alpha = 0` is expected to converge to the same root. Two roots each within `1e-12 scale` of zero differ by at most
   `|K_aa^{-1}| 2e-12 scale`; with `|K_aa^{-1}| = 1 / 1.95e5` and the scale of the order of `|K_aa| |alpha|` (`5.2e7 |alpha|`)
   that is `2e-12 x 270 = 5.4e-10` of `|alpha|`. Limit `1e-7` relative (`|.|_inf`), margin `185`.
8. **NCT, NCW1.** The damping changes every step by a factor `sigma^2 / (sigma^2 + mu) != 1`, so T's iterates differ from
   Newton's in bits wherever a local step is taken; the warm start changes the start, hence the bits of the converged modes.
   Both are expected to change some value of the solve on the cantilever `10 x 2 x 2` at `0.25 h` against the default (NCT)
   and against S (NCW1).
9. **Cost.** ADR 0658's seed test (3 seeds x 4 solves of the `8^3` block) ran 77 s, about 6 s a solve; its block test 60 s,
   its cantilever test 20 s. This item: 14 set-ups x (F-bar, W, T) = 42 solves, plus S on two seeds (NCS2), S on the three new
   seeds (reported), NCW0, NCT, NCW1 on the small cantilever, and the two R solves of the local records: about 52 solves over
   five tests, expected 20 to 100 s each in release. A test over about two minutes is made an ignored heavy record after its
   single run (limits and code unchanged), the local-records test keeping the same code path fast.

## Design

### D1. The opt-in warm start of the mode iteration (library, `nl_incompatible.rs`, `model.rs`, `nonlinear.rs`)

- `SolidModel::set_incompatible_mode_warm_start(on: bool)` (default `false`) with the getter `incompatible_mode_warm_start()`;
  the centrifugal copy (`rotating.rs`) keeps it as it keeps the other opt-ins.
- The internal `converged` takes a start `Option<[f64; 9]>` (`None`: `alpha = 0`, the iteration of before, the same operations).
- `evaluate` takes `warm: bool`. With `warm`, the element state may carry nine values after the material state (`points x
  state_size + 9`); the material part is passed on as before; when the nine values are present the local iteration starts from
  them, and if that iteration fails (any error) it is run once more from `alpha = 0` (the fallback; the start may invert at the
  new displacement, where no step exists to halve). The returned trial state is the material trial state followed by the nine
  converged modes. A state of length `points x state_size` (the initial states, unchanged) starts from `alpha = 0`. Without
  `warm`, every operation is that of 14b155cd and a state of the extended length is refused as now.
- The driver's two evaluations of an Incompatible Hex8 (`internal_force_field`, `force_and_tangent` in `nonlinear.rs`) pass
  `model.incompatible_mode_warm_start()`. The energy record of `nl_transient.rs` (an empty state) and `modes_and_strains_opt`
  are unchanged (cold), a limit.
- For checks: `mode_iteration_record_from(coords, material, displacement, state, context, stabilised, update, start)` (the
  record of ADR 0658 D1 from a given start) and `condensed_force_and_tangent_warm(coords, material, displacement, state,
  context, stabilised, update)` (the driver's warm evaluation: force, tangent, stress and the trial state values).

### D2. The opt-in trust-region mode step (library, `nl_incompatible.rs`, `model.rs`)

- A new variant `IncompatibleModeUpdate::TrustRegion` (the enum is `non_exhaustive`) and
  `SolidModel::set_incompatible_mode_trust_region(on: bool)` (default `false`) with its getter; `incompatible_mode_update()`
  returns `TrustRegion` when it is on, whatever the backtracking and decrease flags, and otherwise exactly what it returned at
  14b155cd. The centrifugal copy keeps it.
- The local iteration with `TrustRegion` is a separate function (the loop of `converged` for the other updates is not touched):
  arithmetic 5, constants `TRUST_REGION_DAMPING = 1e-8` (`lambda_0`), `TRUST_REGION_DAMPING_FLOOR = 1e-16`,
  `TRUST_REGION_ACCEPT = 1e-4` (`eta`), thresholds `1/4`, `3/4`, factor `4`, at most `MODE_HALVINGS` rejections per step, cap
  `LOCAL_ITERATIONS` accepted steps, the convergence test of ADR 0608 D2, the same start rule as D1. The condensation at the
  converged modes is that of ADR 0608 D2 (unchanged).
- For checks: `trust_region_record(coords, material, displacement, state, context, stabilised, start)` returns per accepted
  iterate `|r|_2`, `|r|_inf`, the scale, the rejections of the step that led to it, `lambda` and `rho` of that step, and the
  outcome; `mode_iteration_record_from` with `TrustRegion` gives the same iterates as `ModeIterate` (halvings = rejections).

### D3. The test file `crates/rcfd-fem/tests/si_eas_hg8.rs`

Helpers copied from `si_eas_hg7.rs`. Options per solve: line search, backtracking, decrease, warm start, trust region.

- `w_and_t_on_the_four_blocks`: M0, M1, FR3, W and T on the four block meshes; SLW; LW0, LW2 on DB4.
- `w_and_t_on_the_four_cantilevers`: the same for the cantilevers; NCW0, NCT, NCW1.
- `w_and_t_on_the_seeds_of_adr_0658`: M0, M1, FR3, W, T, SLW on seeds `0xF739_0001` to `_0003`; NCS2.
- `w_and_t_on_three_more_seeds`: the seed rule of reading c1, M0, M1, F-bar, W, T, SLW; S reported.
- `local_records_at_the_stall_of_seed_3`: NCL, LW1, LW3, TR1, TR2; the T record at `u_T` reported.

## Checks

Every measure is required finite before it is compared (a non-finite value or a failed solve misses).

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| M0 | c1, c2 | smallest corner `det J` / centre `det J`, each of the 14 meshes | `> 0` | ADR 0658 (eleven); new seeds unknown (rule of c1) |
| M1 | c1, c2 | moved nodes: block `4^3` 27, `8^3` 343 (and each seed), cantilever `10 x 2 x 2` 9, `20 x 4 x 4` 171 | exactly | ADR 0649 |
| FR3 | c5 | F-bar on the eight meshes and the three seeds of ADR 0658 against arithmetic 1: `|x / x_recorded - 1|` | `<= 1e-6` each | rounding, `<= 7.8e-8` |
| WB1 to WB4 | c1, c5 | W, blocks `4^3` `0.1`, `0.25`; `8^3` `0.1`, `0.25` (DB4): `e_R`, `e_w` | `<= 0.15`, `0.15`, `0.10`, `0.10` each | arithmetic 3 |
| WC1 to WC4 | c1, c5 | W, cantilevers `10 x 2 x 2` `0.1`, `0.25`; `20 x 4 x 4` `0.1`, `0.25`: `e_P`, `e_d` | `<= 0.15`, `0.15`, `0.10`, `0.10` each | arithmetic 3 |
| WR1 to WR3 | c1, c5 | W, `8^3` `0.25 h`, seeds `0xF739_0001` to `_0003`: `e_R`, `e_w` | `<= 0.10` each | seed 1 `0.0743`, `0.0495`; seeds 2, 3 `0.06` to `0.09`, `0.02` to `0.05` |
| WR4 to WR6 | c2, c5 | W, the three new seeds: `e_R`, `e_w` | `<= 0.10` each | as WR2 |
| SLW | c6 | W at its loaded state, each of the 14 set-ups: negative eigenvalues of `K_ff`; lowest / largest diagonal | `0`; `> 0` | 0; ADR 0658 SLR |
| T | c4 | T on the 14 set-ups: outcome, the two errors, SL, "passes" against W's limits | measured (no limit) | arithmetic 6 |
| LW0 | c3, c7 | on DB4, at W's result: for every element, the iterates of the warm record (update S) from the committed modes at the result's displacement | exactly `1` each (count of elements with another count `0`) | arithmetic 4 |
| LW2 | c3, c7 | on DB4, W's committed states: elements whose state does not have exactly 9 values | `0` | D1 (`state_size` 0) |
| NCL | c3 | at `u_T` of seed 3, element 476 (ADR 0658 E1: `u_T = u_b + (p_T - p_b) / (p_b - p_a) (u_b - u_a)`, R solved to `p_a = 0.0850`, `p_b = 0.0855`, `p_T = 0.0858`): the cold record with update S | fails (not converged) | ADR 0658 E1 (stall at `223.1`) |
| LW1 | c3 | at the same `u_T`: the warm record with update S from `alpha_b` (the converged modes of the cold `Backtracking` record at `u_b`, ADR 0658 E3) | converged within 25 iterates | arithmetic 2 |
| LW3 | c3, c7 | at the same `u_T`: `condensed_force_and_tangent_warm` with update S and the state `alpha_b` succeeds, and its trial state's last nine values are the converged modes of LW1 to `1e-12` relative (`|.|_inf`) | succeeds; `<= 1e-12` | the same iteration |
| TR1 | c4, c7 | the T records at `u_T` and at `u_b` (from `alpha = 0`): largest `|r_{j+1}|_2 / |r_j|_2` over the accepted steps; smallest `rho` of an accepted step | `< 1`; `> 1e-4` | by construction (arithmetic 5) |
| TR2 | c4, c7 | at `u_b`: the T record converges, its modes against those of the cold `Backtracking` record, `|.|_inf` relative | converged; `<= 1e-7` | `<= 5.4e-10` (arithmetic 7) |
| NCS2 | c3 | S (cold, the update of W) on seeds `0xF739_0002` and `_0003` | fails on each | ADR 0658 |
| NCW0 | c8 | the default model (no opt-in), rule L, cantilever `10 x 2 x 2` at `0.25 h`: elements whose committed state has a value | `0` | D1 |
| NCT | c4 | T against the default on the same set-up: values differing in bits | `> 0` | arithmetic 8 |
| NCW1 | c3 | W against S on the same set-up: values differing in bits | `> 0` | arithmetic 8 |
| B6 | c8 | `si_eas_hg3_bits.rs`, `si_eas_hg_default_bits.rs`, `si_eas_default_bits.rs`, `si_fbar_default_bits.rs` | pass | |
| S5 | c8 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | |
| S6 | | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |
| S7 | c8 | the existing tests of the changed lines: `cargo test -p rcfd-fem --release --lib`, `--test si_eas_hg7`, `si_eas_hg6`, `si_eas_hg5`, `si_eas_hg3`, `si_hex8x`, `si_eas`, `si_eas_hg`, `si_eas_hg2` | pass | |

Reported without a limit: T on every set-up (all of it); S on the three new seeds (is the warm start needed there); W against S
and T against W where both converge (`|a_W / a_S - 1|`); the increments, halvings and searched steps of every solve; F-bar's
SL and its values on the new seeds; the full records at `u_T` and `u_b`; the distance of the LW1 modes from `alpha_b`.

### Negative controls

| Id | Control | Expected |
|---|---|---|
| NCS2 | S cold on the two failing seeds (checked above) | fails as in ADR 0658: the set-ups reproduce the miss the warm start is to remedy |
| NCL | the cold record at `u_T` (checked above) | fails as ADR 0658 E1: the local state LW1 is tried at reproduces the stall |
| NCW0 | the default commits no modes (checked above) | element state added only behind the opt-in |
| NCT | T against the default (checked above) | differs: the trust-region opt-in reaches the driver |
| NCW1 | W against S (checked above) | differs: the warm-start opt-in reaches the driver |

### Planned mutations (on a scratch copy, each run once against the default tests of `si_eas_hg8.rs`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MW1 | the driver ignores the warm start (passes `false` at both call sites in `nonlinear.rs`) | LW2 (no modes committed), LW0 (no stored modes: a cold record, more than one iterate), NCW1; WR2, WR3 if W passes them |
| MW2 | the evaluation stores the start modes instead of the converged modes | LW0, LW3 |
| MW3 | the start not read (the iteration always from `alpha = 0`, the modes still stored) | LW3 (the warm evaluation at `u_T` fails as NCL); NCW1 (W becomes S bitwise); WR2, WR3 if W passes them |
| MW4 | no fallback to `alpha = 0` after a failed warm iteration | uncertain: only where a warm start fails and the cold one succeeds; registered as possibly not caught |
| MW5 | the warm start always on in the driver (the getter returns `true`) | NCW0 |
| MT1 | the driver ignores the trust region (`incompatible_mode_update()` ignores its flag) | NCT |
| MT2 | the step's sign flipped (`alpha_j + d`) | TR2 (no convergence), TR1 |
| MT3 | `lambda` not increased after a rejection | uncertain: TR2 or TR1 only if a record there has a rejection; registered as possibly not caught |
| MT4 | every trial accepted (no gain-ratio test) | TR1 only if a T record there has a trial with `rho <= 1e-4`; registered as possibly not caught |

## Decision rule

- **done**: M0, M1, FR3, WB1 to WB4, WC1 to WC4, WR1 to WR6, SLW, LW0 to LW3, NCL, TR1, TR2, NCS2, NCW0, NCT, NCW1, B6, S5, S6, S7
  met in their single registered run; T measured on all fourteen set-ups (its outcome recorded); MW1, MW2, MW3, MW5, MT1, MT2
  each caught by at least one check of `si_eas_hg8.rs`; no amendment. (MW4, MT3, MT4 are recorded either way.)
- **partial**: B6, S5, NCW0 met, but a row of W missed on some set-up, or another check missed, or one of MW1, MW2, MW3, MW5,
  MT1, MT2 not caught, or a check met only after an amendment.
- **todo**: B6, S5 or NCW0 fails (a default bit changed), or W cannot be run.

## Limits (not covered)

- One material (hyperelastic, no history; `state_size` 0, so the committed state of W is the nine modes alone) and Poisson
  ratio, one load level per case, the meshes of ADR 0649 and six seeds of one of them; no statistics beyond fourteen set-ups.
- The warm start with a history material (J2, creep, viscoelastic: the modes after the material state) is implemented but not
  run; the driver's active-set record (`record_active_set`, ADR 0515) reads the nine values as a further point there; not
  covered.
- The energy record of the nonlinear transient path and `modes_and_strains_opt` stay cold (an empty state); the transient and
  rotating paths with the warm start are not run.
- T is measured cold only; the warm start with the trust region, and either with the driver's line search, are not run.
- No tangent check (T-FD) of W or T: both change the path to the root of the mode equations, not the condensation at it (ADR
  0608 D2), which ADR 0655 E2 checked at `9.1e-10`; SLW uses the driver's tangent at the root.
- The trial `u_T` stands in for the driver's failing iterate (ADR 0655 E1, ADR 0658 E1), which the driver does not return.
- The default stays cold and without the trust region (open question 2).

## Open questions (taken or open)

1. **The design of the two opt-ins**: the committed modes after the material state of the element, the fallback to `alpha = 0`
   after a failed warm iteration, W with the update S and without the line search, the LM step on the residual merit with QR,
   `lambda_0 = 1e-8`, `eta = 1e-4`, the thresholds and factor of N&W algorithm 4.1, `TrustRegion` taking precedence over the
   other flags: taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. **Default.** Making W (or T) the default would change default results (and the length of the committed states): open for
   the maintainer, not taken.
