# ADR 0379: F-345 registration: BDF2 for the library's moving-mesh step and the three-level follow conditions of a moving wall

Status: **REGISTERED 2026-11-13**, written before any code of F-345 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no check was run: the code and the documents listed under "What was read" were read, the
arithmetic under "Arithmetic before this registration" was done by hand, and the unchanged tree of main was compiled once
(`cargo test --release -p rcfd-fv --test ale_gcl --test overset_f134 --no-run`, to fill the target directory of the new
worktree; nothing was executed). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0241](0241-f338-registration-overset-bdf2.md) (F-338: the BDF2 overset step, its D2 to D5 (the three-level
mesh flux, the volume corrections, the defect, the first outer iteration), questions Q4 and Q5, whose recommendation (a) is
this item; its limit L2), [ADR 0033](0033-wave20b-dynamic-mesh.md) (the ALE step, the swept volumes, G1 and G2),
[ADR 0219](0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md) (`OversetSolver`, the set-ups BF and TR,
Q0), [ADR 0258](0258-f322-registration-consistent-correctors-one-settings-value-and-extended-combinations.md) (F-322: the
rework of the corrector flags in `pimple.rs`, merged; this item comes after it), [ADR 0245](0245-f344-registration-bdf2-step-remainder.md)
and [ADR 0352](0352-f443-registration-step-consistent-face-coefficient-remaining-paths.md) (`rhie_chow_step_consistent`, which
acts on `step_ale_with_walls` and in `OversetSolver`), [ADR 0359](0359-f442-registration-overset-bdf2-motion-remainder.md)
(F-442: the two events of a moving overset coupling that leave a first-order remainder with BDF2). Roadmap item F-345
(depends on F-338 and F-322).

Number: the lead gave 0379 to F-345. Branch `f345-ale-bdf2` from main ee449e26, worktree `w49i-work/f345`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact; each registered run is made once; existing tests and records are not edited;
defaults bit-identical; `baseline_hashes.rs`, the hash tables and the goldens are not edited or re-recorded; no new crate, no
unsafe code; unsigned commits without trailers; no push; no roadmap file edited.

## What F-345 asks

Acceptance (`docs/roadmap.json`, binding in full):

> PimpleSolver::step_ale_with_walls accepts BDF2 as an opt-in (defaults bit-identical); a uniform flow stays uniform to 1e-10
> on the deforming meshes of ADR 0033 G2; with every cell calculated OversetSolver with BDF2 equals it to 1e-12 on BF of ADR
> 0219; the wall velocity and acceleration of set_follow_conditions are three-level with BDF2 and the time error of the
> cylinder case TR falls at a ratio of at least 4 between two step sizes, or the ADR names why not with a number.

Clauses (each becomes a check below):

- **C1** `step_ale_with_walls` accepts BDF2 as an opt-in; **C1d** defaults bit-identical (checks B, E).
- **C2** a uniform flow stays uniform to `1e-10` on the meshes and motions of ADR 0033 G2 with the opt-in (checks G).
- **C3** with every cell calculated, `OversetSolver` with BDF2 equals the library's BDF2 moving-mesh step to `1e-12` on BF
  (checks Q).
- **C4** the wall velocity and the wall acceleration of the follow conditions are three-level with BDF2 (checks F).
- **C5** the time error of TR with BDF2 falls at a ratio of at least 4 between two step sizes, or the ADR names why not with a
  number (checks A).

Reading of C5 (registered here): TR is the overset case of ADR 0219 (a cylinder moving through a background mesh, the motion
`s(t)` of M10). The time error at a step `dt` is `d(dt) = max abs(u_dt - u_ref)` over the cells that are not hole cells at
`t = T`, with `u_ref` the run with `dt / 4`; the ratio is `d(0.02) / d(0.01)`. With the reference four times finer a
second-order error gives `(1 - 1/16) / (1/4 - 1/16) = 5`, a first-order one `(1 - 1/4) / (1/2 - 1/4) = 3`. `T = 1` (the end of
the accelerating part of `s(t)`, where the wall acceleration is largest). The pose at `T` is the same in the three runs, so
the meshes and the cell kinds at `T` are the same.

## What was read (no run)

- ADR 0241 in full (design, amendments, addendum, questions Q4 and Q5); ADR 0219 (M9, M10, the set-ups BF and TR, Q0); ADR
  0033 (D2, D3, G1, G2); ADR 0245 and ADR 0359 (status lines, decisions and roadmap notes); ADR 0258 header; the roadmap
  entries F-338, F-344, F-345.
- `crates/rcfd-fv/src/pimple/moving.rs` in full; `crates/rcfd-fv/src/pimple.rs`: the struct, `new`, `scratch`,
  `lag_preflight_moving`, `step_with` up to the momentum predictor and the use of `ddt_defect`, the pushes at the end,
  `ddt_history_weights`, `snapshot`, `restore`, `reinitialise`; `crates/rcfd-fv/src/ddt.rs` (`History`, `add_ddt`);
  `crates/rcfd-fv/src/ale.rs` in full; `crates/rcfd-fv/src/overset.rs`: the struct, `Bdf2History`, `OldLevel`, `Bdf2Step`,
  `refusal`, `set_time_scheme`, `prepare`, `advance_mesh`, `advance`, `piso`.
- The tests `ale_gcl.rs` (G1 to G3) and `overset_f134.rs` (the set-ups, Q0); the list of tests that call
  `step_ale`/`step_ale_with_walls`/`set_follow_conditions`.

What the code read shows:

1. `step_ale_with_walls` refuses every scheme but implicit Euler ("a moving mesh supports the implicit Euler time scheme
   only"); so do `step_sliding` and `replace_mesh`. No test asserts that refusal (grep of the message: only `moving.rs` and
   the overset messages).
2. `PimpleSolver::step_with` takes the transient term from `add_ddt(settings.time_scheme, ...)` with the new volume and the
   weights `(c1, c2)` from `ddt_history_weights` (BDF2 weights only when `hist_u[0]` has an old-old level), and on a moving
   step adds `AleStep::fix_ddt` (`b += (V0 - V1) / dt u0`), takes `AleStep::defect` as the old-level Rhie-Chow defect and
   subtracts `AleStep::mesh_flux` from the convecting flux (after the first-outer-iteration extrapolation of the flux and the
   convected cell values when `c2 != 0`). With the BDF2 weights in effect on a moving step, as the code stands, the mesh
   flux, the volume correction and the defect would be the two-level ones: the opt-in has to supply the three-level ones.
3. `OversetSolver` (F-338) has a BDF2 step whose terms with every cell calculated are, face by face and cell by cell: the
   mesh flux `c0 m^{n+1} - (c2 / r) m^n`; the transient row `V c0 / dt` on the diagonal and `V / dt (c1 u^n - c2 u^{n-1}) +
   [(V^n - V) c1 u^n - (V^{n-1} - V) c2 u^{n-1}] / dt` on the right; the defect `c1 D^n_r - c2 (r^{n-1} D^{n-1})` with
   `D^n_r = interp(V^n / V) (phi^n - (u^n)_f . S^n)` (the library's two-level defect) and `D^{n-1} = phi^{n-1} -
   (u^{n-1})_f . S^{n-1}` stored raw at its own time, `r^{n-1} = w V^{n-1}_o / V_o + (1 - w) V^{n-1}_n / V_n`; the
   first-outer-iteration extrapolation `(1 + e) q^n - e q^{n-1}`, `e = c1 - 1`, of the flux and the cell values; the first
   step after the opt-in is the implicit Euler step. These are the expressions of the library's `add_ddt` and of
   `step_with`'s extrapolation, so with the same three-level data the library step can equal the overset step bit for bit.
4. `set_follow_conditions` gives a following wall the tangential part of the mean face-centre velocity over the step plus
   the normal part `m^{n+1} / |S| n` (so `u_b . S = m^{n+1}`), and the fixed-flux pressure gradient `dp/dn = -n . (u_b^{n+1}
   - u_b^n) / dt`. Both are two-level: the velocity is the value at the middle of the step (first order at `t^{n+1}`), the
   acceleration a one-step difference. With the three-level mesh flux of a BDF2 step, `u_b . S = m^{n+1}` would leave a
   relative flux `m^{n+1} - m_3` through the wall, of first order in `dt`. `OversetSolver::prepare` calls
   `set_follow_conditions`; `advance_mesh` takes the wall gradients from the caller.
5. No existing BDF2 overset test has a moving wall (grep of `overset_f338.rs`, `overset_motion_f442.rs`, `overset_f368.rs`,
   `overset_f445.rs`: no wall patch is passed to `prepare`/`step`).

## Arithmetic before this registration (disclosed, by hand)

- Weights `c0 = (1 + 2r) / (1 + r)`, `c1 = 1 + r`, `c2 = r^2 / (1 + r)`, `c2 / r = r / (1 + r)`: for `r = 1`: `3/2, 2, 1/2,
  1/2`; for `r = 2`: `5/3, 3, 4/3, 2/3`; for `r = 1/2`: `4/3, 3/2, 1/6, 1/3`.
- F2 (below), `x(t) = A t^3` along `x`, uniform steps `h`, the step ending at `t`: the mean velocity of the step is
  `A (3 t^2 - 3 t h + h^2)`, that of the step before `A (3 t^2 - 9 t h + 7 h^2)`; `(3/2)` of the first less `(1/2)` of the
  second is `A (3 t^2 - 2 h^2)`: the three-level wall velocity has the error `-2 A h^2` exactly (ratio 4 between `h` and `h /
  2`), the two-level one `-3 A t h + A h^2` (first order). The three-level acceleration `(3/2 v^{n+1} - 2 v^n + 1/2
  v^{n-1}) / h` of velocities that all carry the error `-2 A h^2` is `6 A t` exactly (the constant error cancels because `c0 -
  c1 + c2 = 0`, and BDF2 is exact for a quadratic), from the fourth step on (the first wall velocity is the two-level one);
  the one-step difference gives `6 A t - 3 A h`.
- F1, `x(t) = v0 t + a t^2 / 2`: the mean velocity of a step is the velocity at its middle exactly, so the linear
  extrapolation `c0 w^{n+1} - (c2 / r) w^n` (D6) is the velocity at `t^{n+1}` exactly for any `r`, and the three-level
  acceleration of exact velocities is `a` exactly, from the fourth step on.
- Cost. ADR 0219: TR 129 s and BF 24 s for 400 steps with one outer iteration. The A runs below take three outer iterations
  (three times the work per step) to `T = 1`: TR in `50 + 100 + 200 = 350` steps, about `350 / 400 * 129 * 3 = 340` s; BF the
  same number of steps per variant, about 63 s, four variants about 250 s. Both are heavy records (`#[ignore]`), run once;
  the fast tests below cover the same code paths in a few seconds each.

## Design

**D1. The opt-in.** New method `PimpleSolver::set_moving_mesh_time_scheme(&mut self, scheme: TimeScheme) -> Result<(),
String>`: `Bdf2` sets `settings.time_scheme = Bdf2` and a private mark; `ImplicitEuler` sets the scheme, clears the mark and
drops the moving-mesh history; every other scheme is refused, leaving the solver unchanged. `step_ale_with_walls` accepts
`settings.time_scheme = Bdf2` only with the mark (the refusal and its message are unchanged otherwise); `step_ale` follows
(it calls `step_ale_with_walls`). `step_sliding` and `replace_mesh` keep refusing BDF2 (limit L3). No field of
`PimpleSettings`, no case key. Taken as O1 (below), as F-338 did for the overset solver (ADR 0241 O1).

**D2. The three-level transient term on a moving cell and the geometric conservation law.** The momentum balance of a cell
moving with the mesh is `d/dt (V u) + sum_f (phi_f - m_f) u_f = (diffusion and pressure)`. The variable-step BDF2 of `V u` at
`t^{n+1}`, `r = dt / dt_old`, is `(c0 V^{n+1} u^{n+1} - c1 V^n u^n + c2 V^{n-1} u^{n-1}) / dt`; it is exact for quadratics in
time (second order). A uniform `u = U` with `sum_f phi_f = 0` is preserved if and only if `(c0 V^{n+1} - c1 V^n + c2 V^{n-1})
/ dt = sum_f m_f` per cell (the discrete geometric conservation law of the scheme). With `c1 = c0 + c2` the left side is
`[c0 (V^{n+1} - V^n) - c2 (V^n - V^{n-1})] / dt`, and with the swept volumes of the last two steps (`V^{n+1} - V^n = sum_f
W_f^{n+1}`, `V^n - V^{n-1} = sum_f W_f^n`, ADR 0033 G1) it is `sum_f (c0 W_f^{n+1} - c2 W_f^n) / dt`. So the mesh flux of the
BDF2 step is, face by face,

`m_f = c0 m_f^{n+1} - (c2 / r) m_f^n`, `m^{n+1} = W^{n+1} / dt`, `m^n = W^n / dt_old`

(for `r = 1`: `(3/2) m^{n+1} - (1/2) m^n`). This is the swept-volume construction of the discrete geometric conservation law
for BDF2 of Mavriplis and Yang ("Construction of the discrete geometric conservation law for high-order time-accurate
simulations on dynamic meshes", J. Comput. Phys. 213 (2006) 557-573; cited from memory, constant-step form `3/2, -1/2`); the
variable-step weights are derived above and are those of ADR 0241 D2. It is also the linear extrapolation of the two
mid-step values `m^{n+1}`, `m^n` to `t^{n+1}`, so it is the second-order value of the face's volume rate at `t^{n+1}`. With it
the conservation law holds per cell as far as it holds for each single step (to rounding).

In the matrix: `add_ddt(Bdf2)` as it is (`V c0 / dt` on the diagonal and `V / dt (c1 u^n - c2 u^{n-1})` on the right, `V`
the new volume), and in place of `fix_ddt` the correction `[(V^n - V) c1 u^n - (V^{n-1} - V) c2 u^{n-1}] / dt` on the right
(new `AleStep::fix_ddt_bdf2`, written as `Bdf2Step::add_ddt` of the overset solver writes it). `AleStep` gets one new field,
`vol_older: Option<Vec<f64>>`: `Some` on a three-level step; `None` everywhere else, where `step_with` takes `fix_ddt` as now.

**D3. The defect.** On a three-level step the old-level Rhie-Chow defect of internal face `f` is `c1 D^n_r - c2 (r^{n-1}
D^{n-1})` (item 3 above), computed in `step_ale_with_walls` with the overset solver's expressions and statement order. The
raw defect `phi^n - (u^n)_f . S^n` of each moving step is stored for the next one. With `ddt_correction` off, the defect is
zero as now.

**D4. The first outer iteration.** Unchanged code of `step_with`: on a step with the BDF2 weights the convecting flux is
`(1 + r) phi^n - r phi^{n-1}` less the (three-level) mesh flux and the convected cell values are `(1 + r) u^n - r u^{n-1}`,
which is what the overset step does with every cell calculated.

**D5. The history and the start.** The solver keeps, after each moving step taken with the opt-in, the volumes at the start
of that step, its raw mesh flux, its raw defect, its step size and the step count after it. A moving step with the opt-in is
a three-level step when that history exists and was stored by the step just before (the step count matches); otherwise it is
the implicit Euler moving step of the default, statement for statement (the scheme is set to implicit Euler for that step and
put back after it, as `step_sliding` does with `rhie_chow_step_consistent`), and it stores the history. So the first moving
step after the opt-in, and the first after a step at rest (`step`), a `restore` or a `reinitialise` (which drop the history),
is an Euler step: a local error of order `dt^2` once, which keeps the scheme of second order (the argument of ADR 0241 D5).
The weights `(c0, c1, c2)` are those of `add_ddt` (from `hist_u`'s last step size, which is the stored step size).

**D6. The three-level follow conditions.** New in `ale.rs`: `FollowHistory` (what the next step needs of this one) and
`set_follow_conditions_bdf2(u, step, old, patches, dt, &mut FollowHistory) -> Result<Vec<f64>, String>`. With an empty
history it is `set_follow_conditions` (bit for bit) and stores the history. With a history, for every face of the patches,
with `w^{n+1}` the mean face-centre velocity of this step, `w^n` that of the step before, `m^{n+1}`, `m^n` their mesh fluxes,
`n`, `|S|` the unit normal and area of the new face:

- `w_3 = c0 w^{n+1} - (c2 / r) w^n` (the linear extrapolation of the mid-step velocities to `t^{n+1}`, second order; exact for
  a velocity linear in time), `m_3 = c0 m^{n+1} - (c2 / r) m^n` (the mesh flux of D2, same expression), and the wall velocity
  `u_b^{n+1} = (w_3 - (w_3 . n) n) + (m_3 / |S|) n`: the tangential part of the extrapolated velocity and the normal part that
  carries exactly the three-level mesh flux, `u_b . S = m_3` (no relative flux through the wall in the BDF2 step, as D3 of ADR
  0033 for the Euler step);
- the acceleration is the BDF2 derivative of the wall velocity, `a = (c0 u_b^{n+1} - c1 u_b^n + c2 u_b^{n-1}) / dt`, with
  `u_b^n` the wall value before this call and `u_b^{n-1}` the one before the last call, and `dp/dn = -n . a`, the fixed-flux
  condition of the BDF2 momentum equation at the wall.

A call with other patches than the history's, or another number of boundary faces, is refused, leaving `u` unchanged.
`set_follow_conditions` and `set_rest_conditions` are not changed.

**D7. The overset solver.** `OversetSolver::prepare` with the BDF2 opt-in in effect takes `set_follow_conditions_bdf2` with a
history kept by the solver, which is emptied while the solver has no BDF2 history (so the first BDF2 step, an Euler step, has
the two-level walls) and by `set_time_scheme(ImplicitEuler)`. Without the opt-in `prepare` is unchanged. The rigid velocity
of the hole cells stays the mean over the step (L2 of ADR 0241: no calculated row uses it).

**D8. What a run that does not opt in does.** Without the mark `step_ale_with_walls` takes its present statements (the new
parts are in branches taken only with the mark and the BDF2 scheme), `AleStep::vol_older` is `None`, so `step_with` takes
`fix_ddt` as now, and the new history field stays `None`. Checked by B and E.

## Order of the work

1. This registration, committed alone.
2. The library change (`pimple.rs`, `pimple/moving.rs`, `ale.rs`, `overset.rs`) and the tests
   (`crates/rcfd-fv/tests/ale_bdf2_f345.rs`), compiled, formatted and linted without running; committed; then every registered
   check, once.
3. The mutations; the addendum; `docs/dynamic-mesh.md`.

## Registered checks

Release, Windows. PS, BF, TR are those of ADR 0219; the G2 set-up is that of `ale_gcl.rs` (mesh (a), `u0 = (1, 0.5, -0.25)`
fixed on every patch, `nu = 0.01`, central convection, the tight controls, one outer iteration). "BDF2" for the library is
`set_moving_mesh_time_scheme(Bdf2)`, for the overset solver `set_time_scheme(Bdf2)`. "Three-level walls" is
`set_follow_conditions_bdf2` with its own history per solver; "two-level walls" is `set_follow_conditions`. "3o" is three
outer iterations (`n_outer = 3`, factors 1; the overset solver `set_outer_iterations(3, 1.0, 1.0)`); "rc" is
`rhie_chow_step_consistent = true`.

### B. Defaults (preconditions of landing)

| Id | Check | Limit |
| --- | --- | --- |
| B1 | the existing test Q0 of ADR 0219, unedited (`overset_f134`), its hash | `<= 1e-12` each; prints `af2c9e6bd3e5596d` |
| B2 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass |
| B3 | the existing tests, unedited, that cover the changed lines: `ale_gcl`, `ale_flows`, `overset_f134` (non-ignored), `overset_f134_records`, `overset_f335_records`, `overset_f338` (non-ignored), `overset_motion_f442` (non-ignored), `bdf2_remainder_f344` (non-ignored), `sliding_interface_f250`, `layering_f257`, `sixdof_coupled`, `msd_wall_f332`, `added_mass_f351`, and the `rcfd-fv` unit tests (`--lib`) | pass |

### E. The opt-in and the refusals

| Id | Check | Limit |
| --- | --- | --- |
| E1 | without the opt-in, `settings.time_scheme = Bdf2`, `step_ale_with_walls` | `Err` with the present message; `u`, `p`, `phi`, time unchanged bit for bit |
| E2 | `set_moving_mesh_time_scheme` with the bounded BDF2 and with Crank-Nicolson | `Err` each; settings unchanged |
| E3 | with the opt-in: `step_sliding` (any step data) and `replace_mesh` | `Err` each |
| E4 | `set_follow_conditions_bdf2` with another patch list than its history | `Err`; the patch conditions of `u` unchanged |
| E5 | the lid cavity of `ale_gcl.rs` G3 on a translating mesh (8 x 8, `dt = 0.02`), opt-in: two moving steps, one step at rest, one moving step; against the same with `set_moving_mesh_time_scheme(ImplicitEuler)` and `(Bdf2)` called before the last step | equal bit for bit (the history is dropped by a step at rest) |
| E6 | the same cavity, opt-in followed by `set_moving_mesh_time_scheme(ImplicitEuler)`, 5 moving steps, against the default run | equal bit for bit |
| E7 | the same cavity with `settings.time_scheme = Bdf2`: one step at rest, then one moving step with the opt-in; against one step at rest with BDF2 and one default implicit Euler moving step (`time_scheme` set to implicit Euler before it, no opt-in) | equal bit for bit (the first moving step of a history is the default Euler step) |

### G. A uniform flow on the deforming meshes of ADR 0033 G2 (C2)

| Id | Check | Limit |
| --- | --- | --- |
| G2b | the four motions of G2 (translation, rotation, oscillation, expansion), BDF2, 10 steps of `0.05`: every step, `max abs(u_c - u0) / abs(u0)` and `max abs(p_c) / abs(u0)^2` | `<= 1e-10` each |
| G2v | the same with the steps alternating `0.05` and `0.025` (`r` is `1/2` and 2; the motion evaluated at the time reached) | `<= 1e-10` each |
| G2r | expansion, BDF2: one step at rest first, then 10 moving steps of `0.05` | `<= 1e-10` each |
| G2n (negative control) | expansion, BDF2, the mesh flux of every step multiplied by 1.01 before `step_ale_with_walls`: `max abs(u_c - u0) / abs(u0)` | `>= 1e-6` |
| G2s (set-up) | the expansion run of G2b: the largest over the steps of `max_c abs(V^{n+1} - V^n) / V^n` and of `max_f abs(m^{n+1} - m^n) / max abs(m)` | `>= 1e-2` each (the volumes change and consecutive mesh fluxes differ) |

### Q. The overset step with every cell calculated against the library's BDF2 moving-mesh step on BF (C3)

BF for 20 steps of `0.02`, `OversetSolver::advance_mesh` (no coupling) against `PimpleSolver::step_ale_with_walls`, both
with BDF2 and three-level walls (each solver its own history; the same points every step). After the 20 steps: the largest
difference of `u` (over `U = 1`), `p` (over `U^2`) and `phi` (over `max abs(phi)`); printed: equal bit for bit.

| Id | Check | Limit |
| --- | --- | --- |
| QB1 | PS (one outer iteration) | `<= 1e-12` each |
| QB3 | PS with 3o | `<= 1e-12` each |
| QB3c | PS with 3o and rc | `<= 1e-12` each |
| QBn (negative control) | the library run of QB1 against the overset solver without BDF2 (implicit Euler, two-level walls): the largest difference of `u` | `>= 1e-8` |
| QR | the 16 x 16 lid cavity at rest, `dt = 0.02`, 20 steps: `step_ale_with_walls` with the points unchanged and the opt-in against `step` with BDF2 (`u`, `p`, `phi`) | `<= 1e-12` each; printed bit for bit |

### F. The three-level follow conditions (C4)

Mesh `cylinder_ogrid(24, 6, 0.5, 1.5, Uniform, 0.1)`, wall patch `cylinder`, moved rigidly; no flow solve.

| Id | Check | Limit |
| --- | --- | --- |
| F1 | translation `x(t) = (0.3 t + 0.8 t^2, -0.2 t + 0.5 t^2, 0)`, 8 steps of `0.01`, and 8 steps alternating `0.01`, `0.005`: at steps 4 to 8, every wall face, `abs(u_b - x'(t^{n+1})) / max abs(x')` and `abs(dp/dn + n . x'') / abs(x'')` | `<= 1e-11`; `<= 1e-8` |
| F2v | translation `x(t) = (t^3, 0, 0)`, uniform steps `h = 0.02` (20 steps) and `0.01` (40 steps) to `T = 0.4`: `err(h) = max abs(u_b - x'(T))`; `err(0.02) / err(0.01)` | `>= 3.9` |
| F2e | the same: `abs(err(h) / (2 h^2) - 1)` for both `h` (the closed form of the arithmetic) | `<= 1e-6` |
| F2a | the same: `max abs(dp/dn + n . x''(T)) / abs(x''(T))` for both `h` | `<= 1e-8` |
| F3 | rotation about the cylinder axis by `0.5 t + 2 t^2` and translation `(0.4 t + t^2, 0.3 t, 0)`, 8 steps of `0.01`: at steps 2 to 8, `max_f abs(u_b . S_f - m_3,f) / max_f abs(m_3,f)`, `m_3` from the two consecutive `move_mesh` fluxes | `<= 1e-12` |
| F4 | the first call (empty history) against `set_follow_conditions` on a copy: the wall values and `dp/dn` | equal bit for bit |
| F5 | TR with BDF2 (`set_time_scheme(Bdf2)`), 4 steps of `0.02` by `step`: after each `prepare`, the wall values installed on `c_cylinder` and the returned `wall_dpdn` against `set_follow_conditions_bdf2` on a copy of `u` taken before the `prepare` with a history kept by the test (fed the same mesh steps) | equal bit for bit, every step |
| F5d (defaults) | the same without BDF2 against `set_follow_conditions` | equal bit for bit, every step |

### A. The time error of BF and TR (C5; heavy records, each run once)

`d(dt)` as in "Reading of C5", `T = 1`, the runs with `dt = 0.02, 0.01, 0.005` (50, 100, 200 steps); `R = d(0.02) / d(0.01)`.
BF: the largest over all cells; TR: over the non-hole cells at `T`. Both PS with 3o and rc.

| Id | Check | Limit |
| --- | --- | --- |
| A-BF | BF with the library's BDF2 and three-level walls: `R` | `>= 4` |
| A-BFn (negative control) | BF with implicit Euler, two-level walls (3o, rc): `R` | in `[2.5, 3.5]` |
| A-TR (acceptance C5) | TR with BDF2 (three-level walls through `prepare`, D7): `R` | `>= 4` |
| A-c | every step of the BDF2 runs: the continuity of the calculated cells (the library's and the overset solver's measure) | `<= 1e-9` |

Printed, no limit: `d(0.02)`, `d(0.01)` of every run; BF with BDF2 and two-level walls (the wall term alone) and BF with BDF2
without rc: their `R`; TR: `d` and `R` over the component cells and over the background cells, the cell and the distance from
the cylinder centre of the largest difference, `F_x` of the cylinder at `T` in the three runs, the number of cells of TR that
took the Euler row (`euler_fallback_cells`), the run times.

**H1 (registered reading of a miss of A-TR).** If A-TR is missed and A-BF is met, the BDF2 moving step and the three-level
walls of this item are of second order on the same cylinder, motion and wall with every cell calculated, and the remainder of
TR is made by the moving overset coupling: the two events named by ADR 0359 (donor switches of the component's fringe, kind
changes of the background next to the hole), which no change of this item touches. The number named is `R` of A-TR against
`R` of A-BF. If A-BF is missed too, the cause is not located by this registration.

### Mutations (scratch copy, not committed)

Each applied alone to a `git archive` copy of the final tree by an exact replacement that must occur once; the named test run
once and must fail; the file restored and compared byte for byte.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MB1 | the library's mesh flux of a three-level step is the one-step flux `m^{n+1}` | G2b |
| MB2 | `fix_ddt_bdf2` without the old-old volume part | G2b |
| MB3 | the old-old part of the library's three-level defect left out | QB1 |
| MB4 | no implicit Euler step without a history (the scheme not switched for that step) | E7 |
| MB5 | the history kept across a step at rest (no step-count test) | E5 |
| MB6 | the three-level wall velocity replaced by the mean velocity of the step (`w^{n+1}`) | F1 |
| MB7 | the wall acceleration a one-step difference `(u_b^{n+1} - u_b^n) / dt` | F2a |
| MB8 | the normal part of the three-level wall velocity from the one-step mesh flux `m^{n+1}` | F3 |
| MB9 | `OversetSolver::prepare` with BDF2 takes `set_follow_conditions` | F5 |

### Classes

B1, B2, B3, E1 and E6 are preconditions of landing: a miss means the change is not merged and the cause is recorded. A miss of
any other limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. A set-up check
or negative control (G2s, G2n, QBn, A-BFn) that does not hold, or a mutation that does not make its check fail on the first
run, is a defective set-up: corrected by an amendment before any re-run, never counted as a pass.

## Predictions (mine, before any run)

B, E: as stated. G2b, G2v, G2r: near `1e-14`. G2n: about `1e-3`. QB1, QB3, QB3c: equal bit for bit (the expressions are
the overset solver's); QR equal bit for bit. F1: `1e-13`, `1e-11`; F2v `4.000`; F2e below `1e-9`; F2a `1e-11`; F3 `1e-15`.
A-BF: between 4.3 and 5.2; A-BFn: about 3.0. **A-TR: missed**, between 1.8 and 3.5 (the coupled remainder of ADR 0359; TR has
hundreds of kind changes in the first time unit); H1 would then apply.

## Decision rule

- **done**: every registered check met (A-TR among them), the preconditions hold, the controls hold, every mutation failed
  its check on the first run.
- **partial**: the opt-in and the three-level walls are built and the preconditions hold, but A-TR is missed (whether H1
  names the cause with a number or not: the note says which; the acceptance's "or the ADR names why not with a number" is
  then met only if H1 holds, and the item is still proposed partial because a measured clause was missed), or another
  registered check is missed, or a control or mutation did not hold and was not corrected.
- **todo**: the opt-in could not be built or a precondition does not hold (nothing to merge).

## Open choices taken

Each is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none is a decision the
maintainer made in person; none changes a default.

- **O1. Where the opt-in lives.** (a) accept `settings.time_scheme = Bdf2` in `step_ale_with_walls` directly: a case or a
  caller that sets BDF2 and moves the mesh, refused today, would run, which changes the result of an existing input; (b) a
  method of the solver with a private mark (D1), as `OversetSolver::set_time_scheme`; (c) a field of `PimpleSettings`.
  Taken: (b). (c) is for the case-tree wiring (not part of this item).
- **O2. The wall acceleration.** (a) the BDF2 derivative of the wall velocities (D6), the derivative the BDF2 momentum
  equation takes at the wall; (b) the derivative of the motion evaluated at `t^{n+1}` from the face centres alone. Taken: (a).
- **O3. The three-level walls in `OversetSolver::prepare`.** (a) with the BDF2 opt-in in effect (D7): the BDF2 overset run
  with a moving wall changes, which no existing test has (item 5 of "What was read"); the old values were the defect named by
  ADR 0241 L2; (b) a further opt-in. Taken: (a); disclosed: an F-338 BDF2 run with a moving wall gives other numbers after
  this item.
- **O4. A step at rest between moving steps** drops the history (the next moving step is an Euler step, D5), rather than
  counting as a moving step with zero swept volume (the overset solver's reading). Taken because the library's `step` keeps
  no volumes; the cost is one Euler step, of local order `dt^2`.

## Limits of this registration

- L1. The checks run with PS (central convection, two correctors, no non-orthogonal correctors) and the options named; BDF2
  on a moving mesh with the consistent correctors, the pressure-lag check, a body force, a face viscosity or periodic faces is
  not refused and not checked.
- L2. `set_rest_conditions` (a wall at rest on a moving mesh) keeps the two-level values; the rigid velocity of overset hole
  cells stays the mean of the step.
- L3. No BDF2 for `step_sliding` and `replace_mesh`; no case key, CLI or Studio wiring; no restart file carries the moving
  history (a `restore` drops it).
- L4. The time error of TR is measured at one time (`T = 1`) and by the velocity; the force is printed only.

## Open questions

None before the runs.

## Addendum 1: results (2026-11-13)

Measured against the limits registered above; none was changed, no amendment was needed. Release builds on Windows, the
worktree's target directory, other agents building on the machine. Commits: the registration `f679b180`; the library change
and the tests, compiled, formatted and linted and not run, `8d780068`; this addendum with the rename of the missed record
and `docs/dynamic-mesh.md`. Each registered run was made once. Logs in `w49i-work/f345-scratch` (`fast.log`, `a_bf.log`,
`a_tr.log`, `b3.log`, `lib.log`, `baseline.log`, `clippy.log`, `mutations.log`).

**Said plainly.** The library's moving-mesh step takes BDF2 as an opt-in, the overset step with every cell calculated
equals it bit for bit on BF, a uniform flow stays uniform to `7e-15` on the G2 meshes, and the three-level walls are what
makes the body-fitted cylinder second order in time (ratio 5.04; 3.04 with BDF2 and the old walls). **The acceptance
number on TR is missed:** the time error of the overset cylinder does not fall with the step (ratio 1.09 against 4). It sits
in the background cells next to the hole; the component cells fall at 4.22. H1 applies: the cause is the moving coupling
(the kind changes named by ADR 0359), not the step or the walls of this item.

### What was built

- `crates/rcfd-fv/src/pimple/moving.rs`: `PimpleSolver::set_moving_mesh_time_scheme`, `AleStep::vol_older`,
  `AleStep::fix_ddt_bdf2`, the private `AleHistory`, and in `step_ale_with_walls` the BDF2 branch (D2, D3, D5); the default
  branch keeps its statements. `step_sliding` sets `vol_older: None`.
- `crates/rcfd-fv/src/pimple.rs`: two private fields (`ale_bdf2_opt_in`, `ale_bdf2`) set in `new` and `scratch`, the
  call site of the transient correction (`fix_ddt_bdf2` only when `vol_older` is set), the history dropped in `restore` and
  `reinitialise`.
- `crates/rcfd-fv/src/ale.rs`: `FollowHistory`, `set_follow_conditions_bdf2` (D6).
- `crates/rcfd-fv/src/overset.rs`: `prepare` with BDF2 takes the three-level walls (D7), the history cleared by
  `set_time_scheme(ImplicitEuler)`; `AleStep` gets `vol_older: None`.
- Tests: `crates/rcfd-fv/tests/ale_bdf2_f345.rs` (8 run by the gate, 2 ignored heavy records). No existing test edited.

### Checks

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| B1 Q0 of ADR 0219 (unedited) | `<= 1e-12` each; hash | `0`, `0`, `0`; bit for bit; `af2c9e6bd3e5596d` | met |
| B2 `baseline_hashes` | pass | 3 passed (1 ignored) | met |
| B3 existing tests (13 test files and `--lib`) | pass | all passed (`--lib` 104) | met |
| E1 BDF2 without the opt-in | `Err`, message, state unchanged | as stated | met |
| E2 bounded BDF2, Crank-Nicolson | `Err`, unchanged | both | met |
| E3 `replace_mesh`, `step_sliding` with the opt-in | `Err` each | both | met |
| E4 another patch list | `Err`, `u` unchanged | as stated | met |
| E5 history dropped by a step at rest | bit for bit | equal | met |
| E6 opt-in and back = default | bit for bit | equal | met |
| E7 first moving step = default Euler step | bit for bit | equal | met |
| **G2b** four motions, uniform steps, `u`, `p` | `<= 1e-10` each | at most `5.1e-15`, `5.7e-15` | met |
| **G2v** alternating steps | `<= 1e-10` each | at most `7.2e-15`, `8.1e-15` | met |
| G2r a step at rest first | `<= 1e-10` each | `3.9e-15`, `5.1e-15` | met |
| G2n (negative control) flux times 1.01 | `>= 1e-6` | `9.86e-3` | held |
| G2s (set-up) volume and flux change | `>= 1e-2` each | `0.455`, `1.0` | held |
| **QB1** BF, one outer iteration | `<= 1e-12` each | `0`, `0`, `0`; bit for bit | met |
| **QB3** three outer iterations | `<= 1e-12` each | `0`, `0`, `0`; bit for bit | met |
| **QB3c** with rc | `<= 1e-12` each | `0`, `0`, `0`; bit for bit | met |
| QBn (negative control) | `>= 1e-8` | `1.46e-2` | held |
| QR at rest against `step` with BDF2 | `<= 1e-12` each | `0`, `0`, `0`; bit for bit | met |
| **F1** constant acceleration: velocity; acceleration | `<= 1e-11`; `<= 1e-8` | `4.7e-14` / `8.9e-14`; `3.1e-12` / `6.7e-12` (uniform / alternating) | met |
| **F2v** ratio | `>= 3.9` | `4.0000` | met |
| F2e closed form `2 h^2` | `<= 1e-6` | `8.4e-12`, `3.6e-11` | met |
| **F2a** acceleration exact | `<= 1e-8` | `5.3e-13`, `8.5e-13` | met |
| F3 relative wall flux | `<= 1e-12` | `3.1e-16` | met |
| F4 first call = `set_follow_conditions` | bit for bit | equal | met |
| F5 `prepare` with BDF2 | bit for bit | equal, 4 steps | met |
| F5d `prepare` without BDF2 | bit for bit | equal, 4 steps | met |
| **A-BF** BF, BDF2, three-level walls | `>= 4` | `5.041` (`d = 6.21e-4`, `1.23e-4`) | met |
| A-BFn (negative control) implicit Euler | in `[2.5, 3.5]` | `2.971` (`9.39e-3`, `3.16e-3`) | held |
| **A-TR (acceptance)** TR, BDF2, three-level walls | `>= 4` | **`1.088`** (`2.299e-3`, `2.112e-3`) | **missed** (known-miss record) |
| A-c continuity, every BDF2 run | `<= 1e-9` | BF `5.4e-16`, TR `5.2e-17` | met |

Printed, no limit. BF with BDF2 and the **two-level walls**: `R = 3.037` (`7.67e-3`, `2.52e-3`): the old walls alone leave a
first-order error twelve times the three-level one. BF with BDF2, three-level walls and no rc: `R = 4.897` (`6.29e-4`,
`1.29e-4`). `F_x` of BF at `T` with BDF2: `0.120614`, `0.120500`, `0.120472`. TR: component cells `R = 4.219` (`6.44e-4`,
`1.53e-4`), background cells `R = 1.088` (`2.30e-3`, `2.11e-3`); the largest difference in background cell 9570 at `1.108`
from the cylinder centre (the hole radius is 1, so on the first ring of cells outside it); `F_x` of TR at `T`: `0.120285`,
`0.120452`, `0.120457`; no cell took the Euler row (`euler_fallback_cells` 0 in every run). Run times: BF 52 to 99 s per
variant, TR 61, 118, 200 s.

### H1

A-BF is met (`5.04`) and A-TR is missed (`1.09`). By the reading registered, the BDF2 moving step and the three-level walls
are of second order on the same cylinder, motion and wall with every cell calculated, and the remainder of TR is made by
the moving overset coupling. The split by class agrees: the component cells, which never change kind, fall at `4.22`; the
background difference is almost independent of the step and largest one cell outside the hole, where cells change kind
as the cylinder passes (ADR 0359 event 2). The number named is `R(TR) = 1.09` against `R(BF) = 5.04`, with the component
cells at `4.22`. This item does not touch the coupling.

### Mutations

Applied alone to a `git archive` copy of `8d780068` (`f345-scratch/mut`, its own target directory) by `mut_run.py` (an exact
replacement that occurs once), the named test run once, the file restored and compared byte for byte (equal in all nine).
All nine caught on their first run.

| Id | Must fail | Result |
| --- | --- | --- |
| MB1 one-step mesh flux | G2b | failed: expansion `u` `0.89`, `p` `2.08` |
| MB2 no old-old volume | G2b | failed: `0.48`, `7.7` |
| MB3 no old-old defect | QB1 | failed: `u` `1.9e-4`, `p` `1.3e-2`, `phi` `4.3e-3` |
| MB4 no Euler step without a history | E7 | failed: not bit for bit |
| MB5 history kept across a rest step | E5 | failed: not bit for bit |
| MB6 wall velocity the step mean | F1 | failed: `2.4e-2` (and F2) |
| MB7 one-step wall acceleration | F2a | failed: `2.5e-2`, `1.2e-2` |
| MB8 normal part from the one-step flux | F3 | failed: `2.0e-2` (and F1, F2) |
| MB9 `prepare` with the two-level walls | F5 | failed: not bit for bit |

`scripts/limit_audit.py` on the new test file: 0 hits.

### Deviations and disclosures

- The record of A-TR was renamed after its run to `a_tr_time_error_ratio_known_miss`, its limit intact and the number in
  its ignore reason; nothing else in it changed.
- Q0 of ADR 0219 ran twice (in the B3 suite and once more with `--nocapture` to print its hash); deterministic.
- My prediction of A-TR (between 1.8 and 3.5) was wrong: it is `1.09`. The other predictions held.
- Clippy ran once with `--release` before the commit of the code and once in the house form after the runs; both clean.
- The registration's status line says 2026-11-13 (the date of this work); the house rules name 2026-11-12.

### Not run

The whole workspace suite and the gate (the lead's); the other ignored records of the existing files; Linux; TR with
implicit Euler (A-BFn stands for it on BF); `set_rest_conditions` and `step_sliding` with BDF2 (not built, L2, L3).

### After the tests

`cargo fmt -p rcfd-fv --check` clean. `cargo clippy -p rcfd-fv --all-targets -- -D warnings` clean.

### Decision by the registered rule

**Partial.** The opt-in and the three-level walls are built, the preconditions hold, the controls held and every mutation
failed its check on the first run; every check of C1 to C4 is met. A-TR, the measured number of C5, is missed (`1.09`
against 4), and H1 names why with numbers (`R(BF) = 5.04`, the component cells of TR `4.22`, the background next to the
hole `1.09`). By the rule the acceptance's "or the ADR names why not with a number" is met, and the item is proposed
partial because a measured clause was missed.

### Open questions (each with options and a recommendation)

- **Q1. The TR remainder.** (a) leave it with ADR 0359 question 2 (the kind changes next to the hole; its recommendation:
  accept until a case needs the BDF2 order on a moving overset mesh); (b) a research item on cells entering and leaving the
  calculated set within a BDF2 step, with TR as its case. Recommendation (a): the drag of TR changes by only `4e-6` between
  `dt = 0.01` and `0.005`, so the remainder does not limit what TR is for.
- **Q2. The case-tree wiring of BDF2 on a moving mesh.** It would make a refused input run and needs a schema entry; not
  taken. Open for the maintainer.
- **Q3. `set_rest_conditions`, `step_sliding` and `replace_mesh` with BDF2.** (a) a follow-up item; (b) not now.
  Recommendation (b) until a case asks for it.

### For the lead: roadmap entries

No roadmap file was edited. **F-345**, proposed status: `partial`. Proposed note:

> From ADR 0241 questions 4 and 5, recommendation taken on the maintainer's standing instruction of 2026-11-11. After the rework of the corrector flags in pimple.rs (F-322). ADR 0379, 2026-11-12: PARTIAL, every clause met but the measured TR ratio, whose cause is named with a number. Built: PimpleSolver::set_moving_mesh_time_scheme(Bdf2), an opt-in of the library's moving-mesh step (without it step_ale_with_walls refuses BDF2 as before): the mesh flux (c0 W - c2 W_old) / dt of the swept volumes of the last two steps (the discrete geometric conservation law of BDF2), the old and old-old volumes in the transient term, the three-level Rhie-Chow defect, an implicit Euler step when the step before was not a moving step with the opt-in; ale::set_follow_conditions_bdf2 with a FollowHistory: the wall velocity the mid-step velocities of the last two steps extrapolated to the end of the step with the normal part carrying the three-level mesh flux, dp/dn from the BDF2 derivative of the wall velocity; OversetSolver::prepare takes it when BDF2 is on. Defaults bit for bit (Q0 of ADR 0219 hash af2c9e6bd3e5596d, baseline_hashes, the existing moving-mesh and overset suites unedited). Met: a uniform flow stays uniform to 7e-15 on the four motions of ADR 0033 G2 with uniform and alternating steps; with every cell calculated OversetSolver with BDF2 equals the library's BDF2 moving step bit for bit on BF (one and three outer iterations, with and without rhie_chow_step_consistent); the wall velocity and acceleration are exact for a constant acceleration and of second order (ratio 4.000) for a cubic motion; the body-fitted cylinder BF to t = 1 falls by 5.04 when the step is halved (3.04 with BDF2 and the two-level walls, 2.97 with implicit Euler). Missed: the time error of TR falls by 1.09 (limit 4): it sits in the background cells one ring outside the hole, which change kind as the cylinder passes (ADR 0359 event 2); the component cells fall by 4.22. Nine mutations caught. Not delivered: the case-tree wiring, BDF2 for step_sliding, replace_mesh and set_rest_conditions.

**F-338**, status unchanged. In its note replace "Not delivered: BDF2 for the library's moving-mesh step and the follow conditions of a moving wall (they keep the two-level values), the case-tree wiring." by:

> Not delivered: the case-tree wiring. BDF2 for the library's moving-mesh step and the three-level follow conditions of a moving wall came with F-345 (ADR 0379; prepare takes them when BDF2 is on, and the BDF2 overset step with every cell calculated equals the library's bit for bit).

No follow-up item is proposed (question 1 belongs to ADR 0359 question 2).
