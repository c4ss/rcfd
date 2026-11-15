# ADR 0552: F-629 registration: six-unknown nodes in the smoothed-aggregation CG (the rotation components of the rigid fields)

Status: **REGISTERED 2026-11-15, written before any code of F-629 in the repository and before any run of its checks.** Not yet
accepted by the maintainer. Before this text no library or test code of F-629 was written in the worktree. A scratch prototype was
written and run (disclosed under "Arithmetic and prototype"): it uses only the public API at the base 2cc5a1c7 (assembly,
elimination, `rcfd_linalg::amg_elastic::ElasticAmg`, `rcfd_linalg::cg`) with the near-null function of D1 written out in the
scratch crate. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-629 (deps F-541), the follow-up named by [ADR 0545](0545-f541-registration-shell3-on-the-remaining-paths.md) (its
known miss KM1 and its "For the lead" entry). Number 0552 given by the lead. Branch `f629-work` from 2cc5a1c7, worktree
`w49i-work/f629`, scratch `w49i-work/f629-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests not edited; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code; clean-room. The roadmap files are not edited (the entries are in the last section of the addendum). Every choice
below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-629 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. The near-null space of the smoothed aggregation takes the rotations of Beam2, Shell4 and Shell3 nodes (the six rigid fields with their rotation components); the plate and cantilever models of ADR 0432 and ADR 0454 solved by the preconditioned CG within 1e-8 of the direct solve in a registered number of iterations; the solid-only bits unchanged.

Clauses: **a1** every limit registered here; **a2** the near-null space carries, at each Beam2, Shell4 and Shell3 node, the six
rigid fields with their rotation components; **a3** the plate and cantilever models of ADR 0432 (Shell4) and ADR 0454 (Shell3)
solved by the smoothed-aggregation CG within 1e-8 of the direct solve; **a4** in a number of iterations registered here; **a5** the
solid-only bits unchanged.

## What was read (no run of a check)

`AGENT_RULES.md`; the brief; ADR 0545 (D0, D3, KM1, MU8: "the smoothed aggregation panics: `amg_elastic.rs:177` index out of
bounds on six-unknown nodes", and its "For the lead" text, which is F-629's acceptance); ADR 0305 (the smoothed aggregation: D1
rigid-body modes, D3 tentative prolongator with the column-drop rule, D5 settings, the iteration estimates and results); ADR 0370
(geometric aggregation), ADR 0375 (the smoother); ADR 0417 (six unknowns per node, Beam2; D4: the multigrid refusal); ADR 0432
(Shell4: arithmetic 11 and 12, the simply supported plate and the cantilever strip, K1 the rigid motions `u = w x x`, `theta =
w`; D1 `DofMap::for_mesh`; D3 which paths take shells); ADR 0454 (Shell3: arithmetic 10 and 11, the plate 32 x 32 cut in two and
the strip; row N2); the sources at 2cc5a1c7: `rcfd-linalg/src/amg_elastic.rs` (whole: `rigid_body_modes`, `tentative_block`,
`tentative`, `ElasticAmg::build`), `rcfd-fem/src/solve.rs` (whole: the multigrid refusal at the head of `solve_static_report`,
`smoothed_aggregation`, `node_blocks`, `preconditioned_cg` with its true-residual restarts, `solver_name`), `mpc.rs`
(`solve_static_mpc` refuses every preconditioned CG on the transformation path), `rcfd-api/src/solid_case.rs` (the case-file keys
`amg`, `amg_quadratic`, `amg_quadratic_smoothed`); the tests `si_beam.rs`, `si_shell.rs`, `sa_shell3.rs`, `sa_shell3_paths.rs` (the
N2 rows and RC1 assert that `CgPreconditioner::SmoothedAggregation` refuses Beam2, Shell4 and Shell3), `si_amg.rs`.

Method, cited from memory: P. Vaněk, J. Mandel and M. Brezina, "Algebraic multigrid by smoothed aggregation for second and fourth
order elliptic problems", Computing 56 (1996) 179-196 (the near-null space of plates and shells given with the rotational degrees
of freedom). Nothing numeric is taken from it; the fields used are derived in D1.

## D0. Why a new preconditioner variant (and not lifting the refusal)

`CgPreconditioner::SmoothedAggregation` refuses Beam2, Shell4 and Shell3, and that refusal is asserted by existing tests (the N2
rows of `si_beam.rs` (five copies), `si_shell.rs` (four), `sa_shell3.rs` and RC1 of `sa_shell3_paths.rs`). Existing tests are not
edited, and the refusal of the existing variant is the behaviour every existing caller sees. So the capability is opt-in: a new
variant `CgPreconditioner::SmoothedAggregationStructural(ElasticAmgSettings)` (the enum is `#[non_exhaustive]`, so adding a
variant breaks no caller), solver name `cg-amg-rbm-structural`. The existing variant, its refusal, its near-null space
(`rcfd_linalg::amg_elastic::rigid_body_modes`) and `solve::smoothed_aggregation` are not changed (a5 by construction, checked by
B1 and B2). Retiring the N2 rows so that the existing variant takes structures is left to the maintainer (open question Q1).

`rcfd-linalg` is not changed: the near-null space is a vector list the multigrid takes as given (ADR 0305); the six fields with
rotations are formed in `rcfd-fem` (D1). The case-file key in `rcfd-api` is not added (the solid case file builds boxes and Gmsh
solids; a key for a variant no case can use would be untested; follow-up).

## D1. The six rigid fields with rotation components

A small rigid motion with translation `t` and rotation vector `w` about `x_0` moves the point `x` by `u = t + w x (x - x_0)` and
turns every material line through `x` by `theta = w`. For a Beam2, Shell4 or Shell3 node the unknowns are `(u_x, u_y, u_z,
theta_x, theta_y, theta_z)` in global axes (ADR 0417 D1, ADR 0432 D1); ADR 0432 K1 and ADR 0417 K1 check `K r = 0` for exactly
these six fields (`r = (w x x, w)`). A three-unknown node carries `u` only. So the six columns of the near-null matrix `B`,
indexed by the reduced (free) equations `r` with node point `x_r` and component `c_r`, are:

- columns `j = 0, 1, 2` (translations `t = e_j`): `B[r][j] = delta(c_r, j)` for `c_r < 3`, `0` for `c_r >= 3`;
- columns `j = 3 + a`, `a = 0, 1, 2` (rotations `w = e_a`): for `c_r < 3` the component `c_r` of `e_a x (x_r - x_0)` (the
  expression of `rigid_body_modes`, unchanged); for `c_r >= 3`, `delta(c_r - 3, a)`.

`x_0` is the mean of the points of the rows, as in `rigid_body_modes` (ADR 0305 D1; the mean is over all free rows, rotation rows
included). For a model whose free rows all have `c_r < 3` the columns are formed by the same floating-point operations in the same
order as `rigid_body_modes` (the mean summed in row order, then divided, `d = x_r - x_0`, the same `rot` table), so they are
bit-identical to it (B2a). New public function `solve::structural_rigid_body_modes(points, components) -> Vec<Vec<f64>>`
(components `0..6`; a component `>= 6` is a caller error and panics with a message, as an index out of range does now).

The tentative prolongator (ADR 0305 D3, unchanged) takes `B` restricted to each aggregate and drops a column whose norm after
orthogonalisation is at most `1e-10` of its norm before (or which is zero). On the simply supported plates only `w, theta_x,
theta_y` are free (`u = v = theta_z = 0` prescribed at every node), so columns 0, 1 and 5 are zero there and are dropped: three
columns per aggregate (`w = 1`; `w = d_y, theta_x = 1`; `w = -d_x, theta_y = 1`), the three rigid fields of a plate in bending.
On a straight beam the rotation about the axis has `u = 0` on the line only when the aggregate's points are collinear with `x_0`
on it; with the rotation components every column is non-zero and the six stay independent (a rotation is seen through `theta`).

## D2. The solve path

`solve_static_report` (elimination path) with `PreconditionedCg { preconditioner: SmoothedAggregationStructural(settings), .. }`:
no element-kind refusal; `preconditioned_cg` builds `solve::smoothed_aggregation_structural(a, free, dofs, nodes, settings)`,
which is `smoothed_aggregation` with `structural_rigid_body_modes` in place of `rigid_body_modes` (node blocks of 3 or 6 free rows
or fewer where some are prescribed; the geometric aggregation takes the point of each block as before). The CG, the true-residual
restarts and the report are those of the other preconditioners. The transformation path refuses every preconditioned CG
(`mpc.rs`, unchanged), so this variant too. Nothing else changes; the existing arms of every `match` are untouched and the new arm
is added after them.

## Arithmetic and prototype (disclosed)

1. **Sizes and levels by hand.** ADR 0432 plate 16 x 16 Shell4: 289 nodes, free `3 x 289 - 64 (w on the edges) - 2 x 34 (theta_x
   on x = 0, 1; theta_y on y = 0, 1)` `= 735` (the corners counted once in the 64). ADR 0454 plate 32 x 32 Shell3: `3 x 1089 - 128
   - 2 x 66 = 3007`. Strips 15 x 1: `6 x 32 - 12 = 180`. With the default `max_coarse = 1000` the Shell4 plate and both strips are
   their own coarsest level (one level: the V-cycle is the dense LU, CG converges in one iteration up to rounding); only the
   Shell3 plate gets two levels. So the acceptance's iteration counts are also registered with a forced hierarchy **S1** =
   `ElasticAmgSettings { max_coarse: 60, ..default }`: 60 is below the smallest model (180) and above the coarse order of the
   strips' first level (greedy aggregation at `theta = 0` takes a node and its neighbours, about 6 nodes of the 2 x 16 strip,
   so about 6 aggregates x 6 columns = 36), so every model has at least two levels. **S0** = `ElasticAmgSettings::default()`.
2. **Tolerance.** The CG's true relative residual `tau` must be above the rounding floor of the residual (ADR 0305 addendum: the
   floor stopped both solvers at `1.4e-9` on a 121 680-unknown solid). The prototype at `tau = 1e-10` and `1e-12`
   (`f629-scratch/estimate1.log`) shows floors of `1.5e-10` (Shell4 strip) to `7.1e-10` (Shell4 plate t/a = 0.001): the solve
   is refused there after three restarts. The error against the direct solve was at most about the relative residual in every
   prototype row (largest ratio `err_inf / rr = 0.94`, the beam at `tau = 1e-10`). So **`tau = 1e-9`** (above every floor seen, and an expected error of order
   `1e-9`, ten times under the acceptance's `1e-8`), `max_iterations = 5000` (so that a count above its limit is measured, not
   refused).
3. **Prototype at `tau = 1e-9`** (`f629-scratch/estimate2.log`, `f629-scratch/proto/src/main.rs`; the same models, prescribed
   lists and loads as the tests below, the near-null of D1, `ElasticAmg::new` with the greedy aggregation, the CG loop of
   `preconditioned_cg` copied): iterations (S0 / S1), levels at S1, error `|u - u_d|_2 / |u_d|_2`:
   Shell4 plate t/a = 0.01: 1 / 85, 3 levels, `<= 6.0e-12`; t/a = 0.001: 1 / 531 (2 restarts), 3, `<= 1.0e-10`; Shell4 strip:
   1 / 50, 2, `<= 8.3e-11`; Shell3 plate t/a = 0.01: 32 / 50, 3, `<= 1.5e-12`; t/a = 0.001: 38 / 59, 3, `<= 7.6e-13`; Shell3
   strip: 1 / 23, 2, `<= 4.6e-12`; Beam2 40-element cantilever (D3 model BM): 1 / 21, 3, `<= 4.6e-11`. Operator complexity at S1:
   1.08 to 1.19 (shells), 1.45 (beam).
4. **Without the rotation components** (the D1 columns with `B[r][j] = 0` for `c_r >= 3`, the prototype's "norot"): S1 counts
   Shell4 plate 93 and 605, Shell4 strip 78, Shell3 plates 52 and 57, Shell3 strip 28, Beam2 46. The rotation components halve
   the beam's count and cut the Shell4 strip's by a third; on the plates the effect is small (the plate's rotation columns carry
   `w` already and a uniform `theta` is near the coarse space). This is NC1.
5. **Iteration limits** = `ceil(1.5 x prototype count)` (the margin covers a different rounding of the assembly path:
   `solve_static_report` assembles by `stiffness_with_materials`, the prototype by `stiffness`; and is the growth margin of ADR
   0305 H1). The thin Shell4 plate at S1 (531) is the weak case of the hierarchy and is registered as measured, not improved:
   smoothed aggregation with a forced hierarchy on a thin MITC4 plate is not robust in `t/a` here (the Shell3 plate at the same
   thickness needs 59); see L2.

The prototype is the arithmetic of the item. The tests below run the library path once each; their results are compared with
these numbers only through the registered limits.

## D3. The models

- **P4a, P4b**: ADR 0432 arithmetic 11, Shell4 16 x 16 on the unit square, `E = 2e11`, `nu = 0.3`, `rho = 7850`, t = 0.01 and
  0.001, `q = 1000 Pa` by `Load::shell_pressure`, the prescribed lists of `si_shell.rs::ss_plate` (`u, v, theta_z` everywhere,
  `w` on the edges, `theta_x` on `x = 0, 1`, `theta_y` on `y = 0, 1`).
- **C4**: ADR 0432 arithmetic 12, Shell4 strip `1 x 0.1`, 15 x 1, t = 0.01, `nu = 0`, the two root nodes clamped (six each),
  `P = 1 N` along `+z` split on the two tip nodes.
- **P3a, P3b**: ADR 0454 arithmetic 10, Shell3 32 x 32, each square cut along `(i, j)-(i + 1, j + 1)` (diagonal 0), the plate data
  and prescribed lists of P4.
- **C3**: ADR 0454 arithmetic 11, the strip of C4 with each square cut along diagonal 0.
- **BM** (a2 for Beam2; not an ADR 0432/0454 model, registered here): 40 Beam2 elements on the line from `0` to `(2, 1, 0.6)`,
  section `BeamSection::new(0.012, 3e-6, 5e-6, 4e-6, 0.85, 0.75)` (ADR 0417's section1), orientation `(0, 0, 1)`, `E = 2.1e11`,
  `nu = 0.3`, node 0 clamped (six), tip force `(120, -250, 370) N`.
- **NN models** (a2, the kernel check): unconstrained Shell4 4 x 4 on `[0, 0.6] x [0, 0.4]` and Shell3 4 x 4 (diagonal 0), t =
  0.01, both turned by `Q = rotation((1, -2, 2), 0.7)` and shifted by `(0.3, -0.2, 0.5)` (the placement of ADR 0432 K1); the BM
  beam unconstrained.
- **Solid models** (a5): the Hex8 cantilever `hex_box([0; 3], [4, 1, 1], [16, 4, 4])`, steel, the face `x = 0` clamped, a tip
  load on `x = 4` (the ADR 0282 pattern, smaller), solved with both variants.

## Checks (limits fixed now)

Every aggregated error is formed with a NaN-propagating maximum and checked finite (a NaN fails). The direct solve is
`StaticSolver::Direct` (the default) on the same model, prescribed list and loads. Errors are over the whole displacement vector of
the result (translations in m and rotations in rad together; on these models both are of the same order, D3). The iteration count
is `SolveReport::iterations` (summed over restarts).

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| NN1 | a2 | NN models: `max_j |K b_j|_inf / (max |K| max |b_j|)` over the six columns of `structural_rigid_body_modes` on every equation, K the assembled stiffness | `<= 1e-12` each model | rounding (ADR 0432 K1 per element `1e-12`) |
| NN2 | a2 | the same columns with the rotation components zeroed: the same ratio on each NN model | `>= 1e-6` (the check sees the rotations) | `O(1)` on the rotation rows |
| NN3 | a2 | at every row with `c_r >= 3`: `B[r][j] == delta(c_r, j)` for all six `j`, exactly | exact | D1 |
| IT1 | a3, a4 | S0, each of P4a, P4b, C4, P3a, P3b, C3, BM: solve `Ok`; `|u - u_d|_2 / |u_d|_2` and `|u - u_d|_inf / |u_d|_inf` | `<= 1e-8` each | arithmetic 3: `<= 1.6e-10` |
| IT2 | a4 | S0 iterations: P4a, P4b, C4, P3a, P3b, C3, BM | `<= 2, 2, 2, 48, 57, 2, 2` | 1, 1, 1, 32, 38, 1, 1 |
| IT3 | a3, a4 | S1, each model: solve `Ok`; the two errors of IT1 | `<= 1e-8` each | `<= 1.2e-10` |
| IT4 | a4 | S1 iterations: P4a, P4b, C4, P3a, P3b, C3, BM | `<= 128, 797, 75, 75, 89, 35, 32` | 85, 531, 50, 50, 59, 23, 21 |
| IT5 | a4 | S1 levels `>= 2` and operator complexity `<= 1.5` on each model | each | 2 to 3; 1.08 to 1.45 |
| NC1 | a2 | S1, BM and C4: the CG of `rcfd_linalg::cg` (one run from zero, `tol_abs = 1e-9 |b|`, `max_iter = 5000`) preconditioned by `ElasticAmg::new` with the D1 columns, against the same with the rotation components zeroed: `it_six < it_norot` | strict, each | 21 < 46, 50 < 78 (with restarts in the prototype) |
| B1 | a5 | every existing test of `rcfd-fem` passes unedited (among them the N2 rows: `SmoothedAggregation` still refuses the three kinds, and `si_amg*.rs`); `cargo test -p rcfd-api --release --test baseline_hashes` | pass | D0 |
| B2a | a5 | `structural_rigid_body_modes` equals `rigid_body_modes` bit for bit on the free rows of the solid model | exact | D1 |
| B2b | a5 | the solid model by `SmoothedAggregation(S0)` and by `SmoothedAggregationStructural(S0)`: every displacement bit equal, iterations and restarts equal; the same with `ElasticAmgSettings::quadratic()` (the geometric aggregation) | exact | D1, D2 |
| B3 | a5 | `git diff 2cc5a1c7 -- crates/*/src` touches only `rcfd-fem/src/solve.rs`, and there removes no line (additions only) | exact | D0, D2 |
| R1 | | the new variant on the transformation path (Shell4 strip with a nodal spring) is refused with `InvalidValue { key: "solver" }` as every preconditioned CG | exact | D2 |

## Negative controls

- **NN2** is the control of NN1 (a near-null space without the rotation components is not a rigid field of a structural node).
- **NC1** is the control of a2 at the solver level (the rotation components change the coarse space and the count).
- The S0 rows of IT1 and IT2 are one-level solves on five of seven models (arithmetic 1); they check the path, not the hierarchy;
  S1 (IT3 to IT5) checks the hierarchy. Disclosed, not a control.

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `structural_rigid_body_modes`: the rotation rows left zero (the "norot" columns) | NN1; IT4 (BM 46 > 32, C4 78 > 75) |
| MU2 | the rotation row of column `3 + a` put in column `a` (rotation rows on the translation columns) | NN1, NN3 |
| MU3 | the rotation entry `-1` instead of `1` | NN1, NN3 |
| MU4 | `x_0` not subtracted (rotations about the origin; still rigid fields) | B2a, B2b |
| MU5 | `smoothed_aggregation_structural` calls `rigid_body_modes` (the old columns) | IT1, IT3 (panic, ADR 0545 MU8) |
| MU6 | the new variant refused with the kinds like the old one | IT1, IT3 |
| MU7 | the old variant's refusal removed (the old variant takes Beam2, Shell4, Shell3) | B1 (the N2 rows of the existing tests) |

MU4 changes no rigid field (a rotation about the origin is a rotation about `x_0` plus a translation), so it is caught only by the
bits; it is registered to show that B2 sees the solid-only bits. No mutation is declared equivalent.

## Decision rule

- **done:** every registered check met (NN1 to NN3, IT1 to IT5, NC1, B1, B2a, B2b, B3, R1) and every mutation caught.
- **partial:** NN1, NN3, B1, B2a, B2b and B3 met, and some IT row or NC1 missed with a diagnosed cause, or a mutation not caught,
  or a check met only after an amendment.
- **todo:** NN1 or B1 or B2 or B3 missed, or most IT rows missed without a diagnosis.

## Limits (what is not covered)

- **L1.** The existing `SmoothedAggregation` keeps refusing structures (D0); the new variant is opt-in and has no case-file key.
- **L2.** No robustness in `t/a` or mesh size is claimed: two thicknesses on fixed meshes, one forced hierarchy. The thin Shell4
  plate at S1 takes several hundred iterations (arithmetic 5); a smoother or aggregation for thin structures is not part of F-629.
- **L3.** Models mixing solids and structural nodes (blocks of 3 and 6 rows in one aggregate) are not run: a shell may not share a
  node with a solid (ADR 0432 D4), so such an aggregate needs a joint through the transformation path, where the CG is refused.
- **L4.** Only the static solve uses the multigrid; no eigen or transient path takes it.

## Addendum 1: results (2026-11-15)

Run 1, once, at cf80ecdd (`f629-scratch/run1.log`: `cargo test -p rcfd-fem --release --test sa_amg_structural -- --nocapture
--test-threads=1`, 5 tests, 0.31 s). No amendment; no check re-run. Then, at cf80ecdd: `cargo test -p rcfd-fem --release` (1057
passed, 0 failed, 286 ignored; `fem-all.log`), `cargo test -p rcfd-api --release --test baseline_hashes` (3 passed, 1 ignored),
`cargo test -p rcfd-cli --release --test docs_site` (2 passed), `cargo clippy -p rcfd-fem --release --all-targets -- -D
warnings` (clean), `cargo fmt -p rcfd-fem --check` (clean).

| Id | Limit | Measured | Met |
|---|---|---|---|
| NN1 Shell4 / Shell3 / Beam2 | `<= 1e-12` | `3.7e-16` / `3.9e-16` / `1.1e-16` | yes |
| NN2 (rotation components zeroed) | `>= 1e-6` | `5.1e-2` / `1.1e-3` / `8.3e-3` | yes |
| NN3 rotation rows exact | exact | 75 / 75 / 123 rows exact | yes |
| IT1 S0 errors (2-norm, inf-norm), seven models | `<= 1e-8` | largest `9.3e-11`, `1.6e-10` (P4b) | yes |
| IT2 S0 iterations P4a, P4b, C4, P3a, P3b, C3, BM | `2, 2, 2, 48, 57, 2, 2` | `1, 1, 1, 32, 38, 1, 1` | yes |
| IT3 S1 errors, seven models | `<= 1e-8` | largest `1.0e-10`, `1.2e-10` (P4b) | yes |
| IT4 S1 iterations | `128, 797, 75, 75, 89, 35, 32` | `85, 531 (2 restarts), 50, 50, 59, 23, 21` | yes |
| IT5 S1 levels; operator complexity | `>= 2`; `<= 1.5` | 3, 3, 2, 3, 3, 2, 3; 1.145, 1.145, 1.076, 1.187, 1.187, 1.104, 1.449 | yes |
| NC1 BM; C4 (`it_six < it_norot`) | strict | 21 < 46; 50 < 71 | yes |
| B1 existing tests unedited; baseline hashes | pass | pass; pass | yes |
| B2a modes bit for bit on 1200 solid rows | exact | equal | yes |
| B2b S0; quadratic (bits, iterations, restarts) | exact | equal (15 / 15, 0 / 0); equal (15 / 15, 0 / 0) | yes |
| B3 `git diff 2cc5a1c7 -- crates/*/src` | `solve.rs` only, additions only | `solve.rs` +78 -0 (hunks at new lines 97-100, 341-344, 348-350, 394-452, 571-578) | yes |
| R1 transformation path | `InvalidValue { key: "solver" }` | as registered | yes |

Every measured number equals the prototype's (arithmetic 3) to the printed digits: the library path forms the same matrices and
columns on these models. NC1's C4 control count is 71 (one CG run from zero, as registered) against the prototype's 78 (with
restarts); the registered comparison is the single run.

Mutations (scratch copy `f629-scratch/mut`, `target-mut`, `mutate.py`, logs `mut-MU*.log`; `solve.rs` restored and checked
byte-equal after the run):

| Id | Caught by |
|---|---|
| MU1 rotation rows zero | NN1, NN3, IT4 (C4 78 > 75, BM 46 > 32), NC1 |
| MU2 rotation rows on the translation columns | NN1, NN3, IT3 (P4b not converged), IT4, IT5 (P3 OC 1.503), NC1 |
| MU3 rotation entry `-1` | NN1, NN3, IT2 (P3a 53 > 48), IT3, IT4, NC1 |
| MU4 `x_0` not subtracted | B2a, B2b (and IT3: P4b not converged after three restarts at `1.07e-9`) |
| MU5 the old columns | IT (panic at `amg_elastic.rs:177`, as ADR 0545 MU8) |
| MU6 new variant refused | IT1, IT3 (every model) |
| MU7 old refusal removed | B1: `sa_shell3.rs::s1_n2_n3_refusals_after_f578` panics at `amg_elastic.rs:177` (cargo stopped at that binary; the `si_beam`, `si_shell` N2 rows assert the same refusal) |

Seven of seven caught; none declared equivalent.

Not run: models mixing solid and structural nodes (L3); a robustness study in `t/a` or mesh size (L2).

**Decision by the registered rule: done.** Every registered check met in the single run, every mutation caught, no amendment.

Remaining questions:

1. **Q1. Should the existing `SmoothedAggregation` take structures?** (a) Keep two variants (now). (b) Retire the N2 rows of
   `si_beam.rs`, `si_shell.rs`, `sa_shell3.rs` and RC1 of `sa_shell3_paths.rs` as ignored records with `_after_f629` copies, and
   make `SmoothedAggregation` use the structural columns (solid bits unchanged by B2a). Recommendation: (b), as a small follow-up
   the maintainer approves, since it retires the record of four test files.
2. **Q2. The thin Shell4 plate under a forced hierarchy** (531 iterations at t/a = 0.001 against 59 for Shell3). (a) Leave it
   (the default settings solve these sizes in one level). (b) A follow-up: diagnose whether the coarse operator, the block
   Gauss-Seidel or the aggregation limits it. Recommendation: (b), diagnosis first.
3. **Q3. Case-file key.** `rcfd-api` `solid_case` has no shell or beam blocks, so a key `amg_structural` would be untested now.
   Recommendation: add it with the case file's structural blocks, not before.

## For the lead: roadmap entries

Proposed status of F-629: **done**. Proposed note:

> Structural solver, advanced tier. Follow-up of F-541 (ADR 0545, 2026-11-15). ADR 0552, 2026-11-15: new opt-in rcfd_fem::solve::CgPreconditioner::SmoothedAggregationStructural (solver name cg-amg-rbm-structural) with solve::structural_rigid_body_modes: the six rigid fields u = t + w x (x - x_0), theta = w, so a rotation unknown of a Beam2, Shell4 or Shell3 node carries 1 in its rotation column; rcfd-linalg unchanged; the existing SmoothedAggregation and its refusal of the three kinds unchanged (its N2 tests unedited). Near-null fields in the kernel of the assembled stiffness to 3.9e-16 (Shell4 and Shell3 plates placed in space, Beam2; limit 1e-12). The plate and cantilever models of ADR 0432 (Shell4 16 x 16, t/a 0.01 and 0.001; strip 15 x 1) and ADR 0454 (Shell3 32 x 32; strip), and a 40-element Beam2 cantilever, solved to a true relative residual 1e-9 within 1.6e-10 of the direct solve (limit 1e-8); iterations at the default settings 1, 1, 1, 32, 38, 1, 1 (limits 2, 2, 2, 48, 57, 2, 2) and with the forced hierarchy max_coarse 60 (2 to 3 levels, operator complexity 1.08 to 1.45) 85, 531, 50, 50, 59, 23, 21 (limits 128, 797, 75, 75, 89, 35, 32). Without the rotation components the beam takes 46 and the Shell4 strip 71 iterations (21 and 50 with). Solid-only bits unchanged: the columns equal rigid_body_modes bit for bit and a Hex8 cantilever solves to the same bits by both variants (greedy and geometric aggregation); baseline hashes pass. Seven of seven mutations caught. Not covered: robustness in t/a (the thin Shell4 plate with a forced hierarchy takes 531 iterations), solid and structural nodes in one aggregate, a case-file key.

> Follow-up proposed: The existing smoothed-aggregation CG takes structures (retire the N2 multigrid rows of si_beam.rs, si_shell.rs, sa_shell3.rs and RC1 of sa_shell3_paths.rs as ignored records with copies; SmoothedAggregation uses structural_rigid_body_modes). Deps F-629. Acceptance: Every limit not stated here is registered in the ADR before the run. CgPreconditioner::SmoothedAggregation solves the ADR 0552 models with the iteration counts and errors of ADR 0552; the retired refusal tests kept as ignored records with copies; the solid-only bits and the baseline hashes unchanged.

> Follow-up proposed: Smoothed aggregation for thin plates and shells (the 531 iterations of the thin Shell4 plate with a forced hierarchy against 59 for Shell3). Deps F-629. Acceptance: Every limit not stated here is registered in the ADR before the run. The cause (coarse operator, smoother or aggregation) identified by registered controls on the ADR 0552 plate P4b; if a change is made, P4b at max_coarse 60 in at most twice the Shell3 P3b count, the other ADR 0552 counts not increased and the solid-only bits unchanged.
