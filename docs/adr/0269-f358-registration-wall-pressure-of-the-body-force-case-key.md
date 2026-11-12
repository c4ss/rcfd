# ADR 0269: F-358 registration: the wall pressure of the body force selectable in the case driver (`wall_pressure`), the callers listed, the default left to the maintainer

Status: **REGISTERED 2026-11-12**, written before any code of F-358 and before any run of its checks. Not yet accepted by
the maintainer. Before this text no check was run: the code and documents under "What was read" were read, the
arithmetic under "Arithmetic before this registration" was done by hand, and the unchanged tree of main was compiled once
(`cargo test --release -p rcfd-api --test strong_coupling_case_f350 --no-run`, to fill the target directory of the new
worktree; nothing was executed). Results go in the addendum at the end; nothing above the addendum is changed after a
run.

Parents: [ADR 0252](0252-f351-registration-added-mass-deficit-wall-pressure.md) (F-351: the cause of the 1.25 percent
deficit, `sixdof::patch_force_moment_wall_gradient`, `fsi::WallPressure`, `fsi::strong_step_with`; its questions Q1 to
Q3, from which this item comes), [ADR 0266](0266-f350-registration-strong-body-flow-coupling-case-key.md) (F-350: the
optional table `[dynamic_mesh.six_dof.coupling]`, its K0 and its piston case), [ADR
0211](0211-f244-registration-flow-coupled-six-dof-validation.md) (F-244: the spring-mounted piston SP at a mass ratio of
0.2, B1 `1.001307`), [ADR 0183](0183-exp-rot-registration.md) (P-B, the walls at rest of `ale::set_rest_conditions`),
[ADR 0216](0216-f250-registration-sliding-interface-on-a-cylinder.md) (F-250, the sliding step and `sliding::rotating_wall_conditions`).
Roadmap item F-358 (depends on F-351 and F-350).

Number: the lead gave 0269 to F-358. Branch `f358-wall-pressure-case-key` from main c740d17c, worktree `w49i-work/f358`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a missed limit becomes an
ignored `*_known_miss` test with the limit intact; a defective set-up is corrected by an amendment before the corrected
run, the old test kept as an ignored record; each registered run made once; defaults bit-identical; `baseline_hashes.rs`,
the hash tables and the goldens not edited; no existing test edited; no new crate, no unsafe code; `pimple.rs`,
`pimple/*.rs` and `overset.rs` not edited (held by another branch); `fsi.rs`, `sixdof.rs`, `ale.rs` not planned to change
(the driver calls their public functions as they are). Clean-room: nothing here is a new algorithm. The corrected wall
pressure is ADR 0252's (derived there from a Taylor expansion at the wall); the only new derivation is the error of that
formula on a skewed wall cell, below, from the normal equations of the weighted least-squares fit.

## What F-358 asks

The acceptance of `docs/roadmap.json`, binding in full:

> A case key of dynamic_mesh.six_dof selects the wall pressure of the body force ("extrapolated", the default, or
> "prescribed_gradient"), the case driver keeps the vector of ale::set_follow_conditions and hands it to
> sixdof::patch_force_moment_wall_gradient, and the spring-mounted piston of ADR 0211 run through the case driver at a
> mass ratio of 0.2 gives omega_m/omega within 0.05 percent of 1 with the key against 1.0013 without; the existing cases
> whose hashes would move if the prescribed gradient became the default are listed with the size of the change, for the
> maintainer's decision on the default; the callers that evaluate a wall force after a step with a prescribed wall
> gradient (ale::set_rest_conditions, the sliding step) are listed with the measured error on one case each, and the
> corrected evaluation is checked on a wall cell whose centre does not lie on the face normal; with the default every
> existing case hash is bit-identical.

Clauses: (a1) a key of `dynamic_mesh.six_dof` with the values `extrapolated` (default) and `prescribed_gradient`; (a2) the
driver keeps the vector of `set_follow_conditions` and hands it to `patch_force_moment_wall_gradient`; (a3) the piston of
ADR 0211 at mass ratio 0.2 through the driver: `omega_m/omega` within 0.05 percent of 1 with the key, about 1.0013
without; (a4) the existing cases whose hashes would move under the other default, listed with the size of the change;
(a5) the callers that evaluate a wall force after a step with a prescribed wall gradient, listed, with the measured error
on one case each (`set_rest_conditions`, the sliding step); (a6) the corrected evaluation checked on a wall cell whose
centre is off the face normal; (a7) with the default every existing case hash bit-identical. The lead's brief adds:
follow ADR 0266's route (an optional sub-table) and say which and why; (a4) is a measurement, both ways in a scratch
tree, the default not changed; heavy runs as ignored records with a fast test.

## What was read (main c740d17c, read only)

1. ADR 0252 in full (the cause; amendment 3's design; A1 to A5: `1.000042` at mass ratio 4, `0.999963` at 20, `1.000068`
   at 0.2, **`1.000264` for the loose loop at 0.2**, which is `strong_step_with(PrescribedGradient)` with one iteration;
   B2; Q1 to Q3). ADR 0266 in full (read 6: a plain key inside `six_dof` is filled into every case by `coerce` and would
   need an entry in H1's `DEFAULT_KEYS`; an optional table is not; C2: the strong step capped at one iteration equals the
   loose loop bit for bit). ADR 0211's SP (M6, B1 `1.001307` at `T/100`, 650 steps, 6 crossings).
2. `crates/rcfd-fv/src/sixdof.rs`: `patch_force_moment`, `patch_force_moment_wall_gradient` (a vector of the wrong
   length is an error; NaN entries fall back to the default face pressure), `patch_face_forces`; the shared
   `face_loads`. `crates/rcfd-fv/src/fsi.rs`: `strong_step` is `strong_step_with(.., Extrapolated, &mut Vec::new(), ..)`;
   with `PrescribedGradient` the predictor takes the vector on entry (default extrapolation when empty) and an iterate
   the vector of its own flow step; after a successful step the vector holds the last iterate's (in both modes).
3. `crates/rcfd-fv/src/ale.rs`: `set_follow_conditions` and `set_rest_conditions` both return the fixed-flux gradient
   `-n . (u_b,new - u_b,old)/dt` per boundary face, NaN elsewhere. `crates/rcfd-fv/src/sliding.rs`:
   `rotating_wall_conditions` returns the same for the rotor walls of the sliding step (`PimpleSolver::step_sliding`).
4. `crates/rcfd-api/src/dynmesh.rs` in full: the loose loop evaluates `patch_force_moment` at `t_n`, advances the body,
   moves the mesh, calls `set_follow_conditions`, merges the vector of `set_rest_conditions` into it (ADR 0183 P-B) and
   hands the merged vector to `step_ale_with_walls`, which consumes it; nothing keeps it. The strong path calls
   `strong_step`. `coupling_schema` (the optional table of ADR 0266), `check_coupling`, `written`.
5. **Who evaluates a wall force** (every caller of `patch_force_moment`, `patch_face_forces`,
   `rcfd_post::integrals::wall_force_and_moment*`, `least_squares` near a force):
   - the body force of the case driver, loose and strong (`dynmesh.rs`, `fsi.rs`): after a step with the follow walls'
     prescribed gradient (and, loose, the rest walls' when a rest patch is a force patch);
   - the force and moment **reports** of the case tree (`sim.rs` `ReportKind::Force | Moment | ForceCoefficient`): the
     face pressure is `Simulation::pressure_face_values`, the stored boundary value of the case's pressure condition, so
     on a moving wall with the default zero-gradient condition it is the wall-cell value itself (no gradient term);
   - `cosim::FlowWall::face_forces` (F-139, ADR 0256): already takes `WallPressure`, and its default is
     `PrescribedGradient` (it keeps the vector itself);
   - `rcfd_post::turbo` (fan moments) and `design/adjoint.rs`: static walls, no prescribed gradient;
   - the sliding step: no library or driver code evaluates a wall force after it (its tests evaluate viscous torques);
     `overset.rs` applies the wall gradients but evaluates no force.
6. **The existing six-DOF cases run through the case driver** (every case text with `motion = "six_dof"` and
   `enabled = true` that reaches a run): `rcfd-api/tests/dynamic_mesh.rs` T4 (constant-force piston, mass 0.2,
   smoothing, `box_2d` 20 x 2, `dt = 0.005`); every run of `rcfd-api/tests/strong_coupling_case_f350.rs` (the piston of
   ADR 0266 at mass ratios 4 and 0.2). The other texts with `six_dof` (`exp_rot_pab` PB2, `layering_ale_f259`,
   `tree_wiring_f259`, `rcfd-cli/tests/layering_ale_f259.rs`) assert located errors and never step. `cases/` holds no
   six-DOF case. `baseline_hashes.rs` hashes no six-DOF case. `case_hashes_w48u.rs` hashes case texts and their settings,
   not results; the `docs/case-schema.md` block of `[dynamic_mesh.six_dof.coupling]` is a fragment, not a run.
7. **Where the key can go without a hash-table edit** (ADR 0266 read 6, re-read here): a key inside the existing optional
   table `[dynamic_mesh.six_dof.coupling]` is absent from every case that does not write the table, like the table's
   other keys; `get` of it reads its schema default. So is a new sibling optional table. A plain key of `six_dof` is not.

## Arithmetic before this registration (disclosed)

- **The case of SP in the tree.** ADR 0266's piston text with the mass ratio `mu = rho A L0/M = 0.2`: `M = 0.2`,
  `k = (2 pi)^2 (M + 0.04) = 9.474820...`, `X = 0.01`, `box_3d` of `lx = 0.99` with the body at `X` (the column is
  `1 - x`), `dt = T/100`, 650 steps (6.5 periods, ADR 0211's SP), `end_time = 6.5`. ADR 0266 R1 showed that this text
  reproduces ADR 0250's SPm to `4e-7` in `omega_m/omega`; SP and SPm differ only in `M` and `k`. Expected: without the
  key `1.001307` (ADR 0211 B1, ADR 0252's model with `delta = 1/80`); with it `1.000264` (ADR 0252 A5, the loose loop
  with the corrected force, which is the coupling lag of ADR 0211 M6 alone).
- **The loose loop with the key equals `strong_step_with(PrescribedGradient)` at one iteration** (ADR 0252 read 2,
  ADR 0266 C2): the same predictor (the carried vector, or the default on the first step), the same body step, mesh
  move, follow conditions and flow step, in the same order. The library also evaluates the end-of-step force, which
  changes no state.
- **The wall cell off the face normal** (clause a6). Wall `xmin` (outward normal `n = -e_x`), the field's stored wall
  condition the default zero gradient, every other boundary value exact. `least_squares` solves `A G = sum w d dphi`,
  `w = 1/|d|^2`, so each entry adds `d^ d^T` (`d^` the unit vector) to `A`. The stored wall entry has `dphi = 0` where
  the true one is `E = p_f - p_c`, so `dG = A^-1 d^_b E/|d_b|` (with `d_b = x_f - x_c`). The corrected face pressure is
  `p_c + G . d + (g - G . n)(d . n)`; with `n = -e_x` the `G_x` terms cancel and it is `p_c + G_y d_y + G_z d_z + g (d .
  n)`: its error is the tangential part of `dG` times `d`, plus the Taylor remainder `p_nn (d . n)^2/2`.
  - *Mesh S1, a uniform shear `y' = y + gamma x`* (`gamma = 0.5`; every cell a parallelepiped, the wall cell's centre
    `gamma dx/2` off the normal of its wall face). In a wall cell of `box_mesh([N, 2, 2], [1, 0.2, 0.2])` the x
    neighbour and the wall face lie along the same unit vector `u = (1, gamma)/s`, `s = sqrt(1 + gamma^2)`; the y entries
    (one neighbour, one side face, both along `e_y`) add `2 e_y e_y^T`, the z entries `2 e_z e_z^T`. Then `A = 2 u u^T + 2
    e_y e_y^T` in the x-y block, `A^-1 u = (s/2) e_x`: **`dG` is along the normal**, its tangential part is zero, and the
    corrected evaluation is **exact for a linear field** `p = 3 (1 - x) + 0.5 y`, whatever `gamma`. The default's error
    is `dG . d = -E/2` with `E = (3 - 0.5 gamma) dx/2`; over the wall the force is low by `(3 - 0.25)/(4 x 3.05 N) =
    0.6875/(3.05 N)` (the wall's mean pressure is `3 + 0.5 x 0.1 = 3.05`): `1.12705e-2` on 20 cells. For a smooth field
    `p = cos(2x) + 3 (1 - x) + 0.5 y` the corrected error is the remainder `p_xx dx^2/8 = -dx^2/2` (relative `0.5/(4.05
    N^2)`, `3.09e-4` on 20 cells, second order); the default stays first order.
  - *Mesh S3, the first interior plane shifted* (the points of the plane `x = dx` moved by `c = kappa dx` in y,
    `kappa = 0.5`; the wall cell and its x neighbour become parallelogram prisms whose centres both move by `c/2`, so the
    x neighbour lies along `e_x` while the wall face lies along `-(1, kappa)/sigma`, `sigma^2 = 1 + kappa^2`). In the
    x-y block `sigma^2 A = [[sigma^2 + 1, kappa], [kappa, 2 sigma^2 + kappa^2]]`, determinant `D = (sigma^2 + 1)(2 sigma^2
    + kappa^2) - kappa^2 = 95/16`, and `dG = (2E/dx)(2 sigma^2, sigma^2 kappa)/D`. **The tangential part is not zero**:
    the corrected error is `dG_y d_y = -E kappa^2 sigma^2/D = -E/19`, the default's `dG . d = -E sigma^2 (2 + kappa^2)/D
    = -9E/19`. For `p = 3 (1 - x)`, `E = 1.5 dx` and the wall pressure 3: the force is low by `1/(38 N)` corrected
    (`1.3158e-3` on 20 cells) and `9/(38 N)` with the default (`1.1842e-2`). So on a wall cell whose neighbour does not lie
    along its wall face the corrected evaluation is still first order, with a coefficient nine times smaller here.
- **A wall at rest on a rigidly moving column** (clause a5, `set_rest_conditions`). `box_3d` 20 x 2 x 2 of `[1, 0.2,
  0.2]`, rigid prescribed motion `x(t) = X cos(2 pi t)`, `xmin` at rest (ADR 0183 P-B), `xmax` the outlet `p = 0`, slip
  sides. The fluid moves as a plug with the box (no relative flux crosses the wall, the sides or, by continuity, the
  outlet), so the pressure is `a (L - x_rel)` with the column length `L = 1` constant (rigid: the old and new columns
  are equal, so ADR 0252's old-column term is absent). The wall pressure is ADR 0252's piston column: the default force
  is low by `1/(4N) = 0.0125`, the corrected one by the flow part only (ADR 0252: `7e-6` on 20 cells with two
  correctors); the force **report** of the tree takes the stored zero-gradient face value `p_c`: low by `1/(2N) =
  0.025`.
- **A rotating wall of the sliding step** (clause a5). ADR 0216's rigid rotation RR (`pair(120)`, `omega = 1`, `nu = 1`,
  `dt = 5e-4`, two non-orthogonal correctors, the rotor wall `cylinder` at `r_i = 1` with `rotating_wall_conditions`).
  The exact pressure is `omega^2 r^2/2 + C`; on the rotor wall the prescribed gradient along the outward normal `-e_r` is
  `g = -omega^2 r_i = -1`, the wall-cell half-height `dn = 0.5/8/2 = 0.03125`. The wall cell's neighbours lie along its
  ray (radial) and along the circle (azimuthal, where the exact data are zero), so the fit is ADR 0252's: the default
  gradient is half the radial one and the wall pressure misses half of `g dn`: `e = (p_f - p_exact,f)/(g dn) = -0.5`;
  the corrected one misses the remainder `p_rr dn^2/2`, `e = dn/(2 r_i) = 0.016`, plus the error of the cell pressures
  (`C` is fitted as the volume mean of `p_c - omega^2 r_c^2/2`).
- **Cost.** SP: 650 loose steps of 80 cells, two runs: seconds in release. The strong run at ratio 4 with the key (1150
  steps of about 3 solves) took 19 s with ADR 0266's whole file in release. RR: 20 steps on 2112 cells.

## Design

**D1. The key.** `wall_pressure` in the optional table `[dynamic_mesh.six_dof.coupling]` (ADR 0266), values
`"extrapolated"` (default) and `"prescribed_gradient"`. It acts with either scheme: the loose loop and the strong step
evaluate the body force with it. Without the table, or with `wall_pressure = "extrapolated"`, the force is
`patch_force_moment`'s as before.

*Which table and why* (the brief): the acceptance asks for a key of `dynamic_mesh.six_dof`; a plain key there needs an
H1 `DEFAULT_KEYS` edit (read 7). Of the two optional routes, the existing coupling table is taken: the wall pressure is
an argument of the same coupling step (`strong_step_with` takes it beside `StrongCoupling`), and the table is the one
place that describes how the flow and the body exchange the force; a second table for one key would split that. The
alternative, a sibling table `[dynamic_mesh.six_dof.force] wall_pressure = ...`, is listed as a question.

**D2. The driver** (`dynmesh.rs`). `DynMesh` gains `wall_pressure: WallPressure` and `wall_dpdn: Vec<f64>`, the
fixed-flux gradient vector of the last step (empty before the first). Loose path: the force at `t_n` is
`patch_force_moment_wall_gradient(.., &wall_dpdn)` when the key is `prescribed_gradient` and the vector is not empty,
else `patch_force_moment` (the existing line); after the follow and rest conditions the merged vector is copied into
`wall_dpdn` before it is handed to the flow step (in both modes: a copy changes no number). Strong path:
`strong_step_with(.., wall_pressure, &mut wall_dpdn, ..)` in place of `strong_step` (B2 of ADR 0252: the same
operations with `Extrapolated`). `Simulation::dynamic_wall_gradients()` returns the kept vector (empty without a moving
mesh), so a caller can evaluate a wall force with it.

**D3. Static checks** (`check_coupling`): an unknown value is refused by the schema's enumeration at the key;
`wall_pressure = "prescribed_gradient"` with `motion` other than `six_dof` is refused at
`dynamic_mesh.six_dof.coupling.wall_pressure` (a prescribed motion has no body force). Nothing while
`dynamic_mesh.enabled = false`.

**D4. Documentation.** `docs/case-schema.md`: the row of the key, a sentence in the section of ADR 0266 (the keys other
than `scheme` were inert under `loose`; this one is not), a new TOML block (no existing block edited).

## Registered checks

New file `crates/rcfd-api/tests/wall_pressure_case_f358.rs`, release. `sp(mu, dt, table)` is ADR 0266's piston text
with `end_time = 11.5` (the runs step a counted number of times). "Relative" is `abs(a - b)/abs(b)`.

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| K0 (a7) | unedited: `rcfd-api` `case_hashes_w48u`, `baseline_hashes`, `w48v_adaptive_correctors`, `rules_w19c`, `mesh_transform_w48u`, `physics_schema`, `tree_wiring_f259`, `layering_ale_f259`, `registers`, `dynamic_mesh`, `exp_rot_pab`, `adapt_case_f342`, `strong_coupling_case_f350`; `rcfd-cli` `cli`; `rcfd-mcp` `default_f1`, `project_list_f270` | all pass as before (their ignored tests stay ignored), no file edited | read 7 |
| T0 (a1, a7) | the SP case without the table: no `wall_pressure` in `to_toml`, `to_json_string`; `get` reads `"extrapolated"`; with a table holding only `wall_pressure = "prescribed_gradient"`: both texts hold it, the settings hash differs | as stated | ADR 0155 |
| Z0 (a7) | `mu = 0.2`, `T/100`, 100 steps: a table with only `wall_pressure = "extrapolated"` against no table: `u`, `p`, `phi`, `x`, `v` after every step | bit-identical | the default lines |
| Z1 (a7) | `mu = 4`, `T/100`, 40 steps: ADR 0266's registered strong table with `wall_pressure = "extrapolated"` against the same table without it | bit-identical | `strong_step` is `strong_step_with(Extrapolated)` |
| E1 (a2, loose) | `mu = 0.2`, `T/100`, 100 steps, the key, loose: against `strong_step_with(PrescribedGradient)` with `Fixed(1)`, one iteration, called in the test on a second `Simulation` of the case with a vector carried by the test: the bits after every step and the kept vector (`dynamic_wall_gradients`) against the library's | bit-identical, every step | arithmetic |
| E2 (a2, strong) | `mu = 4`, `T/100`, 60 steps, the key with ADR 0266's registered strong table: against `strong_step_with(PrescribedGradient)` with ADR 0250's settings, as E1; iterations | bit-identical; iterations equal; all converged | as ADR 0266 E1 |
| W1 (a2) | the run of E1, steps 2 to 5: the kept vector has one entry per boundary face, NaN off `xmin`, and on `xmin` equals `(w_n - w_{n-1})/dt`, `w_n = (x_n - x_{n-1})/dt` of the body positions (`w_0 = 0`) | relative `<= 1e-9` | ADR 0252 W1 `3.2e-14` |
| A1 (a3) | SP (`mu = 0.2`), `T/100`, 650 steps, loose, with the key: `omega_m/omega` | `abs(. - 1) <= 5e-4` (acceptance) | `1.000264` |
| A2 (a3, "against 1.0013 without") | the same without the table | `abs(. - 1.001307) <= 1e-5`, and `. - 1 > 5e-4` (A1's limit not met without the key: the control) | `1.001307` |
| A3 | both runs: upward crossings | `>= 6` | 6 |
| A4 (the strong path, heavy) | `mu = 4`, `T/100`, 1150 steps, ADR 0266's registered strong table with the key: `omega_m/omega`; every step converged; iterations largest | `abs(. - 1) <= 1e-3` (ADR 0252 A2's limit); `abs(. - 1.000042) <= 1e-5`; all; `<= 8` | `1.000042`; 4 |
| N (D3) | `wall_pressure = "bogus"`; `"prescribed_gradient"` with `motion = "prescribed"` (the six-DOF keys removed): a diagnostic at `dynamic_mesh.six_dof.coupling.wall_pressure` each; the valid SP case with the key: none; the second text with `enabled = false`: none | as stated | D3 |
| SK1 (a6) | mesh S1 (`gamma = 0.5`), `N = 20`, linear `p = 3 (1 - x) + 0.5 y`, `u = 0`, `nu = 0`, exact fixed values on `ymin`, `ymax`, `xmax`, zero gradient on `xmin` and the z sides, the vector `g = 3` on `xmin`: relative error of the x force against `rho sum_f p(x_f) S_f`: corrected; default against `-0.6875/(3.05 N)` | `<= 1e-12`; relative to the prediction `<= 1e-9` | exact; `-1.12705e-2` |
| SK2 (a6) | mesh S1, `N = 20, 40, 80`, `p = cos(2x) + 3 (1 - x) + 0.5 y`: observed orders of the corrected and the default error | `[1.8, 2.2]`; `[0.8, 1.2]` | 2; 1 |
| SK3 (a6) | mesh S3 (`kappa = 0.5`), `N = 20`, `p = 3 (1 - x)`: relative error of the corrected and of the default force against `-1/(38 N)` and `-9/(38 N)` | relative to the prediction `<= 1e-9` each | `-1.3158e-3`; `-1.1842e-2` |
| SK0 (control of SK1, SK3) | the same evaluations on the unsheared box (`gamma = 0`, `kappa = 0`), the linear field: corrected; default against `-1/(4N)` of the x part | `<= 1e-12`; `<= 1e-9` relative | ADR 0252 U2 |
| AU1 (a5, rest walls) | the rigid column with `xmin` at rest, through the tree, `dt = 0.01`, steps 3 to 12, the worst: `d = 1 - F_x/(-rho g L A)` (`g` the kept vector on `xmin`, `L = 1`, `A = 0.04`) for the default (`patch_force_moment`), the corrected (`patch_force_moment_wall_gradient` with the kept vector) and the report (`wall_force_and_moment` with `pressure_face_values`, its pressure part) | default within 1 percent of `1/(4N)`; `abs(corrected) <= 1e-4`; report within 1 percent of `1/(2N)` | `0.0125`; `1e-5`; `0.025` |
| AU2 (a5, the sliding step) | RR, 20 steps: the mean over the rotor wall faces of `e = (p_f - p_exact,f)/(g dn)` for the default and the corrected evaluation, with the vector of the last step | default in `[-0.6, -0.4]`; corrected `abs(.) <= 0.1` | `-0.5`; `0.016` |
| L (a4, the listing, ignored record) | each case of read 6 both ways (the key absent, and the key `prescribed_gradient` on the same text): T4 of `dynamic_mesh.rs` (steps to `x >= 0.5`, its ratio `v^2/v_exact^2`, the change of `x`, `v` at the last common step); ADR 0266's S1 run (`omega_m/omega`, largest turning ratio); its L run at `T/100` (step of `10 X`, step of the stop); its Z case (`x` and `max abs p` after 100 steps); SP of A1, A2 | printed: each case's change | ADR 0252's model numbers |
| LS (a4, in the scratch tree) | a `git archive` copy where a case without the key takes `prescribed_gradient` (the default flipped in `from_case` only): K0's suites, the result of each (passes, fails with the number) | printed | `baseline_hashes`, `case_hashes_w48u` pass (no six-DOF case is hashed); `dynamic_mesh` passes; `strong_coupling_case_f350` E1, E2, R1 fail |

Printed, no limit: every number above, the turning points, the forces per step of AU1.

### Negative controls

A2 is the control of A1 (the same run without the key misses A1's limit). SK0 and SK3's default are the controls of the
corrected SK checks (the default is wrong where the corrected is right). AU1's default and report and AU2's default are
the controls of their corrected numbers.

### Mutations (scratch copy, not committed)

Each applied alone to `dynmesh.rs` of a `git archive` copy of the final tree by an exact replacement that must occur
once; the named tests run once and must fail; the file restored and compared byte for byte.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the loose path ignores the key (always `patch_force_moment`) | E1, A1 |
| MU2 | the loose path never stores the vector (`wall_dpdn` stays empty) | E1, W1 |
| MU3 | the strong path passes `Extrapolated` whatever the key | E2 |
| MU4 | the strong path hands a fresh empty vector every step | E2 |
| MU5 | the static check of a prescribed motion removed | N |
| MU6 | a case without the table takes `prescribed_gradient` | Z0, T0 |

## Predictions (mine, before any run)

A1 `1.000264`, A2 `1.001307`, A4 `1.000042`; E1, E2, Z0, Z1 bit-identical; SK1 to SK3 as derived; AU1 `0.0125`,
`< 1e-5`, `0.025`; AU2 `-0.50`, `+0.02`. LS: no hash table fails. The check I trust least is AU2 (curvature and the
field error at the wall), then SK3 (it assumes the cell centres of `FvGeometry` are the vertex means of the
parallelogram prisms).

## Decision rule

- **done**: K0, T0, Z0, Z1, E1, E2, W1, A1 to A4, N, SK0 to SK3, AU1, AU2 met; L and LS run and listed; every mutation
  caught. The acceptance stays the maintainer's, and the default stays as it is (its decision is the maintainer's).
- **partial**: K0 fails (the key is removed, the driver and its tests stay, the schema step is the maintainer's); or an
  acceptance check (A1, A2, SK1 to SK3, AU1, AU2) is missed (kept as `*_known_miss`, diagnosed); or L or LS could not be
  run; or a mutation is not caught.
- **todo**: the driver does not hand the vector to the corrected evaluation.

T0, Z0, Z1, E1, E2 are preconditions: a miss means the change is not proposed for merging.

## Open choices taken

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none changes a default.

- **T1. The key in the existing optional table** (D1), not a sibling table; the sibling is question Q2.
- **T2. The key acts in both schemes** (the loose loop is the case of the acceptance).
- **T3. The kept vector is the merged one** (follow and rest walls): the corrected evaluation then also applies to a
  rest patch that is a force patch, which is what its pressure equation used.
- **T4. The default is not changed** (ADR 0252 Q1 is the maintainer's: it moves existing results); L and LS give the
  numbers for that decision.
- **T5. The force reports are listed, not changed** (sim.rs keeps `pressure_face_values`): changing them changes
  report values of every moving-wall case; listed as a question.

## Limits of this registration

One geometry for the coupled runs (the piston column, translation only, the moment zero). The skewed-cell check is a
static evaluation on two sheared meshes, not a coupled run; a general unstructured wall cell is not tested. The sliding
step is measured on rigid rotation only (the wall moves tangentially; its prescribed gradient is the centripetal one).
The Studio's and GUI's editors are not touched (they read the schema). No case file in `cases/`.

## Amendment 1 (2026-11-12, after the one run of the registered tests, before any further run)

**The run.** `cargo test --release -p rcfd-api --test wall_pressure_case_f358 -- --nocapture --test-threads=1` on
fc193d15, once (54 s): 10 passed, 1 failed, 1 ignored (the record L, not run). **AU1 missed all three of its limits**:

| Step | `g` | default | corrected | report |
| --- | --- | --- | --- | --- |
| 3 | `-0.3915` | `4.083e-2` | `2.617e-2` | `4.963e-2` |
| 4 | `-0.3877` | `1.501e-2` | `2.254e-3` | `2.584e-2` |
| 6 | `-0.3753` | `1.323e-2` | `5.09e-4` | `2.429e-2` |
| 8 | `-0.3571` | `1.290e-2` | `2.03e-4` | `2.414e-2` |
| 12 | `-0.3041` | `1.290e-2` | `2.47e-4` | `2.437e-2` |

Limits: default within 1 percent of `1.25e-2`, corrected `<= 1e-4`, report within 1 percent of `2.5e-2`. The other
checks of that run are in the addendum; none is re-run.

**What the numbers show (no new run).** (1) A start-up part, common to the three evaluations and decaying over about
six steps (`2.6e-2` at step 3 in the corrected deficit, `5e-4` at step 6). (2) Past it, a residual of about `2e-4` in
the corrected deficit and `+3.7e-4` above `1/(4N)` in the default's. (3) The registration's identity "report minus
corrected `= dx/(2L) = 0.025`" fails by `1.1e-3` at step 8 (`2.414e-2 - 2.03e-4 = 2.39e-2`): the report is the pressure
part only, while the default and the corrected forces carry the viscous part (`nu = 0.01`), so the viscous part is about
`1e-3` of the force, against `7e-6` on ADR 0252's piston. A viscous normal stress of that size means the fluid next to
the resting wall does not move with it: the flow is not the plug the registration assumed. The registration's premise
(the fluid moves as a plug with the box, the column's pressure is `a (L - x_rel)`) is therefore unverified, and the
first-step start-up was not allowed for in the window "steps 3 to 12" (copied from ADR 0252's DP, where it was small).
Not understood from this run: why a rigid box with a resting wall carries a non-plug flow.

- **AU1 (registered form)** stays in the file as `au1_wall_at_rest_on_a_rigid_column_known_miss`, ignored, limits
  intact, the numbers in its comment.

**X1, an exploration registered here, before it is run: prints only, no limit, it decides nothing.**
`x1_rest_column_exploration_prints_only` (ignored, run once by name), 20 steps of the AU1 case and of the same case with
`algorithm = "pimple"`, `n_outer = 4` (the flow step converged), per step: the box velocity `w_n = (x(t_n) -
x(t_{n-1}))/dt` of the expression; the largest `abs(u_x - w_n)/max abs(w)` over the cells and the same over the wall
cells; the deficits of the default and the corrected evaluation **with the pressure part only** (`nu = 0` in the force)
and the report's; the viscous part of the force over the force; `s = (p_0 - p_1)/(g dx)` of the first row of cells.
What it is expected to show (guesses, written before the run): a non-plug start-up from the first step that the
converged step removes; the viscous part responsible for most of the residual.

A corrected AU1 follows in amendment 2, after X1 and before any corrected run.
