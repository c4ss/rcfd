# ADR 0241: F-338 registration: a second-order time scheme on the overset path (BDF2, opt-in)

Status: **REGISTERED 2026-11-12**, written before any code of F-338 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no check was run: the code and the documents listed under "What was read" were read, the
arithmetic under "Arithmetic before this registration" was done by hand, and the unchanged tree of main was compiled once
(`cargo test --release -p rcfd-fv --test overset_f335 --test overset_f335_records --test overset_f134 --no-run`, to fill the
target directory of the new worktree; nothing was executed). Results go in the addendum at the end; nothing above the
addendum is changed after a run.

Parents: [ADR 0219](0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md) (F-134: `OversetSolver`, the model
M1 to M11, the set-ups BF, U, TG_L, TGB_L, the checks Q0, T1, T1c, T2, E1),
[ADR 0232](0232-f314-registration-overset-observed-order-miss.md) (F-314: `d_200`, the single moving mesh SMc),
[ADR 0238](0238-f335-registration-overset-outer-iterations.md) (F-335: `set_outer_iterations`, the records R2, the miss of
D1 at `1.690e-4` and its cause, the implicit Euler truncation on the moving cells; questions Q3 and Q4, whose
recommendation is this item), [ADR 0033](0033-wave20b-dynamic-mesh.md) (the ALE step, the swept volumes and the geometric
conservation law), [ADR 0137](0137-linux-gate-platform-values.md) (platform values). Roadmap item F-338 (depends on F-335).

Number: the lead gave 0241 to F-338. Branch `f338-overset-bdf2` from main 249bec79, worktree `w49i-work/f338`.

House rules (ADR 0130, ADR 0213, ADR 0219, ADR 0232, ADR 0238):

- limits are fixed here, before the code and the runs, and are not changed after a run; the limits of F-134, F-314 and
  F-335 are not changed and their known-miss tests stay as recorded;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written and committed before the corrected run, and the old
  one stays as an ignored record;
- defaults stay bit-identical: a run that does not opt in runs as before, bit for bit;
- `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables and the goldens are not edited or re-recorded; no existing
  test is edited;
- each registered run is made once, and nothing is re-run to pick a result (a deterministic run that two tests share is the
  same run);
- no new third-party crate, no unsafe code, no commit trailers, no push.

Clean-room. Everything below is derived in this text from first principles. The names of the ideas, cited from general
knowledge (no paper, book, code or vendor document was opened for this work): the three-level backward differentiation
formula with a variable step (E. Hairer, S. P. Nørsett, G. Wanner, "Solving Ordinary Differential Equations I", Springer);
the geometric conservation law of a moving mesh (P. D. Thomas and C. K. Lombard, "Geometric conservation law and its
application to flow computations on moving grids", AIAA J. 17 (1979) 1030-1037; I. Demirdžić and M. Perić, "Space
conservation law in finite volume calculations of fluid flow", Int. J. Numer. Methods Fluids 8 (1988) 1037-1050); the
three-level scheme on a moving mesh, whose mesh flux must be the same combination of swept volumes as the time term is of
cell volumes (B. Koobus and C. Farhat, Comput. Methods Appl. Mech. Engrg. 170 (1999) 103-129; P. Geuzaine, C. Grandmont
and C. Farhat, J. Comput. Phys. 191 (2003) 206-227); PISO and outer iterations as in ADR 0238 (Issa 1986; Ferziger and
Perić). The formulas used are derived under "Design"; the citations only name where such schemes are discussed.

## What F-338 asks

The roadmap entry (acceptance, quoted, binding): "OversetSolver accepts BDF2 as an opt-in (defaults bit-identical, Q0 of
ADR 0219 and the records R2 of ADR 0238 still bit for bit); with it and three outer iterations max abs(d_200) of TG_2 at
dt 0.0025 (1.69e-4 in ADR 0238) falls below 1.2e-4 and its ratio to max abs(d_400) is at least 4 (second order gives 5),
and T1c of ADR 0219 is run with it as a new check against its limit 1.7, or the ADR names why not with a number." Note:
"From ADR 0238 questions 3 and 4, recommendation taken on the maintainer's standing instruction of 2026-11-11. T1c with
three outer iterations was observed at 1.80 (unregistered)."

**Reading (registered here).**

1. "Accepts BDF2 as an opt-in": `OversetSolver::new` keeps its refusal of every scheme but implicit Euler (check E1 of ADR
   0219 asserts that `new` refuses BDF2; its test stays unedited). A new method of the solver switches BDF2 on (O1).
2. `d_N` is the difference field of ADR 0232 and ADR 0238: over the non-hole cells at `t = 0.5`, over the decay `e^{-2 nu
   t}`, the difference of the velocities of TG_2 in `N` steps and in 800 steps. Both runs take BDF2 and three outer
   iterations (`set_outer_iterations(3, 1.0, 1.0)`). `dt = 0.0025` is 200 steps. A second-order error gives `d_200 / d_400 =
   (16 - 1) / (4 - 1) = 5`, a first-order one 3.
3. "T1c of ADR 0219 with it": `log2(e(TG_1) / e(TG_2))` with TG_1 in 200 steps and TG_2 in 800 steps, both with BDF2 and
   three outer iterations (the pair of the acceptance), limit `>= 1.7`. The known-miss test of ADR 0219 is not edited.
4. The three numbers (D1, D2, T1c-b below) are the acceptance. A miss of one of them is reported as a miss; see the
   decision rule.

## What was read (no run)

- ADR 0238, ADR 0232 and ADR 0219 (the parts on Q0, T1, T1c, T2 and the model); the roadmap entries F-134, F-314, F-335,
  F-338.
- `crates/rcfd-fv/src/overset.rs` in full; `crates/rcfd-fv/src/ddt.rs` (`History`, `add_ddt`);
  `crates/rcfd-fv/src/pimple.rs`, `step_with` (the weights `ddt_history_weights`, the old-level defect, the extrapolation on
  the first outer iteration, the pushes of the history); `crates/rcfd-fv/src/pimple/moving.rs` in full;
  `crates/rcfd-fv/src/ale.rs` (`move_mesh`); `crates/rcfd-fv/src/convection.rs` (`convection_impl`).
- The tests `overset_f134.rs` (Q0, E1, T1c), `overset_f314.rs` (`sm_run`, `sm_conditions`), `overset_f335.rs`,
  `overset_f335_records.rs`, `ale_gcl.rs` (G2).

What the code read shows:

1. **The library's BDF2 on a mesh at rest** (`PimpleSolver::step_with`). `History` keeps the old and the old-old level and
   the last step size. With `r = dt / dt_old` the weights are `c0 = (1 + 2 r) / (1 + r)`, `c1 = 1 + r`, `c2 = r^2 / (1 +
   r)` (`c0 = c1 - c2`); `add_ddt` adds `V c0 / dt` to the diagonal and `V / dt (c1 u^n - c2 u^{n-1})` to the right-hand
   side. The first step, which has no old-old level, is an implicit Euler step (`add_ddt` falls back). The old-level
   Rhie-Chow defect is `c1 (phi^n - (u^n)_f . S) - c2 (phi^{n-1} - (u^{n-1})_f . S)`. On the first outer iteration of a
   step with an old-old level the convecting flux and the cell values handed to the convection operator (central
   convection is a deferred correction on those values) are extrapolated in time, `(1 + r) q^n - r q^{n-1}`; the later
   outer iterations take the current ones. After the step the velocity components and the flux are pushed.
2. **The library's moving-mesh step has no BDF2.** `step_ale_with_walls`, `step_sliding` and `replace_mesh` refuse every
   scheme but implicit Euler ("a moving mesh supports the implicit Euler time scheme only", `pimple/moving.rs` lines 102,
   169, 261). `AleStep` carries one mesh flux, one old volume and one old-level defect; `fix_ddt` puts the old volume
   back for the old level only. No old-old volume or mesh flux of a moving mesh is kept anywhere. The lead's brief asks
   for a check "with every cell calculated, the BDF2 overset step against the library's BDF2 moving-mesh step": **that
   reference does not exist.** What is registered instead is under "Design" D6 and in the checks Q0b, M1 and G1; adding
   BDF2 to the library's ALE step is listed as an open question (it would touch `pimple.rs` and `pimple/moving.rs`, which
   this item leaves alone).
3. **The overset step** (`advance`, `piso`) takes its transient term from `add_ddt(ImplicitEuler)` and `AleStep::fix_ddt`,
   its convecting flux from `AleStep::relative_flux`, and its old-level defect from the flux of the old time on the faces
   that keep it (`kept`: a face with a calculated cell and no hole cell at the old kinds). `u_old` is taken before the
   refresh M9, so the old value of a cell that was a fringe cell is its interpolant at the old pose, and the old value of
   a cell that was a hole cell is the rigid velocity of the component. A cell that was a hole and is calculated one step
   later is refused ("uncovered"); with one fringe layer a cell can be a hole at `t^{n-1}`, a fringe cell at `t^n` and
   calculated at `t^{n+1}`.
4. `move_mesh` returns the mesh flux `W_f / dt` of the swept volumes, which satisfy `V^{n+1} - V^n = sum_f W_f` per cell to
   rounding (ADR 0033 G1). The component of an overset coupling moves rigidly and the background is at rest, so there the
   cell volumes are constant and `sum_f W_f = 0` to rounding.

## Arithmetic before this registration (disclosed, by hand)

- ADR 0238: the implicit Euler remainder with three outer iterations is `max abs(d_200) = 1.690e-4`, `d_400 = 5.63e-5`
  (ratio 3.00), a time error of `2.25e-4` at `dt = 0.0025`. On the single moving mesh of ADR 0232 (SMc) three outer
  iterations left `0.153 * 3.726e-4 = 5.7e-5`.
- The leading truncation of implicit Euler is `(dt / 2) u_tt`, of BDF2 `(dt^2 / 3) u_ttt` along the cell paths. With a
  rate `omega = u_ttt / u_tt` of order 1 to 5 per unit time on the paths of TG_2 (the component turns at 1 rad per unit
  time and moves at 0.58 through a wave of length `2 pi`), the ratio is `(2 / 3) omega dt`, between `2e-3` and `1e-2`: the
  BDF2 truncation at `dt = 0.0025` would be of the order `1e-6`. The start-up (one implicit Euler step) leaves the local
  error of one Euler step, about the global Euler error over the number of steps, `2.25e-4 / 200`, of the order `1e-6`,
  falling with `dt^2`. So if nothing else of first order is left, `max abs(d_200)` should be far below `1.2e-4`; whether
  the ratio reaches 4 depends on what the changes of kind leave (the cells that take one Euler step, D3, and the faces
  whose flux is refreshed), which was not estimated.
- Cost. ADR 0238: TG_2 with three outer iterations takes 46 s in 200 steps. The slow test below (TG_2 in 200, 400 and 800
  steps with BDF2 and with implicit Euler, both with three outer iterations; two more with BDF2 and one outer iteration;
  TG_1) about 14 minutes.

## Design

**D1. The switch.** `OversetSolver::new` and `refusal` are unchanged for a solver that does not opt in. New method

`OversetSolver::set_time_scheme(&mut self, scheme: TimeScheme) -> Result<(), String>`

accepts `TimeScheme::Bdf2` (sets `settings.time_scheme` and a private mark; from the next step on the solver keeps the
history below) and `TimeScheme::ImplicitEuler` (clears the mark and the history: the solver is again the solver of ADR
0219 and ADR 0238). It refuses the bounded BDF2 and Crank-Nicolson, leaving the solver unchanged. `advance` refuses
`settings.time_scheme = Bdf2` on a solver that did not opt in, as now. It combines with `set_outer_iterations`. New
read-only method `euler_fallback_cells(&self) -> usize`: the number of calculated cells that took the implicit Euler
weights in the last step because of their history (D3; zero without BDF2 or on the start-up step). No field of
`PimpleSettings`, no case-schema key, no change in `pimple.rs`, `pimple/moving.rs`, `ale.rs`, `ddt.rs` or `rcfd-api`;
`OversetInfo` and every existing signature are unchanged.

**D2. The scheme on a moving cell and the geometric conservation law.** The momentum balance of a cell that moves with
the mesh is `d/dt (V u) + sum_f (phi_f - m_f) u_f = (diffusion and pressure)`, with `phi_f` the flux through the moving
face and `m_f` the mesh flux. The three-level backward difference of `V u` at `t^{n+1}` with `r = dt / dt_old` is

`(c0 V^{n+1} u^{n+1} - c1 V^n u^n + c2 V^{n-1} u^{n-1}) / dt`, `c0 = (1 + 2 r) / (1 + r)`, `c1 = 1 + r`, `c2 = r^2 / (1 +
r)`

(for `r = 1`: `3/2, 2, 1/2`; it is exact for a quadratic in time, so its error is of second order). A uniform velocity
`u = U` with `sum_f phi_f = 0` is a solution if and only if `(c0 V^{n+1} - c1 V^n + c2 V^{n-1}) / dt = sum_f m_f`. With
`c1 = c0 + c2`, the left side is `[c0 (V^{n+1} - V^n) - c2 (V^n - V^{n-1})] / dt`, and with the swept volumes of the last
two steps, `V^{n+1} - V^n = sum_f W_f^{n+1}` and `V^n - V^{n-1} = sum_f W_f^n`, it is `sum_f (c0 W_f^{n+1} - c2 W_f^n) /
dt`. So the mesh flux of the three-level scheme is, face by face,

`m_f = (c0 W_f^{n+1} - c2 W_f^n) / dt = c0 m_f^{n+1} - (c2 / r) m_f^n`,

with `m^{n+1} = W^{n+1} / dt` and `m^n = W^n / dt_old` the mesh fluxes that `move_mesh` gave for this step and the one
before (`c2 / r = r / (1 + r)`; for `r = 1`: `(3/2) m^{n+1} - (1/2) m^n`). With it the geometric conservation law holds
per cell exactly as far as it holds for each single step (ADR 0033 G1, to rounding). It is also the second-order value
of the mesh flux at `t^{n+1}`: `m^{n+1}` and `m^n` are the means of the face's volume rate over the two steps, so they
are its values at the two mid-times to second order, and `c0 m^{n+1} - (c2 / r) m^n` extrapolates them linearly to
`t^{n+1}` (weights `1 + r / (1 + r)` and `-r / (1 + r)`, the linear extrapolation from `t^{n+1} - dt / 2` and `t^n -
dt_old / 2` to `t^{n+1}`: `(dt / 2) / ((dt + dt_old) / 2) = r / (1 + r)`). The implicit Euler step uses `m^{n+1}`, the
value at the middle of the step.

In the matrix: the library's `add_ddt(Bdf2)` expressions with the new volume (`V c0 / dt` on the diagonal, `V / dt (c1
u^n - c2 u^{n-1})` on the right), and the correction `[(V^n - V^{n+1}) c1 u^n - (V^{n-1} - V^{n+1}) c2 u^{n-1}] / dt` on
the right, which is exactly zero on a mesh at rest (so the step can be compared with the library there, D6).

**D3. What the old-old state of a cell means when its role changed.** The solver keeps per cell the velocity, the volume
and whether the cell was a hole, at the old-old time; per face the flux of the old-old time, whether that flux was a
kept one (the `kept` of that step: a calculated cell and no hole cell on the face), the old-old Rhie-Chow defect and the
mesh flux of the last step; and the last step size. The values follow the cell (the mesh moves, the index stays), which
is what the moving-cell balance of D2 needs. Only a cell that is calculated at `t^{n+1}` has a momentum row. Its history
can be:

| kind at `t^{n-1}` | kind at `t^n` | old-old value | old value | scheme of the row |
| --- | --- | --- | --- | --- |
| calculated or fringe | calculated or fringe | a solved value or the interpolant at the old-old pose | a solved value or the interpolant at the old pose | BDF2 |
| hole | fringe | the rigid velocity of the component, not a fluid value | the interpolant at the old pose | implicit Euler for this cell in this step |
| any | hole | (refused: uncovered, as now) | | |

A value interpolated at a fringe cell is the field at the cell's position to the accuracy of the donor interpolation
(second order, ADR 0232), so it is a valid level of the three-level formula in the same sense in which the implicit
Euler step of ADR 0219 already uses the old fringe value. A hole value is not: the cell takes the implicit Euler row
(`V^{n+1} / dt` and `V^n u^n / dt`) once, in the step in which it becomes calculated two steps after it was a hole. That
is a start-up of the cell: its local error is that of one Euler step, of order `dt^2`, and it happens once per cell, so
the scheme stays of second order (the same argument as for the start-up step of the run). No old-old value is rebuilt by
a donor search at the old-old pose (it would need the donors of two poses back; not built, O2).

The mesh flux is one number per face, so the cells that take the Euler row see the mesh flux of D2 too. Their uniform
flow needs `V^{n+1} - V^n = dt sum_f m_f = c0 sum_f W^{n+1} - c2 sum_f W^n`; hole cells exist only with an overset
coupling, whose component moves rigidly and whose background is at rest, so both sums are zero to rounding and the law
holds (item 4 of "What the code read shows"). This is checked by U1b on a turning component; it would not hold on a
deforming mesh with holes, which the solver cannot produce (limit L3).

The Rhie-Chow defect of a face is `c1 r^n D^n - c2 r^{n-1} D^{n-1}` with `D = phi - (u)_f . S` of its time on its
geometry, `r^n = interp(V^n / V^{n+1})` and `r^{n-1} = interp(V^{n-1} / V^{n+1})` (the volume scaling of ADR 0033), `D^n`
zero on a face that does not keep its flux (as now) and `D^{n-1}` zero on a face that did not keep it the step before. A
face with a cell that takes the Euler row takes the Euler weights `(1, 0)`.

**D4. The first outer iteration.** As the library: on the first outer iteration of a step with an old-old level the
convecting flux is `(1 + r) phi^n - r phi^{n-1}` (then less the mesh flux of D2) and the cell values handed to the
convection operator are `(1 + r) u^n - r u^{n-1}`; later outer iterations take the current flux and values. A flux
through a moving face is a smooth function of time along the face, so the extrapolation is of second order where both
fluxes are fluxes of the fluid: on a face that kept its flux at both old times (a boundary face: whose cell was
calculated at both). Elsewhere the convecting flux is the flux as refreshed by M9, as now. A cell value is extrapolated
where the cell was not a hole at either old time; elsewhere it is the current value (refreshed by M9). With one outer
iteration the faces and cells without extrapolation keep a first-order lag; outer iterations remove it (ADR 0238).

**D5. The start.** The first step after `set_time_scheme(Bdf2)` has no old-old level and is the implicit Euler step of ADR
0219 and ADR 0238, statement for statement; at its end the history is stored. Its error is the local error of one Euler
step, of order `dt^2`. Every later step is a BDF2 step with `r = dt / dt_old` (the step size may change).

**D6. Why a solver that does not opt in runs as before, and what the BDF2 step is compared with.** Without the mark no
history is kept and `piso` takes the present statements (the BDF2 parts sit in branches taken only with a history); the
mesh flux and the defect handed to `piso` are the present ones. Checked by Q0, R2 and the existing suites. With every
cell calculated on a mesh at rest (`advance_mesh` with the points unchanged) the BDF2 step is, by D2 to D5, the
library's `PimpleSolver::step` with BDF2, expression for expression (check Q0b). On a moving mesh the library has no
BDF2 (item 2 above), so the moving-mesh terms are checked for what they must do: a uniform flow on a deforming mesh
(G1: the conservation law with changing volumes), and the order in time on a single rigidly moving mesh with every cell
calculated (M1: SMc of ADR 0232 through `advance_mesh`).

## Order of the work

1. This registration, committed alone.
2. The library change (`crates/rcfd-fv/src/overset.rs` only) and the tests `crates/rcfd-fv/tests/overset_f338.rs`;
   compiled, formatted and linted without running; committed; then every registered check, once.
3. The mutations; the addendum.

The records R2 exist already (`overset_f335_records.rs`, hashes taken on main 65c36e60 before F-335); no new record is
taken.

## Registered checks

Release, Windows. PS, BF, U, TG_L, TGB_L and the error `e` are those of ADR 0219; SMc is that of ADR 0232 amendment 2.
"BDF2" is `set_time_scheme(TimeScheme::Bdf2)`; "three outer iterations" is `set_outer_iterations(3, 1.0, 1.0)`. Tests that
take more than about two minutes are `#[ignore]` with their run time as the reason and are run once.

### B. Defaults (preconditions of landing)

| Id | Check | Limit |
| --- | --- | --- |
| Q0 | the existing test of ADR 0219 Q0, unedited, after the code | `<= 1e-12` each; equal bit for bit; hash `af2c9e6bd3e5596d` |
| R2 | the existing tests of `overset_f335_records.rs`, unedited, after the code (R2a, R2b, R2c and R2') | pass (the hashes `7aa14d1e2e2da052`, `75ac2daf13879872`, `fddb420dc287502a`) |
| R2e | U for 80 steps and TG_1 for 200 steps after `set_time_scheme(ImplicitEuler)`, and after `set_time_scheme(Bdf2)` followed by `set_time_scheme(ImplicitEuler)`: the hash of ADR 0238 R2 | equal to R2a and R2b (Windows; printed elsewhere) |
| D-S | the existing tests, unedited: `overset_f134` (12 run), `overset_f134_records`, the non-ignored tests of `overset_f314` and of `overset_f335` | pass |

### Q. Equivalence with the library step (mesh at rest)

| Id | Check | Limit |
| --- | --- | --- |
| Q0b | TGB_1 (the background of level 1 alone, the Taylor-Green conditions) for 40 steps of `dt = 0.0025`: `advance_mesh` with the points unchanged, BDF2, against `PimpleSolver::step` with BDF2; with one and with three outer iterations: the largest difference of `u`, `p`, `phi` (over the largest flux); printed: whether equal bit for bit | `<= 1e-12` each |
| Q0bn (negative control) | Q0b's library run with three outer iterations against the overset run without BDF2 (implicit Euler, three outer iterations): the largest difference of `u` | `>= 1e-8` |

### G. The geometric conservation law with changing volumes (every cell calculated)

The set-up of G2 of ADR 0033 (`ale_gcl.rs`: the graded box `mesh_a`, the uniform velocity `(1, 0.5, -0.25)` fixed on
every patch, `nu = 0.01`, its tight solver controls), run through `OversetSolver::advance_mesh` with BDF2 and three outer
iterations for 10 steps, with the motion "expansion" of G2 (every point scaled about the centre by `1 + 0.2 sin(2 pi t /
0.5)`: the volumes change and the swept volumes of consecutive steps differ).

| Id | Check | Limit |
| --- | --- | --- |
| G1 | `dt = 0.05`: `max abs(u_c - U) / abs(U)` and `max abs(p_c) / abs(U)^2` after the 10 steps | `<= 1e-10` each |
| G1v | the same with the steps alternating `0.05` and `0.025` (`r` is 2 and 1/2) | `<= 1e-10` each |
| G1n (negative control) | G1 with the mesh flux of every step multiplied by 1.01 before `advance_mesh`: `max abs(u_c - U) / abs(U)` | `>= 1e-6` |

### U. Uniform flow through a moving, turning component

| Id | Check | Limit |
| --- | --- | --- |
| U1b | U with BDF2 and three outer iterations, every step, over the non-hole cells: `max abs(u_c - U) / abs(U)`, `max abs(p_c) / abs(U)^2` | `<= 1e-10` each |
| U3b | the same run: the continuity of the calculated cells; `abs(Q_xmax - U_x A_xmax) / (U_x A_xmax)` | `<= 1e-9`; `<= 1e-10` |
| U1b1, U3b1 | U with BDF2 and one outer iteration: the same four numbers | the same limits |
| U1v | U with BDF2 and three outer iterations, the steps alternating `0.01` and `0.005` (107 steps, see the note): the two numbers of U1b | `<= 1e-10` each |
| S-U (set-up) | the U1b run: the sum over the steps of `euler_fallback_cells()` | `>= 1` (the rule of D3 acts in the run) |
| N2b (negative control) | U with BDF2, three outer iterations and every stencil weight multiplied by 1.001 after `prepare`: `max abs(u_c - U) / abs(U)` | `>= 1e-6` |

Note on U1v: 107 steps, alternating `0.01` and `0.005` starting with `0.01` (54 steps of `0.01` and 53 of `0.005`, end time
`0.805`); the pose is `u_pose` at the time reached.

### M. The order in time on a single moving mesh (every cell calculated)

SMc of ADR 0232 (the level-2 component mesh alone, 40 by 40 cells, moved by `tg_pose(t)`, the exact velocity on its four
sides less its net flux) through `OversetSolver::advance_mesh`, three outer iterations, in 200, 400 and 800 steps.
`d_N` is the largest difference over the cells of the velocities of the run in `N` steps and the run in 800, over the
decay.

| Id | Check | Limit |
| --- | --- | --- |
| M0 (set-up, negative control) | implicit Euler: `max abs(d_200)`: the number of ADR 0232 (`0.153 * 3.726e-4`) | in `[5.5e-5, 5.9e-5]` |
| M1 | BDF2: `max abs(d_200)` | `<= 1.5e-5` |
| M2 | BDF2: `max abs(d_200) / max abs(d_400)` | `>= 4` |
| M3 | every step of the six runs: continuity | `<= 1e-9` |

### D. The acceptance: the time error of TG_2 and T1c

`d_N(scheme)`: TG_2 in `N` and in 800 steps, both with the scheme and three outer iterations, over the non-hole cells.

| Id | Check | Limit |
| --- | --- | --- |
| S2 (set-up, negative control) | implicit Euler, three outer iterations: `max abs(d_200)` and its ratio to `max abs(d_400)`: the numbers of ADR 0238 (`1.690e-4`, `3.00`) | in `[1.68e-4, 1.70e-4]`; in `[2.9, 3.1]` |
| D1 (acceptance) | BDF2, three outer iterations: `max abs(d_200)` | `< 1.2e-4` |
| D2 (acceptance) | BDF2, three outer iterations: `max abs(d_200) / max abs(d_400)` | `>= 4` |
| T1c-b (acceptance) | BDF2, three outer iterations: `log2(e(TG_1) / e(TG_2))`, TG_1 in 200 steps, TG_2 in 800 steps | `>= 1.7` |
| T2b | BDF2, three outer iterations: `e(TG_2)` in 200 steps over `e(TGB_2)` in 200 steps (the library, implicit Euler, one outer iteration, as recorded by ADR 0219) | `<= 2` |
| T3b | every step of the BDF2 runs of this table: the continuity of the calculated cells | `<= 1e-9` |
| S-D (set-up) | the BDF2 run of TG_2 in 200 steps: the sum over the steps of `euler_fallback_cells()` | `>= 1` |
| L1b | TG_1 in 200 steps with BDF2 and three outer iterations: `e(TG_1) / e(TGB_1)` (TGB_1 with the library, implicit Euler, one outer iteration) and the continuity of every step | `<= 2`; `<= 1e-9` |

Printed, no limit: `max abs(d_200)` and `max abs(d_400)` per class (CF, CC, BF, BC) and the cell of the maximum; `e(TG_2)` of
every run; T1 of ADR 0219 at `dt = 0.0025` with BDF2 and three outer iterations (limit of ADR 0219 `>= 1.7`; ADR 0232
showed that it needs `e(TG_2) <= 3.32e-4`, below the spatial error; not checked); BDF2 with **one** outer iteration:
`max abs(d_200)` (runs in 200 and 800 steps; no decision rests on it); the number of cells that took the Euler row; the
range of `delta`; the pressure iterations per step and the unconverged linear solves; the run times.

### E. Refusals

| Id | Check | Limit |
| --- | --- | --- |
| E1 | the existing test of ADR 0219 E1, unedited (`new` with BDF2 is `Err`) | pass |
| E8 | `set_time_scheme` with the bounded BDF2 and with Crank-Nicolson | `Err` each; `settings` unchanged |
| E9 | a solver that did not opt in, `settings.time_scheme = Bdf2` written directly, then `advance` | `Err`; `u`, `p`, `phi`, time unchanged bit for bit |
| E10 | `set_time_scheme(Bdf2)`, `set_outer_iterations(2, 1.0, 1.0)` and three steps of U | `Ok` |

### Mutations (scratch copy, not committed)

Each is applied alone to a `git archive` copy of the final tree by an exact replacement that must occur once, the named
test is run once and must fail; the file is restored and compared byte for byte. The numbers go in the addendum.
`scripts/limit_audit.py` is run on the new test file and every hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MB1 | the mesh flux of a BDF2 step is that of the step alone (`m^{n+1}`, no old part) | G1 |
| MB2 | the correction of the old-old volume is left out of the right-hand side | G1 |
| MB3 | the hole history is ignored: every calculated cell takes the BDF2 row | U1b |
| MB4 | the old-old velocity is stored one level too new (the velocity after the step instead of the one before it) | Q0b |
| MB5 | no extrapolation on the first outer iteration (flux and cell values) | Q0b |
| MB6 | the old-old part of the Rhie-Chow defect is left out | Q0b |

### Classes

Q0, R2, R2e, D-S and E1 are preconditions of landing: a miss means the change is not merged and the cause is recorded. A
miss of any other limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. A
set-up check or negative control (S2, S-U, S-D, M0, Q0bn, G1n, N2b) that does not hold, or a mutation that does not make
its check fail, is a defective set-up: corrected by an amendment before any re-run, never counted as a pass.

## Predictions (mine, before any run)

Q0, R2, R2e: equal. Q0b: equal bit for bit, or within `1e-13` (the background of TGB_1 is uniform, so the volume ratios
are exactly 1). G1, U1b: near `1e-13`. M0: `5.7e-5`. M1: below `2e-6`. M2: between 4 and 5.5; it may be disturbed if the
BDF2 error is so small that the remainder of three outer iterations shows. S2: `1.690e-4`, `3.00`. D1: between `2e-6` and
`3e-5`, met. D2: uncertain, between 3 and 5; the cells that change kind are the likely disturbance. T1c-b: about `1.8`,
met. T2b: about `1.6` to `1.9`. T1 at `dt = 0.0025`: about `1.7`, not checked.

## Decision rule

- **done**: every registered check is met (D1, D2 and T1c-b among them), the preconditions hold, the controls held and
  the mutations failed their checks.
- **partial**: the opt-in is built and the preconditions hold, but one of D1, D2, T1c-b is missed (with its cause named
  by a measurement and a deciding number registered in an amendment before that measurement, or left open: the note says
  which), or another registered check is missed, or a control or mutation did not hold and was not corrected.
- **todo**: the opt-in could not be built, or a precondition does not hold (then nothing is to be merged).

## Open choices taken

Each is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none is a decision the
maintainer made in person, and each is listed again in the addendum. None changes a default.

- **O1. Where the opt-in lives.** (a) `PimpleSettings::time_scheme = Bdf2` accepted by `OversetSolver::new`: one knob, but
  the registered check E1 of ADR 0219 asserts that `new` refuses BDF2, so an existing test would have to be edited; (b)
  `new` unchanged and a method of the solver (D1), as F-335 did for the outer iterations. Taken: (b).
- **O2. A cell whose old-old value is a hole value.** (a) one implicit Euler row for that cell in that step (D3); (b)
  rebuild the old-old value by interpolating the old-old field at the cell's old-old position (a donor search at a past
  pose, more state); (c) refuse the step. Taken: (a): it is of second order by the start-up argument and needs no new
  search; S-U and S-D show that it acts, MB3 that it matters.
- **O3. The reference for "every cell calculated" on a moving mesh.** The library has no BDF2 moving-mesh step. (a) check
  the rest case against the library (Q0b) and the moving terms by G1 and M1; (b) add BDF2 to
  `PimpleSolver::step_ale_with_walls` first and compare bit for bit. Taken: (a); (b) changes `pimple.rs` and
  `pimple/moving.rs`, which another job is about to rework, and is a library feature of its own (open question in the
  addendum).
- **O4. The bounded BDF2 and Crank-Nicolson** stay refused on the overset path.

## Limits of this registration

- L1. The acceptance numbers are measured on the Taylor-Green set-up of ADR 0219 (uniform Cartesian parts, one component,
  `nu = 0.1`, `t = 0.5`, PS: central convection, two correctors, no relaxation). Nothing is concluded for the cylinder
  case TR with BDF2: it is not run with it.
- L2. The follow conditions of a moving wall (`ale::set_follow_conditions`) give the wall the mean velocity of the step
  and the pressure gradient of a two-level acceleration; they are not changed, so a run with an accelerating wall and
  BDF2 keeps a first-order term at the wall. The Taylor-Green set-ups have no wall. The rigid velocity of the hole cells
  is the mean over the step too (no calculated row uses it).
- L3. The Euler row of a cell with a hole history (D3) keeps the conservation law only because the meshes of a coupling
  move rigidly.
- L4. With one outer iteration the faces and cells without a valid old-old value keep a first-order lag in the convecting
  flux (D4).
- L5. No BDF2 for the library's own moving-mesh step, the sliding step or `replace_mesh`; no case-tree, CLI or Studio
  wiring (ADR 0219 Q1); no restart file carries the overset history.
- L6. The first-order term of the calculated component cells (ADR 0232, F-334) and the fringe truncation are untouched.

## Open questions

Listed with options and a recommendation in the addendum, after the results.

## Amendment 1 (2026-11-12, written after the registered runs of Q, G, U, M, R2e, L1b, E and D and before any further run)

The library change and the tests were committed compiled and not run (`2d2c3709`), then every registered check of the new
file ran once, release. All numbers are in addendum 1. Met: Q0b (bit for bit, one and three outer iterations), Q0bn, G1,
G1v, G1n, U1b, U3b, U1b1, U3b1, U1v, N2b, R2e, E8 to E10, L1b, M0 (`5.697e-5`), M1 (`3.96e-6`), M3, S2 (`1.6897e-4`,
`3.001`), **D1** (`2.125e-5` against `1.2e-4`), T2b (`1.634`), T3b. Not met:

| Id | Limit | Measured |
| --- | --- | --- |
| D2 (acceptance) | `>= 4` | `2.456` (`max abs(d_200) = 2.125e-5`, `max abs(d_400) = 8.65e-6`, both in a calculated component cell at `(1.67, 0.97)`) |
| T1c-b (acceptance) | `>= 1.7` | `1.664` (`e(TG_1) = 1.0076e-3` in 200 steps, `e(TG_2) = 3.179e-4` in 800 steps) |
| M2 | `>= 4` | `2.282` (SMc, every cell calculated: `max abs(d_200) = 3.96e-6`, `max abs(d_400) = 1.73e-6`) |
| S-U (set-up) | `>= 1` | `0` |
| S-D (set-up) | `>= 1` | `0` |

The limits stay. D2, T1c-b and M2 become ignored `*_known_miss` tests with the limits intact; the tests of U, M and D keep
the checks that held.

**S-U and S-D: a defective set-up, corrected here.** No cell took the implicit Euler row in any run. I had not worked out
how long a cell stays a fringe cell: a background cell of size `h` leaves the fringe layer about `h / v` after it left the
hole, `0.09` time units in U (`h = 0.1`, `v` about 1.1) against a step of `0.01`, and `0.07` in TG_2 against `0.0025`. The
history "hole, fringe, calculated" in three consecutive times needs a step of about half that time or more. So the rule
of D3 does not act in the acceptance runs at all (S-D as a requirement was wrong; its number, 0, is reported and nothing
of D depends on the rule), and the set-up U as registered does not exercise it. Corrected set-up **Uc**: U with `dt =
0.05` for 16 steps (the component moves about half a background cell per step), BDF2 and three outer iterations.

| Id | Check | Limit |
| --- | --- | --- |
| U1c | Uc, every step, over the non-hole cells: `max abs(u_c - U) / abs(U)`, `max abs(p_c) / abs(U)^2` | `<= 1e-10` each |
| U3c | Uc: the continuity of the calculated cells; the outflow error | `<= 1e-9`; `<= 1e-10` |
| S-Uc (set-up) | Uc: the sum over the steps of `euler_fallback_cells()` | `>= 1` |

If a step of Uc is refused as uncovered, the set-up is defective again and is corrected by a further amendment. **MB3 must
fail U1c** (not U1b, where the rule does not act). The registered S-U and S-D stay as ignored record tests.

**The misses of D2 and M2: hypotheses, fixed before their runs.** The remainder is small (D: `0.126` of the implicit Euler
remainder; SMc: `0.069`) and falls more slowly than first order between the two pairs of runs; a ratio below 3 of two
maxima is what a first-order term and a second-order term of opposite sign give, or a term that the step does not
control. It appears on the single moving mesh with every cell calculated (M2), so it is not made by the overset coupling.

| Id | Hypothesis | Measurement | Deciding number |
| --- | --- | --- | --- |
| H16 | the remainder belongs to the library's BDF2 step on a mesh at rest too (it is not made by the moving-mesh terms of D2) | the mesh of SMc at rest at `tg_pose(0)`, the conditions of SMc, `PimpleSolver::step` with BDF2 and three outer iterations in 200, 400 and 800 steps: `R_rest = max abs(d_200) / max abs(d_400)`, and `max abs(d_200)` | confirmed: `R_rest < 3.5`; ruled out: `R_rest >= 4` |
| H17 | the remainder is left by the start (the implicit Euler first step and the first projection of the initial field), not by the steps after it | SMc, BDF2, three outer iterations: 160 steps of `0.000625` to `t = 0.1` in every run, then to `t = 0.5` in 160, 320 and 640 steps: `R_start = max abs(d'_160) / max abs(d'_320)` (differences to the run with 640) | confirmed: `R_start >= 4`; ruled out: `R_start < 3.5` |
| H18 | the remainder is what two correctors and three outer iterations leave of the pressure-velocity coupling | SMc, BDF2, (a) four correctors and three outer iterations, (b) two correctors and six outer iterations: the ratio `max abs(d_200) / max abs(d_400)` of each | confirmed: either ratio `>= 4`; ruled out: both `< 3.5` |

Reading fixed here. If H16 is confirmed and H17 and H18 are ruled out, the cause is located: a term of the library's BDF2
PISO step itself, present at rest, smaller than `max abs(d_200)` of SMc; what it is stays open and is a follow-up. If H16
is ruled out, the moving-mesh terms of this item carry a term of lower order and the item has a defect to find. If H17 is
confirmed, the cause is the start and the number is `R_start`. Between the deciding numbers a hypothesis is undecided.
None of this moves the limit of D2.

**The miss of T1c-b: hypothesis.**

| Id | Hypothesis | Measurement | Deciding number |
| --- | --- | --- | --- |
| H19 | with BDF2 the errors of both levels are free of time error, so `1.664` is the spatial order of the pointwise maximum between levels 1 and 2 (ADR 0232: the donor interpolation sampled at one pose); the `1.80` observed in ADR 0238 was the time error of TG_1 in 200 steps (`e = 1.1056e-3` there) | TG_1 with BDF2 and three outer iterations in 800 steps: `s_1 = abs(e_800 / e_200 - 1)`; from the registered runs `s_2 = abs(3.1698e-4 / 3.1791e-4 - 1) = 0.003` | confirmed: `s_1 <= 0.01`; ruled out: `s_1 > 0.03` |

Tests of this amendment (in `overset_f338.rs`): `uc_uniform_flow_with_a_large_step_and_bdf2`,
`a1_remainder_of_bdf2_on_the_single_mesh` (prints H16 to H18), `a1_level_1_error_is_free_of_time_error` (prints H19);
the last two assert nothing about their numbers (decided here). Predictions (mine): Uc holds with a few tens of cells;
H16 confirmed; H17 ruled out or undecided; H18 ruled out; H19 confirmed. The decision rule is unchanged: with D2 and
T1c-b missed it gives **partial**.
