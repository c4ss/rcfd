# ADR 0252: F-351 registration: the 1.25 percent added-mass deficit of the piston column and the wall pressure of the body force

Status: **REGISTERED 2026-11-12**, written before any code of F-351 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no check was run: the code and the documents listed under "What was read" were read, the
arithmetic under "Arithmetic before this registration" was done (by hand and with a scratch script on the model problem, no
flow solver), and the unchanged tree of main was compiled once (`cargo test --release -p rcfd-fv --test strong_fsi_f251
--no-run`, to fill the target directory of the new worktree; nothing was executed). Results go in the addendum at the end;
nothing above the addendum is changed after a run.

Parents: [ADR 0250](0250-f251-registration-strong-fluid-body-coupling.md) (F-251: `rcfd_fv::fsi::strong_step`, the piston
column, checks C1 and S1, question Q4), [ADR 0211](0211-f244-registration-flow-coupled-six-dof-validation.md) (F-244: the
spring-mounted piston SP, check B1), [ADR 0033](0033-wave20b-dynamic-mesh.md) (D6 the body force, D7 the loose coupling, the
fixed-flux pressure condition of a moving wall). Roadmap item F-351 (depends on F-251 and F-244).

Number: the lead gave 0252 to F-351. Branch `f351-added-mass-deficit` from main c97f469a, worktree `w49i-work/f351`.

House rules (ADR 0130, ADR 0211, ADR 0239, ADR 0250):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written and committed before the corrected run, and the old
  one stays as an ignored record;
- the corrective design is registered in an amendment, with its derivation, after the cause is measured and before the
  corrected code and its runs;
- defaults stay bit-identical: `patch_force_moment`, `strong_step` and every existing force evaluation return exactly what
  they returned before; the corrected evaluation is a new opt-in that nothing existing calls;
- `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables and the goldens are not edited or re-recorded; no existing
  test is edited; no case-schema key is added;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no unsafe code, no commit trailers, no push.

Clean-room. Everything below is derived in this text from first principles (a Taylor expansion at the wall and the normal
equations of a weighted least-squares fit). No paper, book, code or vendor document was opened. The idea that the pressure
of a wall whose normal flux is prescribed obeys `dp/dn = -n . a_wall` (the normal momentum equation at the wall) is the
standard compatibility condition of the pressure equation of an incompressible flow (named from memory: P. M. Gresho, R. L.
Sani, "On pressure boundary conditions for the incompressible Navier-Stokes equations", Int. J. Numer. Methods Fluids 7
(1987) 1111-1145); no formula is taken from it, the repository already applies the condition (ADR 0033) and the formula
used here is derived below.

## What F-351 asks

The acceptance text of `docs/roadmap.json`, binding in full:

> The cause of the discrete added mass of the piston column lying 1.25 percent below rho A L0 (ADR 0250 C1 and S1, ADR 0211
> B1) is identified by a test that separates the wall-pressure extrapolation of patch_force_moment from the pressure of the
> flow step; if it is the extrapolation, an opt-in evaluation that uses the prescribed pressure gradient of the wall brings
> the sub-iteration gain at ratio 4 within 0.2 percent of the 2.977 of the model and omega_m/omega within 0.1 percent of 1
> on the 20-cell column, and the convergence of the deficit with the cell size is shown on 20, 40 and 80 cells; the default
> force evaluation is unchanged unless the maintainer approves the change with its effect on the existing hashes listed.

Clauses: (c1) the cause identified by a test that separates the extrapolation from the pressure of the flow step; (c2) if
it is the extrapolation, an opt-in evaluation with the prescribed wall gradient; (c3) with it the gain at ratio 4 within 0.2
percent of 2.977; (c4) with it `omega_m/omega` within 0.1 percent of 1 on 20 cells; (c5) the convergence of the deficit
with the cell size on 20, 40 and 80 cells; (c6) the default unchanged.

## What was read (no run)

1. **`patch_force_moment`** (`crates/rcfd-fv/src/sixdof.rs`, the only definition; callers: `fsi.rs`, `rcfd-api/src/dynmesh.rs`
   and tests; `rcfd-post` has none). The face pressure is `p_f = p_c + (grad p)_c . (x_f - x_c)` with `(grad p)_c =
   least_squares(geom, p.values, p.boundary_values(geom))`.
2. **`least_squares`** (`crates/rcfd-fv/src/gradient.rs`): per cell the normal equations `sum w d d^T g = sum w d dphi` over
   the neighbours and the boundary faces, **weight `w = 1/|d|^2`**, a boundary face entering with `d = x_f - x_c` and
   `dphi = p_b - p_c`, `p_b` the boundary value of the pressure field's own condition.
3. **The pressure condition of the moving wall** (`crates/rcfd-fv/src/ale.rs` `set_follow_conditions`,
   `crates/rcfd-fv/src/pimple/moving.rs` `AleStep::apply_wall_gradients`): `set_follow_conditions` returns the outward-normal
   gradient `dp/dn = -n . (u_b,new - u_b,old)/dt` per boundary face (NaN elsewhere); during the step
   `apply_wall_gradients` replaces the pressure coefficients of those faces by `p_b = p_c + dn g` (a prescribed gradient).
   This lives only in the step (`self.ale` is `None` again after it): **the pressure field's stored condition on the piston
   stays what the caller set**, in SP the default zero gradient (`strong_fsi_f251.rs` `Sp::flow` sets only `xmax`). So
   `p.boundary_values` on the piston gives `p_b = p_c` to `patch_force_moment`, after a step whose pressure equation used
   `p_b = p_c + dn g`.
4. **`strong_step`** (`crates/rcfd-fv/src/fsi.rs`): calls `patch_force_moment` for the predictor (the flow at `t_n`) and
   after the flow step of every iterate; the vector returned by `set_follow_conditions` is handed to the flow step and
   dropped.
5. **ADR 0250** in full (the model, D5, C1 `2.95223`, S1 `1.005081`, X1 `1.005968`, A1 `1.001110` and `1.001307`, Q4 and
   its untested hypothesis of a factor `0.8`, which assumed an unweighted fit) and **ADR 0211** (SP, B1 `1.001307`).

## Arithmetic before this registration (disclosed)

Column of `N` equal cells of length `dx = L/N` along x, the piston at `x = 0` (outward normal `-x`), the outlet `p = 0` at
`x = L`, a plug of acceleration `a`. Exact pressure `p(x) = a (L - x)` (kinematic), wall pressure `a L`, prescribed
outward gradient `g = a`. If the flow step is exact the cell values are `p_i = a (L - x_i)` (the discrete Laplacian with
the prescribed-gradient wall is exact for a linear field), so the wall cell holds `a (L - dx/2)`: half a cell is missing
and the evaluation at the wall has to put it back. The relative deficit of the wall pressure `delta = 1 - p_f/(a L)` for
the candidates:

| Evaluation of the wall pressure | wall-cell gradient over the true one | `delta` | `N = 20` |
| --- | --- | --- | --- |
| the cell value (zero-gradient extrapolation) | 0 | `1/(2N)` | 2.5 percent |
| cell gradient, least squares with `w = 1/d^2`, wall value `p_b = p_c` (**the code as read**) | `1/2` | `1/(4N)` | **1.25 percent** |
| the same with `w = 1/d` | `2/3` | `1/(6N)` | 0.83 percent |
| the same unweighted (the guess of ADR 0250 Q4) | `4/5` | `1/(10N)` | 0.5 percent |
| linear extrapolation from two cells; the momentum balance of the wall cell; the prescribed gradient `p_c + dn g` | 1 | 0 | 0 |

Derivation of the second row. In the wall cell the fit along x has two entries: the neighbour at `d = +dx` with `dphi =
-a dx`, and the wall face at `d = -dx/2` with `dphi = p_b - p_c = 0`. With `w = 1/d^2`: `sum w d^2 = 1 + 1 = 2`, `sum w d
dphi = (1/dx^2)(dx)(-a dx) + 0 = -a`, so `g_x = -a/2`: half the true gradient, because the weight makes the wall face
count as much as the neighbour and the wall face says the gradient is zero. Then `p_f = a (L - dx/2) + (-a/2)(-dx/2) = a
(L - dx/4)`: `delta = dx/(4L) = 1/(4N)`. The transverse directions do not couple (the cells are boxes). The general weight
`w = d^-m` gives `g_x = -a/(1 + 2^(m-2))`.

So the hypothesis is sharp: **exactly 1.25 percent for `N = 20` is what the code as read does to an exact flow-step
pressure**, and none of the other candidates gives that number.

The scratch script (`w49i-work/f351-scratch/model.py`, not committed) is the model of ADR 0250 (RK4 body with a constant
force, fluid force `-rho A (L0 - x_{n+1})(1 - delta)(w_{n+1/2} - w_{n-1/2})/dt`, the step's force converged). It gives:

| Model, `T/100` | `delta = 0` | `1/80` (`N = 20`) | `1/160` (`N = 40`) | `1/320` (`N = 80`) |
| --- | --- | --- | --- | --- |
| strong, `mu = 4`, 11.5 periods: `omega_m/omega` | 1.000042 | 1.005081 | 1.002552 | 1.001294 |
| strong, `mu = 20` | 0.999963 | 1.005968 | 1.002952 | 1.001454 |
| strong, `mu = 0.2`, 6.5 periods | 1.000068 | 1.001110 | 1.000589 | 1.000328 |
| loose, `mu = 0.2`, 6.5 periods | 1.000264 | 1.001307 | | |
| gain of the first step, `mu = 4`, `w = 0.25, 0.5, 1`, `k = 1..5`: smallest to largest | 2.97492 to 2.97704 | 2.95039 to 2.95233 | 2.96266 to 2.96468 | 2.96879 to 2.97086 |

The column `1/80` reproduces what the flow measured to the last printed digit: `1.005081` (ADR 0250 S1), `1.005968` (X1),
`1.001110` (A1 strong), `1.001307` (ADR 0211 B1 and ADR 0250 A1 loose), `2.95223` (C1). The other candidates give, at `mu =
4`: `1.010197` and `2.92762` (`delta = 1/40`), `1.003392` and `2.96056` (`1/120`), `1.002048` and `2.96715` (`1/200`). This
is a fit of existing numbers, not the separating test the acceptance asks for; the test is registered below.

## Hypotheses

Each with the measurement that confirms or rules it out, on the set-up DP below. `g` is the prescribed outward gradient of
the piston faces of the step, `L` the column length after the step, `dx = L/N`, `p_c` the pressure of the wall cells, `p_f`
the face pressure of `patch_force_moment`, rebuilt in the test from `least_squares` with the function's own formula.

- **H1 (the extrapolation).** The flow-step pressure is exact and the deficit is made by `patch_force_moment`: its
  least-squares gradient sees the stored zero-gradient wall value and gives half the gradient in the wall cell. Confirmed
  by: `r = (grad p)_c . n / g` equal to `0.5` within `1e-6`; the extrapolation part `d_ext = (p_c + g dx/2 - p_f)/(g L)`
  equal to `1/(4N)` within `1e-6`; the flow part `d_flow = 1 - p_c/(g (L - dx/2))` at most `1e-4` in magnitude; the total `d_tot
  = 1 - F_x/F_exact` (`F_exact = -rho A L g` along the outward normal, the force of the exact wall pressure) equal to
  `1/(4N)` within `2e-4`. Ruled out by `r` outside `[0.45, 0.55]` or `d_ext` outside `[0.9, 1.1]/(4N)`.
- **H2 (the pressure of the flow step).** The cell pressure next to the wall is itself low (the prescribed-gradient
  condition, the non-iterated correctors or the Rhie-Chow terms of the moving-mesh step). Confirmed by `abs(d_flow) >=
  5e-3` (at least 40 percent of the deficit); ruled out by `abs(d_flow) <= 1e-4`. Between the two both contribute and the
  shares are reported.
- **H3 (another extrapolation: zero gradient, another weight).** Ruled out beforehand by the arithmetic (2.5, 0.83, 0.5
  percent against the measured 1.25) and by the measurement `r` (0, `2/3`, `4/5` against `1/2`).
- **H4 (the viscous part of the force).** A wall-normal viscous stress from the one-sided velocity gradient. Ruled out by
  the force with `nu = 0.01` against the force with `nu = 0` on the same state: relative difference at most `1e-9`.
  Confirmed by a difference of at least `1e-3`.
- **H5 (the reference, not the force).** The wall acceleration that the flow step receives is not the model's `(w_{n+1/2} -
  w_{n-1/2})/dt`, or the piston area or the column length of the moved mesh is not `A` or `L0 - x`. Ruled out by: `g`
  against `(w_new - w_old)/dt` of the prescribed piston positions within `1e-9` relative; the piston area within `1e-12` of
  `0.04`; the distance from the piston to the outlet within `1e-12` of `L0 - x_p`; the spread of `g` and of `p_c` over the
  four piston faces at most `1e-9` relative.

Prediction: H1 holds, H2 to H5 are ruled out.

## Set-ups

**DP (prescribed piston, the separating test).** The channel of SPm (ADR 0250) with `N` cells along x: `box_mesh([N, 2, 2],
[1, 0.2, 0.2])`, `xmin` the piston, `xmax` fixed with `p = 0`, slip sides, `rho = 1`, `nu = 0.01`, Laplacian smoothing from
the initial mesh, `piso(0.01)`, central, solvers `1e-16`/`1e-13`/5000, fluid at rest. No body: the piston is at `x_p(t_n) =
X cos(2 pi t_n)`, `X = 0.01`, `dt = 0.01`, 12 steps, each step the loose sequence `move_mesh`, `set_follow_conditions`
(its vector kept), `step_ale_with_walls`. The quantities of the hypotheses are taken after each of the steps 2 to 12
(step 1 starts the wall from rest and is printed only). `N = 20, 40, 80`.

**SPm.** ADR 0250's set-up unchanged (helpers copied into the new test file, with the cell count as a parameter), strong
settings as registered there (Aitken from `2M/(2M + m_a)` with `max = 2`, tolerance `1e-8`, floor `k X`, at most 20 iterations),
`T/100`, 11.5 periods at `mu = 4` and `20`, 6.5 periods at `mu = 0.2`.

New test file `crates/rcfd-fv/tests/added_mass_f351.rs`; the heavy tests carry `#[cfg_attr(debug_assertions, ignore =
"heavy: run in release")]`. `strong_fsi_f251.rs` and `sixdof_flow_f244.rs` are not edited.

## Registered checks

"Relative" is `abs(a - b)/abs(b)`. "Corrected" is the opt-in evaluation, whose design is registered by an amendment after
the diagnosis (sketch under "Design intended").

### Diagnosis (clause c1; test code only, run before any library change)

| Id | Check (DP, `N = 20`, steps 2 to 12, the worst step) | Limit | Predicted |
| --- | --- | --- | --- |
| G1 (H1) | `r = (grad p)_c . n / g` in the wall cells | `abs(r - 0.5) <= 1e-6` | 0.5 |
| G2 (H1) | `d_ext` | `abs(. - 1/80) <= 1e-6` | 0.0125 |
| G3 (H2) | `d_flow` | `abs(.) <= 1e-4` | below `1e-5` (the model with `delta = 1/80` matches the flow to six digits) |
| G4 (H1) | `d_tot` of `patch_force_moment` | `abs(. - 1/80) <= 2e-4` | 0.0125 |
| G5 (the rebuilt `p_f` is the function's) | `rho sum_f p_f S_f` rebuilt against the x force of `patch_force_moment` | relative `<= 1e-12` | rounding |
| G6 (H4) | force with `nu = 0.01` against `nu = 0` | relative `<= 1e-9` | 0 to rounding |
| G7 (H5) | `g` against `(w_new - w_old)/dt`; area; column length; spread over the faces | `1e-9`; `1e-12`; `1e-12`; `1e-9` | rounding |
| G8 (the split adds up) | `d_tot - (d_ext + d_flow (1 - 1/(2N)))` | `abs(.) <= 1e-9` | an identity when G6 holds |

### Convergence with the cell size (clause c5)

| Id | Check | Limit | Predicted |
| --- | --- | --- | --- |
| K1 | DP, default evaluation, `N = 20, 40, 80`: `d_tot` (worst step of 2 to 12) | each within 2 percent (relative) of `1/(4N)`: `0.0125`, `0.00625`, `0.003125` | those |
| K2 | observed order `log2(d_tot(N)/d_tot(2N))`, both pairs | in `[0.95, 1.05]` | 1.000 (first order) |
| K3 | DP, corrected evaluation, `N = 20, 40, 80`: `abs(d_tot)` (worst step) | each `<= 1e-4` | below `1e-5` |
| K4 | SPm, `mu = 4`, strong, default evaluation: `omega_m/omega` on `N = 20, 40, 80` | in `[1.0049, 1.0052]`, `[1.0024, 1.0027]`, `[1.0012, 1.0014]` | 1.005081, 1.002552, 1.001294 |
| K5 | SPm, `mu = 4`, strong, corrected evaluation, `N = 40` and `80`: `omega_m/omega` | `abs(. - 1) <= 1e-3` | 1.00004 |

### Acceptance with the corrected evaluation (clauses c3, c4; `N = 20`)

| Id | Check | Limit | Predicted |
| --- | --- | --- | --- |
| A1 (c3) | first step from rest, `mu = 4`, `T/100`, `Fixed(w)`, `w = 0.25, 0.5, 1`, cap 8: gain `(1 - r_{k+1}/r_k)/w`, `k = 1..5` (ADR 0250 C1 with the corrected force) | every value within 0.2 percent of 2.977: in `[2.971046, 2.982954]` (acceptance) | 2.97492 to 2.97704 |
| A2 (c4) | strong, `mu = 4`, `T/100`, 11.5 periods: `omega_m/omega`; crossings | `abs(. - 1) <= 1e-3` (acceptance); `>= 11` | 1.000042 |
| A3 | the run of A2: every step converged; iterations largest and mean; largest ratio of successive same-side turning points | all; `<= 8`, `<= 5`; `<= 1 + 1e-3` | 4, 3.0; 0.925 |
| A4 (every mass ratio) | strong, corrected: `mu = 20`, 11.5 periods; `mu = 0.2`, 6.5 periods: `omega_m/omega` | `abs(. - 1) <= 1e-3` each | 0.999963; 1.000068 |
| A5 (the loose loop) | `mu = 0.2`, 6.5 periods, the corrected step with `max_iterations = 1`: `omega_m/omega` | `abs(. - 1) <= 1e-3` | 1.000264 (the lag of `1.5 dt`, ADR 0211) |

### Unit checks of the corrected evaluation (no flow)

| Id | Check | Limit | Predicted |
| --- | --- | --- | --- |
| U1 (order) | static box `[N, 2, 2]` of length 1, `N = 20, 40, 80`, cell pressure `p_i = cos(2 x_i) + 3 (1 - x_i)` (point values at the centres), wall `xmin`, prescribed outward gradient `g = -dp/dx(0) = 3`, the field's stored wall condition the default zero gradient: relative error of the x force against `rho A p(0)`: order of the corrected evaluation; of the default | corrected in `[1.8, 2.2]`; default in `[0.8, 1.2]` | 2.0 (error `p'' dx^2/8`); 1.0 |
| U2 (linear field) | the same with `p = 3 (1 - x)`: relative error of the corrected force; of the default against `1/(4N)` | `<= 1e-12`; `1e-12` | exact; the arithmetic above |
| U3 (fall-back) | corrected evaluation with a vector that is NaN on every face: every component of force and moment against `patch_force_moment` | bit-identical | same arithmetic |

### Defaults (clause c6)

| Id | Check | Limit |
| --- | --- | --- |
| B1 | `git diff main`: the body of `patch_force_moment` and of `strong_step` unchanged or reduced to a call that carries out the same operations in the same order; no line of `pimple.rs` or `pimple/moving.rs` touched (intended; any hunk is listed) | as stated |
| B2 | SPm, `mu = 4`, 40 steps: `strong_step` against the corrected entry point with the default wall pressure: `x`, `v` after every step, `u`, `p`, `phi` at the end | bit-identical |
| B3 | the existing suites `sixdof`, `sixdof_coupled`, `sixdof_flow_f244`, `strong_fsi_f251` of `rcfd-fv`, release, unedited | all pass |
| B4 | `cargo test -p rcfd-api --release --test baseline_hashes` (`sixdof.rs` is used by the case driver) | passes, the file unedited |

### Negative controls

| Id | Control | Must give |
| --- | --- | --- |
| N1 (of K3, A2) | DP, `N = 20`, the corrected evaluation handed a gradient of zero on the piston faces: `d_tot` | within 2 percent of `1/(2N) = 0.025` (the cell value: the term is what removes the deficit) |
| N2 (of A2) | SPm, `mu = 4`, strong, corrected evaluation handed a zero gradient (through a wrapper in the test): `omega_m/omega - 1` | `>= 5e-3` (model 1.010197) |
| N3 (of G3) | DP, `N = 20`, the flow step taken **without** the wall gradient (`step_ale`): `abs(d_flow)` against the `g` of `set_follow_conditions` | `>= 5e-3` (G3 can see a wrong flow-step pressure) |

### Mutations (scratch, not committed)

Each applied alone to the library of the final tree by an exact text replacement that must occur once; the new test file
is run once and the named checks must fail; the file is restored and compared byte for byte with a kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the corrected face pressure drops the gradient term (`p_f = p_c + grad . d` as the default) | K3, A1, A2, U1, U2 |
| MU2 | the sign of the prescribed gradient is reversed | K3, U2 |
| MU3 | the normal part of the cell gradient is not removed (`p_c + grad . d + g (d . n)`) | K3, U2 |
| MU4 | the force of an iterate is evaluated with the gradient of the previous time step, not of the iterate's flow step | A1 |
| MU5 | the default entry `strong_step` selects the corrected wall pressure | B2 |
| MU6 | the stored gradient is not updated after a step | a check of the stored vector (registered with the design) |

`scripts/limit_audit.py` is run on the new test file and every hit is read.

### After the code

`cargo test -p rcfd-fv --release` for the new test file and the suites of B3; B4; `cargo fmt -p rcfd-fv --check`; `cargo
clippy -p rcfd-fv --all-targets -- -D warnings`. The whole-workspace suite is not run (the lead's gate).

## Design intended (registered in full by an amendment after the diagnosis)

If H1 holds: a new function next to `patch_force_moment` that takes the vector of `set_follow_conditions` and, on the
faces where it is finite, replaces the normal part of the extrapolation by the prescribed one,

    p_f = p_c + (grad p)_c . d + (g - (grad p)_c . n)(d . n),    d = x_f - x_c,

which is `p_c + dn g` on a cell whose centre lies on the face normal; elsewhere the default extrapolation. A Taylor
expansion at the wall gives the error `p'' dn^2/2` (second order) against `(1 - r) g dn` of the default (first order
whenever the stored wall value is not the one the step used). And a second entry point of the strong coupling that keeps
the vector of the last flow step for the predictor. `patch_force_moment` and `strong_step` keep their signatures and
results. No case key.

## Decision rule

- **done**: the cause is identified (G1 to G8 decide one hypothesis and rule out the others, N3 holds); and, the cause
  being the extrapolation, the opt-in is delivered with A1 and A2 met, K1 to K5 met, U1 to U3 met, A3 to A5 met, B1 to B4
  met, N1 and N2 holding, every mutation caught. The acceptance stays the maintainer's.
- **partial**: the cause is identified with its number and (i) it is not the extrapolation, and no fix is delivered; or
  (ii) it is the extrapolation and an acceptance, convergence or extension check of the corrected evaluation is missed
  (kept as `*_known_miss`, diagnosed), or a mutation is not caught, or B4 could not be run.
- **todo**: the diagnosis does not decide between the hypotheses.

A set-up check (G5, G7, G8) or a negative control (N1 to N3) that does not hold means the check it guards is not shown to
be able to fail: reported as a defective set-up, corrected by an amendment before any re-run, never counted as a pass.

## Open choices taken

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none changes a default.

- **T1. The separating test prescribes the piston motion** (DP) instead of reading the pressures out of a coupled run: the
  reference force `rho A L g` then depends on nothing the coupling computes.
- **T2. The "deficit" of clause c5 is shown twice**: as the force deficit `d_tot` on DP (K1 to K3) and as the frequency of
  the coupled run (K4, K5).
- **T3. The default is not changed and the change is not proposed as taken**: if H1 holds, the first-order wall pressure of
  the default is stated with its number and listed as an open question for the maintainer.

## Limits of this registration

One geometry (a plug column, orthogonal cells, the wall normal to the motion); the corrected formula's tangential part on
a skewed wall cell still uses the cell gradient, which is not tested here. One degree of freedom; the moment only through
the unit checks. Implicit Euler flow, RK4 body. No case-tree wiring, no `rcfd-api` change. The pressure of a wall that
does not move with a prescribed flux (a fixed wall with the default zero-gradient condition) is outside: there the default
extrapolation and the pressure equation use the same wall value.

## Amendment 1 (2026-11-12, after the one run of the diagnosis tests, before any library change and before any further run)

**The run.** `cargo test --release -p rcfd-fv --test added_mass_f351 -- --nocapture --test-threads=1` on 784ea04e (test code
only), once. All three tests failed their registered limits:

| Check | Limit | Measured (worst of steps 2 to 12, DP, `N = 20`) | |
| --- | --- | --- | --- |
| G1 `r` | `0.5 +- 1e-6` | `0.499766` (step 12), `0.500126` (step 2) | missed |
| G2 `d_ext` | `0.0125 +- 1e-6` | `1.250585e-2`; over the steps `1.249686e-2` to `1.250585e-2` | missed (by `5.9e-6`) |
| G3 `d_flow` | `abs <= 1e-4` | `4.256e-4` (step 12); `1.19e-5` at step 2, rising by about `3.8e-5` a step | missed |
| G4 `d_tot` | `0.0125 +- 2e-4` | `1.292790e-2` (step 12); `1.250822e-2` at step 2 | missed (by `4.3e-4`) |
| G5 rebuilt force | `<= 1e-12` | `7.229e-6` | missed (a defect of the check, below) |
| G6 viscous part | `<= 1e-9` | `7.229e-6` (step 12); `2.7e-7` at step 2 | missed |
| G7 `g`; area; length; spread | `1e-9`; `1e-12`; `1e-12`; `1e-9` | `3.0e-14`; `6.9e-18`; `0`; `3.2e-14` | held |
| G8 identity | `<= 1e-9` | `7.136e-6` | missed (it is G6's viscous part) |
| K1 `d_tot`, `N = 20, 40, 80` | within 2 percent of `1/(4N)` | `1.292790e-2`, `6.697305e-3`, `2.330438e-3` (3.4, 7.2, 25 percent off) | missed |
| K2 orders | `[0.95, 1.05]` | `0.948836`, `1.522980` | missed |
| N3 `abs(d_flow)` without the wall gradient | `>= 5e-3` at every step | `1.58e-3` (step 2) to `7.2e-3` (step 12) | the control did not hold |

The three tests stay in the file as ignored records, the limits intact and the numbers in their comments
(`g1_to_g8_..._known_miss`, `k1_k2_..._known_miss`, `n3_..._registered_record`). Nothing was re-run.

**What the run shows, against the registered hypotheses.**

- The mechanism of H1 is there: at every step the wall-cell gradient is `0.4998` to `0.5001` of the prescribed one and
  the extrapolation part is `0.012497` to `0.012506`, within `6e-6` of `1/80`; it carries 96.7 (step 12) to 99.9 (step 2)
  percent of `d_tot`.
- H1 **as registered** ("the flow-step pressure is exact", `1e-6`, `1e-4`) is not confirmed: the flow part is `1.2e-5`
  to `4.3e-4` and grows with the piston's speed. H2 as registered (`>= 5e-3`) is not confirmed either. The registered
  text for this case applies: both contribute, shares as above.
- H4: the viscous part is `2.7e-7` to `7.2e-6` of the force, neither below `1e-9` nor above `1e-3`: present, 0.06
  percent of the deficit at most.
- H5 is ruled out (G7).

**Diagnosis of the misses from the numbers of this run (no new run).**

1. *The flow part is the column of the old mesh.* With `w` the wall velocity of the step, `d_flow` is `-w dt/(L - dx/2)`
   to within `4e-6` at the steps worked by hand (3, 4 and 12: `9.80e-5` against `1.018e-4`, `1.387e-4` against `1.420e-4`,
   `4.256e-4` against `4.292e-4`): the pressure of the implicit Euler moving-mesh step is the acceleration times the
   column length **before** the step. That follows from its momentum balance, `(V1 u1 - V0 u0)/dt + sum_f (phi_f - m_f)
   u_f = -V1 grad p`: for a plug the convective sum is `-u1 (V1 - V0)/dt` (the geometric conservation law), which leaves
   `V0 (u1 - u0)/dt`: the mass of the old volume is accelerated. This is an `O(dt)` property of the flow step in phase
   with `w a`, which changes sign four times a period and so is not a deficit of the added mass in the mean; the
   registration's reference `rho A L_new g` did not allow for it, and its premise "if the flow step is exact the cell
   values are `a (L - x_i)`" was wrong at the level of `X/L0` times `omega dt`. At step 2 the same subtraction leaves
   `-4.9e-5`: a start-up effect (step 1 starts the wall from rest).
2. *G1, G2.* `r` is, by the algebra of the fit, exactly half of `(p_0 - p_1)/(g dx)`; it deviates from `0.5` by what the
   pressure difference of the first two cells deviates from `g dx` (`2.3e-4` at most). The registered `r` mixes the
   property of the extrapolation (the factor `1/2`) with the flow field; the limit `1e-6` assumed the field exact.
3. *G5.* The check compared the rebuilt pressure force with the force of `patch_force_moment` at `nu = 0.01`, which
   contains the viscous part: its number equals G6's to six digits (`7.229399e-6`, `7.229e-6`). The rebuilt `p_f` should
   have been compared with the force at `nu = 0`. A defect of the check.
4. *N3.* A defective control by construction: where the wall flux is prescribed the pressure equation of the wall cell
   has the same flux whatever the wall coefficient, so the cell pressures barely depend on the gradient handed to the
   step; only the wall value does. N3 cannot show that G3 can fail.
5. *K1 at `N = 40` and `80`* (`+7` and `-25` percent off `1/(4N)`) is **not understood** from this run: the test printed
   only the worst step. A flow part that the old-column term does not explain must be `-8e-4` at some step on 80 cells.
   Guess (not tested): the start-up effect of item 1, larger on a finer mesh (the wall-normal viscous term, `nu dt/dx^2`
   is 0.04 on 20 cells and 0.64 on 80, with two non-iterated correctors).

**X1, an exploration registered here, before it is run: prints only, no limit, it decides nothing.** Purpose: to see the
flow part per step so that the corrected set-up and its limits can be written down before the corrected run. The test
`x1_exploration_of_the_flow_step_part_prints_only` (ignored, run once by name) prints per step of DP: `g`; `d_tot` with
`nu = 0.01` and `nu = 0` in the force; `d_ext`; `d_flow` against the new column and `d_flow_old` against the old one
(`1 - p_c/(g (L_old - dx_old/2))`); `s - 1`, `s = (p_0 - p_1)/(g dx)`; `r_alg - 0.5`, `r_alg = (grad p)_c . n dx/(p_0 -
p_1)`; the viscous part; `u_c/w - 1` of the wall cell. Runs: `N = 20, 40, 80` at `dt = 0.01`, 30 steps; `N = 20, 80` at
`dt = 0.005`, 60 steps; `N = 80` with four correctors, with four outer iterations, and with `nu = 0` in the flow. What it
is expected to show (guesses, written before the run): `r_alg - 0.5` at rounding everywhere; `d_flow_old` a few `1e-6`
after the first steps on 20 cells; a start-up part that grows with `N`, falls with more correctors or outer iterations
and vanishes with `nu = 0`.

The corrected diagnosis checks and the corrective design follow in amendment 2, after X1 and before any corrected run.
The acceptance checks A1 to A5, K4, K5, the unit checks U1 to U3, the defaults B1 to B4, N1, N2 and the mutations stand
as registered; K1 to K3 (on DP) are re-registered by amendment 2 with the reference corrected.

## Amendment 2 (2026-11-12, after the exploration X1, before any library change and before the corrected runs)

**X1, run once** (`... --test added_mass_f351 x1_exploration -- --ignored --nocapture`, on a2617baa; the full print is in
`w49i-work/f351-scratch/run2-x1.log`, not committed). It carries no limit and decides nothing; what it showed:

- `r_alg - 0.5` is `0` or `+-1.1e-16` at every printed step of every run: the factor one half is the algebra of the fit,
  whatever the field.
- With the flow step converged (`N = 80`, four outer iterations) the flow part against the **old** column, `d_flow_old`,
  is `7.8e-8` to `1.7e-6` over 30 steps, and `d_ext` is `3.1251e-3` to `3.1272e-3` (`1/320 = 3.125e-3`). With `nu = 0` in
  the flow (two correctors) `d_flow_old` is `2e-10` to `2.6e-6`. So item 1 of amendment 1 holds: the pressure of the
  converged step is the acceleration times the column before the step.
- With the registered settings (two correctors, one outer iteration) there is, on top, a part that is largest in the
  first two steps and where `g` passes through zero, and grows with the cell count: `abs(d_flow_old)` at step 2 is
  `4.8e-5` (20 cells), `2.4e-4` (40), `9.2e-4` (80); at steps 3 to 21 it is `1.2e-6` to `2.6e-5` (20), `3.4e-6` to
  `9.4e-5` (40), `1.1e-5` to `3.5e-4` (80). Four correctors reduce it at 80 cells to `1.1e-6` to `1.5e-5`; halving `dt`
  reduces it (`7.7e-6` against `3.9e-5` at `t = 0.06` on 80 cells). It is absent with `nu = 0`. So it is the splitting
  error of the non-iterated step in the wall-normal viscous term (`nu dt/dx^2` is 0.04, 0.16, 0.64), not a property of
  the force evaluation. This explains K1's numbers at 40 and 80 cells (the worst step was step 2).
- The viscous part of the force is at most `2.4e-5` of the force on 20 cells while `abs(g) > 0.1`.

**Corrected set-ups.**

- **DPc**: DP with `n_outer = 4` (the flow step converged), reference the column of the old mesh: `d_flow_old = 1 -
  p_c/(g (L_old - dx_old/2))`, `d_tot_old = 1 - F_x/(rho g L_old S_x)`; `r_alg = (grad p)_c . n dx_c/(p_0 - p_1)`, `p_1`
  the cell behind the wall cell. Steps 2 to 12, `N = 20, 40, 80`, the worst piston face and step.
- **DP** (the registered settings, which the coupled runs use), 20 cells, steps 3 to 12, the same quantities.

**Corrected checks** (they take the place of G1 to G6, G8, K1 to K3, N1 and N3 in the decision rule; G7 held and stands).
The limits are written after X1 was seen, which is disclosed: the numbers seen are in brackets.

| Id | Check | Limit | Seen in X1 |
| --- | --- | --- | --- |
| G1b (H1: the factor of the fit) | DPc, `N = 20, 40, 80`: `r_alg` | `abs(. - 0.5) <= 1e-12` | `1.1e-16` |
| G2b (H1) | DPc: `d_ext` | within 0.5 percent of `1/(4N)` | 80 cells: within 0.07 percent; 20 and 40 not seen with DPc |
| G3b (H2) | DPc: `d_flow_old` | `abs <= 2e-5` | 80 cells: `1.7e-6` |
| K1b (c5) | DPc: `d_tot_old` of `patch_force_moment` (with the viscosity of the case) | within 1 percent of `1/(4N)` on each of 20, 40, 80 | not seen |
| K2b (c5) | order `log2` of the ratio of `d_tot_old` at step 12, both pairs | in `[0.98, 1.02]` | not seen |
| G5b | the rebuilt pressure force against `patch_force_moment` with `nu = 0` | relative `<= 1e-12` | not seen |
| G6b (H4) | viscous part of the force, DPc | `<= 1e-4` of the force | `6.4e-6` (80 cells) |
| G2p, G3p | DP, 20 cells, steps 3 to 12: `d_ext`; `d_flow_old` | within 0.5 percent of `1/80`; `abs <= 1e-4` | `1.2501e-2` to `1.2506e-2`; `7.1e-6` |
| N3b (control of G3b) | DP, 80 cells, step 2 (the flow step not converged): `abs(d_flow_old)` | `>= 5e-4` | `9.2e-4` |
| K3b (c5; needs the corrected evaluation) | DPc, `N = 20, 40, 80`: `abs(d_tot_old)` of the corrected evaluation; DP, 20 cells, steps 3 to 12 | `<= 5e-5` each; `<= 1e-4` | not seen. Estimate: `dx` changes by `w dt/N` in the step, which leaves `w dt/(2 N L) = 1e-5` on 20 cells, plus the viscous part |
| N1b (control of K3b) | DPc, 20 cells: the corrected evaluation handed a zero gradient: `d_tot_old` | within 2 percent of `1/(2N) = 0.025` | not seen |

How the hypotheses are decided now: **H1** holds if G1b, G2b and K1b are met and G3b is met (the flow part carries none
of the deficit); **H2** holds if `abs(d_flow_old) >= 0.4/(4N)` on DPc and is ruled out by G3b; **H4** is ruled out as the
cause by G6b. The registered statement of H1, "the flow-step pressure is exact", is replaced by "the pressure of the
converged flow step is the acceleration times the column of the old mesh"; that the reference changed after a run is
the deviation recorded here.

**Added, because X1 showed the splitting error grows with the cell count:** K4 and K5 (the coupled runs on 40 and 80
cells with the registered two correctors) stand with their windows, run once. In addition **K4c, K5c**: the same runs on 80
cells with `n_outer = 4`, the same windows (`[1.0012, 1.0014]` default; `abs(. - 1) <= 1e-3` corrected). If K4 or K5 miss
on 80 cells and K4c, K5c are met, the miss is the splitting error of the flow step, recorded as a known miss.

The corrective design is registered by amendment 3 after these checks are run.

## Amendment 2a (2026-11-12, after the first run of the corrected diagnosis, before its corrected run)

The run (`--test added_mass_f351`, once, on 7d36f5b4): N3b held (`-9.2211e-4` against `abs >= 5e-4`). The two other tests
failed at G2b and G2p with `d_ext = 0`, on 20 cells, and the first one stopped there (40 and 80 cells were not reached).
The zero is **a defect of the test code, not a measurement**: the search for the worst `d_ext` over the faces started
from 0 and replaces its value only by one farther from `1/(4N)`, which no face value near `1/(4N)` is. Seen before the
stop, on 20 cells: G1b `-1.1e-16`, G3b `2.086e-6`, K1b `1.2509688e-2` (`+0.078` percent), G5b `0`, G6b `7.472e-6`; G3p
`7.063e-6`.

Correction: the search starts from the target `1/(4N)`. No limit, set-up or reference is changed. The two tests stay as
ignored records (`..._defective_record`, which pass the old start value) and are replaced by
`g1c_to_g6c_k1c_k2c_...` and `g2q_g3q_...`, the same checks under the same ids' limits (G1b to G6b, K1b, K2b, G2p, G3p).
N3b is not run again.

## Amendment 3 (2026-11-12, after the corrected diagnosis, before any library change): the cause, and the corrective design

**The corrected diagnosis, run once** (`--test added_mass_f351 g`, on the commit of amendment 2a; the filter `g` also
matched `n3b_...` (the letter in "unconverged"), which therefore ran a second time against the statement of amendment 2a,
with the identical number):

| Check | Limit | Measured, `N = 20`; `40`; `80` | |
| --- | --- | --- | --- |
| G1b `r_alg - 0.5` (DPc) | `abs <= 1e-12` | `-1.1e-16`; `-1.1e-16`; `-1.1e-16` | met |
| G2b `d_ext` (DPc) | within 0.5 percent of `1/(4N)` | `1.2505506e-2` (`+0.044` percent); `6.2528055e-3` (`+0.045`); `3.1264215e-3` (`+0.045`) | met |
| G3b `d_flow_old` (DPc) | `abs <= 2e-5` | `2.086e-6`; `1.593e-6`; `9.03e-7` | met |
| K1b `d_tot_old` of `patch_force_moment` (DPc) | within 1 percent of `1/(4N)` | `1.2509688e-2` (`+0.078`); `6.2578233e-3` (`+0.125`); `3.1293414e-3` (`+0.139`) | met |
| K2b orders at step 12 | `[0.98, 1.02]` | `0.999313`, `0.999802` | met |
| G5b rebuilt force against `nu = 0` | `<= 1e-12` | `0`; `0`; `0` | met |
| G6b viscous part (DPc) | `<= 1e-4` | `7.47e-6`; `6.10e-6`; `3.34e-6` | met |
| G2p, G3p (DP, 20 cells, steps 3 to 12) | within 0.5 percent of `1/80`; `abs <= 1e-4` | `1.2505851e-2` (`+0.047` percent); `7.063e-6` | met |
| N3b (control) | `abs >= 5e-4` | `9.2211e-4` | held |

**Verdict on the hypotheses.**

- **H1 holds**: the deficit is made by the wall-pressure extrapolation of `patch_force_moment`. Its least-squares
  gradient in the wall cell is exactly half of the pressure difference quotient of the first two cells (`r_alg = 0.5` to
  rounding, G1b), because the fit sees the pressure field's stored zero-gradient wall value with the weight of the
  neighbour; the wall pressure is therefore low by a quarter cell: `d_ext = 1/(4N)` within 0.05 percent on 20, 40 and 80
  cells, and the force of `patch_force_moment` is low by `1/(4N)` within 0.14 percent, first order in the cell size
  (orders `0.9993`, `0.9998`).
- **H2 is ruled out as the cause**: the pressure of the converged flow step is the acceleration times the column of the
  old mesh within `2.1e-6` (G3b; against a deficit of `1.25e-2`). Two properties of the flow step were found that are not
  the deficit: its pressure belongs to the old column (first order in `dt`, changing sign with `w a`, amendment 1); and
  with the registered two correctors the non-iterated step leaves a viscous splitting error (up to `9.2e-4` at the second
  step on 80 cells, `7.1e-6` on 20 cells past the start).
- **H3** ruled out (G1b: the factor is `1/2`, not `0`, `2/3` or `4/5`). **H4** ruled out as the cause (G6b: the viscous
  part is at most `7.5e-6` of the force, 0.06 percent of the deficit). **H5** ruled out (G7).

So the default wall pressure of `patch_force_moment` is **first order in the wall-cell size on a wall whose pressure
condition during the step (the prescribed gradient) is not the stored condition of the pressure field**: the relative
error of the force of the piston column is `1/(4N)`, 1.25 percent on 20 cells.

**The corrective design (clause c2), registered before its code.**

*Derivation.* `n` the outward unit normal of the wall face, `d = x_f - x_c` from the centre of the wall cell to the face
centre. Taylor at the cell centre: `p_f = p_c + grad p . d + O(|d|^2)`. Split `grad p . d = (n . grad p)(d . n) + (the
tangential part) . d`. On a fixed-flux wall the normal part is known: the step prescribed `n . grad p = g = -n . (u_b,new
- u_b,old)/dt` (the normal momentum equation at the wall) and its pressure equation used the wall value `p_c + dn g`. The
tangential part is taken from the cell gradient, `(grad p)_c . d - ((grad p)_c . n)(d . n)`. Hence

    p_f = p_c + (grad p)_c . d + (g - (grad p)_c . n)(d . n).

Where `d` is along `n` this is `p_c + dn g`, the wall value the step's pressure equation itself used. Its error is `p_nn
dn^2/2` (expand `p` along the normal from the wall: `p_c = p_f - g dn + p_nn dn^2/2`), second order, against `(1 - r) g
dn` of the default. On a skewed wall cell the tangential part still carries the cell gradient, which the stored wall
value contaminates through the off-diagonal terms of the fit: a limit, not tested here.

*Library* (`crates/rcfd-fv/src/sixdof.rs`, `crates/rcfd-fv/src/fsi.rs`; no line of `pimple.rs` or `pimple/moving.rs`; nothing
in `rcfd-api`; no case key; nothing existing calls the new functions):

- `sixdof::patch_force_moment_wall_gradient(geom, u, p, rho, nu, patches, x_ref, wall_dpdn)`: `wall_dpdn` is the vector of
  `ale::set_follow_conditions` (one entry per boundary face, NaN = none). On the faces with a finite entry the face
  pressure is the formula above; on the others, and in the viscous part, it is `patch_force_moment`. A vector of another
  length is an error. The body of `patch_force_moment` moves into a private function that both call; without a vector it
  carries out the same operations in the same order.
- `fsi::WallPressure::{Extrapolated, PrescribedGradient}` and `fsi::strong_step_with(flow, body, dt, coupling, strong,
  wall_pressure, wall_dpdn: &mut Vec<f64>, points)`: `strong_step` with the force evaluation chosen. With
  `PrescribedGradient` the force after the flow step of an iterate is evaluated with the vector that this flow step was
  given, and the predictor (the force of the flow at `t_n`) with the vector in `wall_dpdn`, which is the one of the
  previous step; an empty `wall_dpdn` (the first step) makes the predictor use the default extrapolation, which is the
  same for a fluid at rest. After a successful step `wall_dpdn` holds the vector of the last iterate (in both modes); a
  failed step leaves it as it was; a non-empty vector of the wrong length is an error. `strong_step` becomes
  `strong_step_with(.., Extrapolated, a scratch vector, ..)`: the same operations in the same order.

*Changes to the registered checks that the design forces* (no limit of an acceptance check is changed):

- **N2 cannot be built as registered**: the opt-in entry takes its gradient from its own flow step, not from the caller.
  It is replaced by **N2b**: the run of A2 through the same entry point with `WallPressure::Extrapolated`: `omega_m/omega
  - 1 >= 4e-3` (model `5.081e-3`): the check A2 fails without the corrected evaluation. It is the 20-cell run of K4.
- **MU6's check, W1**: SPm, `mu = 4`, corrected, the first 5 steps: after every step `wall_dpdn` has one entry per boundary
  face, NaN off the piston, and on the piston faces equals `(w_n - w_{n-1})/dt`, `w_n = (x_n - x_{n-1})/dt` of the body
  positions (`w_0 = 0`), within `1e-9` relative.
- **U4** (refusal): `patch_force_moment_wall_gradient` with a vector one entry short is an error; `strong_step_with` with
  a non-empty vector one entry short is an error and leaves the body state and the flow time unchanged.
- **B2** as registered (`strong_step` against `strong_step_with(Extrapolated)`, 40 steps, bit-identical).

A1 to A5, K3b, K4, K4c, K5, K5c, N1b, U1 to U3, B1, B3, B4 and MU1 to MU6 stand. The gradient handed to the corrected
function in N1b and K3b is the vector of the DP step (N1b: with the piston entries set to zero).

*Predictions (mine, before the code):* A1 `2.9749` to `2.9770`; A2 `1.00004`; A4 `0.99996`, `1.00007`; A5 `1.00026`; K3b
`1e-5` to `2e-5`; K4 `1.005081`, `1.00255`, `1.00129` (80 cells perhaps off by the splitting error); K5 `1.00004`; U1
`2.00` and `1.00`. The check I trust least is K4 on 80 cells.

## Amendment 4 (2026-11-12, after the mutation runs, before the run of the check it adds)

The mutations MU1 to MU6 failed their named checks (addendum). **MU4b, a variant added during the mutation runs, was not
caught**: the force of an iterate evaluated with the carried vector of the previous step whenever one exists (and with
the iterate's own vector on the first step). All 14 active tests passed with it. Reason: A1 measures the gain on the
first step, where the carried vector is empty; in a run the two vectors differ by the change of the wall acceleration
over one step, which moves the wall pressure by `(g - g_prev) dn`, about `omega dt/(2N)` of it and in quadrature, too
little for the frequency limits. But the sub-iteration is then wrong in kind: the force no longer answers the iterate's
acceleration through the wall term, so its gain falls to `1 + 1.977 (1 - 1/(2N)) = 2.928`.

**A1s (added check, limit fixed now):** SPm, `mu = 4`, 20 cells, corrected: two converged steps with the registered
settings, then the third step with `Fixed(w)`, `w = 0.25, 0.5`, cap 6, tolerance `1e-300`: the gain `(1 - r_{k+1}/r_k)/w`,
`k = 1..3`, in the window of A1, `[2.971046, 2.982954]`. Predicted `2.977` (the column is `2e-4` longer than at the
start); with MU4b `2.928`. `w = 1` is left out: from a moving state its residuals are near rounding after a few
iterations only if the iteration converges, and it does not. After A1s is run once on the unchanged library, MU4b is
applied again and A1s must fail.
