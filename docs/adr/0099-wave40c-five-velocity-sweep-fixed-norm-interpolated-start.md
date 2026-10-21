# ADR 0099: Wave 40C, partials close-out for F-095 and F-115

Status: registered before the registered tests were run. Limits are fixed; a miss becomes an `#[ignore]`d `*_known_miss` test
with the limit unchanged. No library code changes: every addition is test code, so defaults and the baseline hashes are untouched.
Clean-room: the only reference values are analytic (plane Poiseuille pressure drop, derived below from first principles). The
acceptance text of both items is unchanged.

The residual gaps are those of [partials group 8](../audit/partials-group-8.md). I re-checked them before writing: no
non-ignored test in crates/rcfd-cli/tests/study_w20c.rs runs the five-velocity sweep (only the ignored S7 does); the F-115
residual check in crates/rcfd-api/tests/interpolate.rs compares residuals that are each divided by their own run's variation, so
the uniform start gives 1.0 by construction.

## F-095: five-velocity sweep (test file crates/rcfd-cli/tests/study_w20c.rs)

Acceptance (unchanged): "Sweep of 5 inlet velocities runs in parallel and returns a table; resumable".

Case: the existing `POIS` case of S7 (channel_2d, nx 100, lx 4, ly 0.1, nu 0.01, rho 1, simplec, upwind, tolerances 1e-9,
max_iterations 8000, report `dp` = area-average p on the inlet), parameter `u` = inlet velocity vector, json values
`[0.02,0,0]`, `[0.04,0,0]`, `[0.06,0,0]`, `[0.08,0,0]`, `[0.1,0,0]`, `--no-fields`.

* P1 (test `p1_five_velocity_sweep_parallel_equals_sequential_and_resumes`, ny 10): run A `--parallel 2 --json` into `a`, run B
  `--parallel 1 --json` into `b`, run C `--resume --json` into `a`. Limits: A exits 0, its summary has `run` 5 and `skipped` 0,
  table.json has 5 rows, every row `status` = `ok` and a finite `dp`; `table.csv` and `table.json` of A and B are byte-identical
  (the S2 standard: "equal" means bytes); C exits 0 with summary `run` 0 and `skipped` 5, and A's table files are byte-identical
  to their state before C. This is the acceptance as written; ny 10 is used because it is the S7 mesh and the cheaper one.
* P2 (test `p2_poiseuille_map_ny20_within_one_percent`, ny 20, `--parallel 2`): for each of the five rows,
  |dp / dp_exact - 1| <= 0.01 with dp_exact = 12 rho nu U L / h^2 (fully developed plane Poiseuille: u(y) = 6 U y (h - y)/h^2
  has wall shear 6 mu U / h on each of two walls, so the force balance h dp/dx = 2 * 6 mu U / h gives dp/dx = 12 mu U / h^2; the
  entrance length at Re = U h / nu <= 1 is a few hundredths of L and adds a small positive excess). Limit justification (rule a):
  the 1 % comes from the merged-review scope clause; the mesh study is the existing non-ignored
  `poiseuille_error_decreases_with_refinement`. What I knew when writing this: at u = 0.04 the error is 1.90 % at ny 10 and
  0.29 % at ny 20 (both from the earlier wave); the other four velocities at ny 20 have not been measured. The audit proposed this
  re-registration at ny 20; the S7 test at ny 10 stays ignored and unchanged as a known miss.

Smoke (rule d, before registering): `rcfd study --list` on the ny 20 case lists the 5 points `u=[0.02,0.0,0.0]` to
`u=[0.1,0.0,0.0]`, exit 0. Resume round trip on a tiny version (nx 20, ny 4, lx 0.8, same json parameters, `--no-fields`):
`--parallel 2` run 5 / skipped 0 exit 0, `--resume` run 0 / skipped 5 exit 0, and `--parallel 1` table.json byte-identical to the
`--parallel 2` one. No smoke run of the registered ny 10 or ny 20 sweep.

Decision rule: if P1 passes, the acceptance as written is met by a non-ignored test and F-095 is set `done`; the remaining scope
in its note (multi-objective optimiser, real-fan study, warm start beyond steady SIMPLE/SIMPLEC, and P2 if it misses) should be
split into a new item, which this wave does not create.

## F-115: initial residual with a fixed normalisation (test file crates/rcfd-api/tests/interpolate.rs)

Acceptance (unchanged): "Solution from a coarse cavity interpolated to a fine mesh starts at a lower initial residual than
uniform init; conservative and non-conservative modes selectable".

The default normalised residual is `num / norm_ref` with `num = sum|b - A x|` and `norm_ref = sum(|A x - A xbar| + |b - A xbar|)`
taken at the current iterate x. At a uniform start x = 0 this ratio is 1 by construction. The new check freezes the denominator
at the converged solution, so both starts are measured on the same scale.

Set-up (test `f1_fixed_normalisation_initial_residual_interpolated_below_uniform`): the existing pair, 12 x 12 cavity converged to
1e-7, checkpoint, interpolated onto 24 x 24 (simplec, upwind, tolerance 1e-6), both modes `NonConservative` and `Conservative`;
uniform start on the same 24 x 24 case. `record_parts` is set on the solver after `interpolate_from` and before the first step
(it only records; the solve uses the same residual path). Denominators: from the uniform run converged to 1e-6, the last
iteration's `norm_ref` of each active momentum component (D_ux, D_uy) and of the pressure equation (D_p). Metrics, from the first
iteration of each start: R_u = max over components of num_k / D_k, R_p = num_p / D_p.

* F1 limit: for both modes, R_u(interpolated) < R_u(uniform) and R_p(interpolated) < R_p(uniform). The strict inequality is the
  acceptance text itself; with a common denominator it can fail, so it is no longer vacuous. No mesh study is needed because
  the limit is the acceptance's own comparison, not a tolerance. The absolute R values are printed and recorded.
* The existing S5b (`restart_w19c.rs`, 10 versus 23 iterations to within 1e-2 of the converged speed) stays as it is; the
  negative iteration-count result (103 interpolated versus 89 uniform to tolerance 1e-6) stays in the note.

Smoke (rule d, before registering): 6 x 6 cavity after 3 iterations, checkpoint, onto 12 x 12, with and without
`interpolate_from`, `record_parts` set afterwards, 3 iterations: `parts_history` has 3 entries of 2 components and every
`pressure_parts_history` entry is `Some`. So the flag survives `interpolate_from`. The registered 12 to 24 pair was not run.

Decision rule: if F1 passes for both modes, F-115 is set `done`.

## Addendum: measured values

Run once each, after this ADR was committed (commit 16d0952), release build.

F-095:
* P1 passes (`p1_five_velocity_sweep_parallel_equals_sequential_and_resumes`): `--parallel 2` exit 0, run 5 / skipped 0,
  5 rows all `ok` with finite dp; `table.csv` and `table.json` byte-identical to the `--parallel 1` run; `--resume` exit 0,
  run 0 / skipped 5, table files unchanged. The acceptance as written is met by this non-ignored test, so F-095 is set `done`.
* P2 passes (`p2_poiseuille_map_ny20_within_one_percent`), limit 1 %: relative dp error at ny 20 is 0.309 % (u 0.02),
  0.294 % (0.04), 0.280 % (0.06), 0.265 % (0.08), 0.251 % (0.1), all below the exact value. S7 at ny 10 stays an ignored
  known miss (1.9 %).
* Remaining F-095 scope (no multi-objective optimiser, no CMA-ES, no real-fan study, warm start only for steady SIMPLE/SIMPLEC,
  T3 Nelder-Mead known miss) should be split into a new item; this wave does not create it.

F-115:
* F1 MISSES. Denominators from the converged uniform run: D_ux 0.07650, D_uy 0.03710, D_p 0.03765. Uniform start:
  R_u 0.2614, R_p 0.1259. Non-conservative: R_u 0.1547 (ratio 0.592, passes), R_p 0.1488 (ratio 1.181, fails).
  Conservative: R_u 0.1546 (ratio 0.591, passes), R_p 0.1480 (ratio 1.175, fails). The test is kept as the ignored
  `f1_fixed_normalisation_initial_residual_known_miss` with the limit unchanged (it now evaluates both modes before asserting,
  so both are recorded; the first run stopped at the non-conservative pressure failure).
* Reading, not a fix: on a common scale the interpolated start has a lower momentum residual but a higher pressure-equation
  residual than the uniform start. A likely cause is the face flux rebuilt from the interpolated velocity, which is not
  divergence-free on the fine mesh (flux imbalance 0.18 after a 12 to 24 transfer, ADR 0031); this was not tested here.
  F-115 stays `partial`.
