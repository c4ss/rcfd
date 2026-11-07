# ADR 0219: F-134 registration: overset (chimera) coupling of a moving component mesh with a background mesh, in the library

Status: **REGISTERED 2026-11-07**, written before any code of F-134 and before any run of its checks. Not yet accepted by
the maintainer. Before this text one thing was run on main bfa9a3ee, with the code of main as it is: the record test R1
(the new file `crates/rcfd-fv/tests/overset_f134_records.rs`, no source change), twice, which made the record below.
Results go in the addendum at the end.

Parents: [ADR 0033](0033-wave20b-dynamic-mesh.md) (F-063: the incompressible ALE step, the swept volumes and the GCL,
the moving-wall pressure condition D3, the force on a patch D6), [ADR 0101](0101-wave41a-oscillating-cylinder-two-timescale-decay-vortex-bracket-milne.md)
(the continuity `1e-9` and GCL `1e-12 mean(V)` limits of the moving-mesh checks), [ADR 0216](0216-f250-registration-sliding-interface-on-a-cylinder.md)
(F-250: a step onto a mesh whose connectivity changes every step, the uniform-flow and negative-control pattern),
[ADR 0213](0213-f257-registration-prism-layering-and-incompressible-layering.md) (F-257: the house rules of these
registrations), [ADR 0129](0129-test-time-limit-jobs5-measurement-limit-audit.md) (test time limits, `limit_audit.py`),
[ADR 0137](0137-linux-gate-platform-values.md) (platform values). Roadmap item F-134 (depends on F-063).

Number: the lead gave 0219 to F-134. Main bfa9a3ee holds 0216 as its highest; 0215 is on the branch
`f247-local-remeshing`, 0217 on `f221-wet-steam` and 0218 on `f219-immersed-solid`. No branch holds a 0219.

House rules (ADR 0130, ADR 0191, ADR 0213, ADR 0216):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded, and the
  cause is diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as
  an ignored record;
- every new code path is opt-in: a run that does not call the new types and functions runs as before, bit for bit (R1 and
  the existing suites);
- no case-schema key is added (S5), so neither `crates/rcfd-api/tests/baseline_hashes.rs` nor the record R0
  (`case_hashes_w48u.rs`) is touched, no golden is re-recorded, and no existing test is edited;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no commit trailers.

Clean-room: the overlapping-grid (chimera) idea, a body-fitted grid embedded in a background grid with a hole cut where
the body is and interpolated cells on the edges of the overlap, is old public knowledge. No paper, book, code or vendor
document was read for this work. Every formula below is derived here from the finite-volume definitions of the solver
(ADR 0033) and from linear algebra. The reference values are a closed form derived here (the Taylor-Green vortex, an exact
solution of the Navier-Stokes equations) and runs of the solver itself (the body-fitted run). No published data are used.

## What F-134 asks

The roadmap entry: overset (chimera) mesh coupling; "Gap: overset boundary type and methods; extends F-063".
Acceptance: "A cylinder moving through a background mesh conserves mass and gives drag within 5 percent of a body-fitted
run."

**Reading of the acceptance (registered here).**

1. "A cylinder moving through a background mesh": a 2-D circular cylinder of diameter `D = 1` on a body-fitted component
   O-grid that translates rigidly through a static Cartesian background mesh, from rest to the speed `U = 1` over one time
   unit and then at constant speed, for 7.5 diameters, at `Re = U D / nu = 40` (laminar, steady wake in the body frame, no
   shedding). The hole cut into the background and the interpolation partners are rebuilt every step (set-up TR).
2. "Conserves mass" is read in two parts, both every step: (a) every calculated cell of both meshes satisfies the discrete
   continuity (A1a); (b) the domain as a whole: the cylinder has a fixed volume and the background is closed except one
   fixed-pressure side, so the net outflow through that side must be zero; a leak of the overlap would leave there (A1b).
3. "Drag within 5 percent of a body-fitted run": the in-line force on the cylinder of TR at every step from `t = 2` to
   `t = 8` is within 5 percent of the force at the same step of the same motion computed on one body-fitted O-grid that
   translates rigidly with the cylinder (ADR 0033's ALE step, set-up BF). The inner 30 radial layers of BF are the
   component mesh of TR, so the two runs differ in the overset coupling and in the outer mesh (A2). The window starts one
   time unit after the start-up, so the comparison is made in steady motion.
4. The acceptance is library work. The case tree is not in it (S5, Q1). The decision rule (below) sets F-134 done when every
   registered check is met, at the library scope registered here.

## What the code read shows (main bfa9a3ee, read only)

1. **`PimpleSolver::step`** (`rcfd-fv/src/pimple.rs`) is one function for every option. For the subset used here
   (implicit Euler, one outer iteration, no non-orthogonal correctors, central or upwind convection, no sources) one step
   is: the pressure conditions `pressure_bc` (with `AleStep::apply_wall_gradients` on a moving mesh), the boundary velocities
   `ub`, the Green-Gauss pressure gradient; per velocity component the matrix `convection(phi - m) + laplacian(nu)`,
   `add_ddt` and `AleStep::fix_ddt`, the right-hand side `- V grad p`, solved by `FvMatrix::solve` (BiCGStab with ILU(0)
   for a non-symmetric matrix); `a_P` the mean of the component diagonals, `rAU = V / a_P`, `gamma_p = interp(rAU)`; per
   corrector `HbyA = H / a_P`, `phiHbyA = HbyA_f . S + gamma_p / dt * defect` plus `AleStep::add_wall_flux`,
   `solve_pressure` (CG with IC(0) for the symmetric matrix, the soft pin `diag += d` at `p_ref` when no boundary fixes the
   level), `corrected_flux`, `u = HbyA - rAU grad p`. Every matrix is face-addressed (LDU): only face neighbours couple.
2. **`step_ale_with_walls`** (`pimple/moving.rs`) zeroes the mesh flux on empty faces, keeps the old volumes and computes
   the old-level Rhie-Chow defect `r_f (phi_f - (u_old)_f . S_f)` with the old geometry and `r_f = w V0_o/V1_o + (1 - w)
   V0_n/V1_n`.
3. **`rcfd_fv::ale`**: `move_mesh` (new geometry and the mesh flux `W_f/dt` of ADR 0033's swept volumes; a face whose points
   do not move sweeps exactly zero), `set_follow_conditions` (the velocity of a wall that follows the mesh and its pressure
   gradient `dp/dn = -n . a`).
4. **`rcfd_fv::sixdof::patch_force_moment`**: `F = rho sum [p S - nu (grad u + grad u^T) . S]` over named wall patches.
5. **`rcfd_mesh::gen`**: `cylinder_ogrid(n_theta, n_r, r_in, r_out, grading, depth)` (patches `cylinder`, `farfield`,
   `frontAndBack`); the 2-D boxes take their depth from the cell size, so no generator makes a rectilinear 2-D grid of given
   node coordinates and given depth. `Mesh::from_face_lists` validates a mesh, which need not be connected.
6. Nothing in the library or the case tree cuts holes, finds donor cells or couples two overlapping meshes.

## Run before this registration (disclosed)

The test file `crates/rcfd-fv/tests/overset_f134_records.rs` was written with no source change and run twice on main
bfa9a3ee (Windows, release). Both runs printed the same value:

| Record | Run | Hash |
| --- | --- | --- |
| R1 | the first 20 steps of BF below with `PimpleSolver::step_ale_with_walls`; the bits of `u`, `p`, `phi`, the time and the step count | `af2c9e6bd3e5596d` |

R1 printed 6336 cells and a largest continuity of `7.6e-17`. No check below was run.

## The model (derived)

Notation: the composite mesh has the background cells `0..n_B` and the component cells after them. "Part" is background
or component. `P` is the rigid pose of the component (a rotation about an axis through a point, then a translation,
`rcfd_mesh::motion::Pose`); `x_ref = P^-1(x)` maps a point into the component's reference frame.

**M1. The composite mesh.** The disjoint union of the background mesh and the component mesh: points, cells and internal
faces of the background, then those of the component (indices shifted), then the background patches, then the component
patches with a prefix on their names (names must be unique). No face joins the two parts. The component points are moved
to `P(x_ref)` from their reference positions every step, so no round-off accumulates.

**M2. Cell kinds.** At a pose every cell is *calculated* (its finite-volume equations are solved), *fringe* (its values are
interpolated from the other part) or *hole* (excluded; inside the body or in the cut-out part of the overlap).

- Component: a cell with a face on one of the named outer patches of the component is fringe; every other component cell
  is calculated. One fringe layer is enough: every operator of the step couples face neighbours only (M5, M7), and the
  calculated cells next to a fringe cell see its interpolated value.
- Background: a cell is a hole when the user's hole function, evaluated at `P^-1` of its centre, is true (for the cylinder:
  `|x_ref| < 1`, twice the radius); a non-hole cell with a hole face neighbour is fringe; every other background cell is
  calculated. So no calculated cell has a hole neighbour.

**M3. Donors.** For a fringe cell with centre `x`, the donor cell is the cell of the other part that contains `x`
(component donors: `x_ref` in the reference component mesh; background donors: `x` in the background). Containment of a
convex cell: `sigma_f S_f . (x - fc_f) <= 0` for all its faces (outward area vectors), within `1e-10` of the cell size;
among several cells the one with the largest smallest distance to its faces, and of those the lowest index. A uniform
grid of bins over each part speeds up the search. The donor and the stencil (below) must consist of calculated cells;
otherwise the pose is refused with the fringe cell, its part and the point named ("no donor").

**M4. Interpolation weights.** The stencil of a fringe cell is its donor cell (first) and those face neighbours of the donor
that are calculated cells of the same part. With `r_k = x_k - x` in the solved directions (those without empty faces) and
`a_k = (1, r_k)`, the weights are the minimum of `sum_k w_k^2 / omega_k` under `sum_k w_k a_k = (1, 0)`, `omega_k = 1 /
(|r_k|^2 + 1e-6 h^2)`, `h^2` the mean of `|r_k|^2`: `w_k = omega_k a_k^T lambda` with `(sum_k omega_k a_k a_k^T) lambda =
(1, 0)`. They reproduce every linear field exactly (`sum w_k = 1`, `sum w_k r_k = 0`), so the interpolation is second
order. A stencil whose small system has a pivot below `1e-12` of its largest entry is refused. Component weights are
computed in the reference frame (a rigid motion keeps them).

**M5. Momentum.** The finite-volume rows of the calculated cells are those of item 1 of the code read, assembled on the
whole composite mesh (the rows of the fringe and hole cells are assembled too and then replaced). The system is converted
to compressed rows and the rows replaced: a fringe row `s_i (u_i - sum_k w_ik u_k) = 0`, a hole row `s_i u_i = s_i
V_hole,i`, with `s_i = V_i / dt` (of the size of the transient diagonal). Solved by BiCGStab with ILU(0) (the momentum
controls). `V_hole` is the rigid velocity of the component at the cell centre over the step,
`(x - P_old(P_new^-1(x))) / dt`. With no fringe and no hole cell the LDU system is solved by `FvMatrix::solve`, as the
library does.

**M6. HbyA and rAU of fringe and hole cells.** Calculated cells: `HbyA = H / a_P`, `rAU = V / a_P` as the library.
Fringe cells: `HbyA_i = sum_k w_ik HbyA_k`, `rAU_i = sum_k w_ik rAU_k` (the donors are calculated). Hole cells: `HbyA =
V_hole`, `rAU = dt`; no calculated row uses them. So `phiHbyA` and `gamma_p = interp(rAU)` on a face between a calculated
and a fringe cell are those of a face between two calculated cells with the fringe cell's interpolated state.

**M7. Pressure, and the conservation of mass across the overlap.** The pressure matrix is the library's (`laplacian` of
`gamma_p`, right-hand side `-div(phiHbyA)`, the soft pin at `p_ref` when no fixed pressure fixes the level), and its fringe
and hole rows are replaced as in M5 with `s_i` the Laplacian diagonal of the cell. Every calculated cell then keeps its
continuity equation. That is not enough for the mass of the domain. Summed over the calculated background cells, the
continuity equations give `Q_D + Q_G = 0`, `Q_D` the net outflow through the domain boundary and `Q_G` that through
`Gamma_B`, the faces between a calculated and a fringe background cell. `Gamma_B` encloses the body, so the fluid inside
it has a constant volume and `Q_G` must be zero; with an interpolated fringe it is not, and the difference leaves through
the domain boundary (an open domain) or makes the coupled system singular and inconsistent (a closed domain: the composite
matrix keeps the constant null vector of a Neumann problem, and its left null vector need not be orthogonal to the
right-hand side).

The conservative coupling adds one unknown `delta`, a uniform offset of the background fringe pressures from their
interpolants: background fringe rows `s_i (p_i - sum_k w_ik p_k - delta) = 0`, and one more equation:

- open domain (some boundary fixes the pressure): `Q_G(p) = sum_{f in Gamma_B} sigma_f (phiHbyA_f - gamma_f L_f
  (p_N - p_O)) = 0` (`sigma_f = +1` when the calculated cell owns `f`, `L_f` the face coefficient of the Laplacian), so the
  net outflow of the domain is zero;
- closed domain: `p_r = p_ref,value` exactly, so that the soft pin, `d (p_r - value)`, adds no mass at the pin cell `r`
  (which must be a calculated cell).

Both are affine in `p`, so with `A` the pressure matrix (with the pin), `A p0 = b` and `A p1 = e` (`e_i = s_i` on the
background fringe rows), `p = p0 + delta p1` and `delta` follows from the one equation: `delta = -Q_G(p0) / (Q_G(p1) -
Q_G(0))` in the open case, `delta = (value - p0_r) / p1_r` in the closed case. `A` does not change between the correctors
of a step (the same `gamma_p`, conditions and rows), so `p1` is solved once per step. Without background fringe cells
`delta` is not used. The interpolated coupling (`MassMode::Interpolated`, the negative control NM) leaves `delta = 0`.

**M8. After each pressure solve.** The flux is the library's `corrected_flux` on every face; it is the flux of the pressure
equation on every face of a calculated cell. The calculated cells get `u = HbyA - rAU grad p` (the Green-Gauss gradient),
then the fringe cells `u_i = sum_k w_ik u_k` and the hole cells `V_hole`, and the flux of every face without a calculated
cell is replaced by the face interpolation of that velocity (it enters no equation; this keeps it finite and its Courant
number meaningful). The pressure update is the library's `p_old + 1 (p_new - p_old)`.

**M9. A change of kinds between two steps.** The kinds of the background cells change as the body moves. Before the
momentum predictor of a step on the new kinds:

- a cell that was a hole and is now calculated is refused ("uncovered"): its old value is not defined. The fringe layer
  makes this impossible while the body moves less than one background cell per step;
- every fringe cell gets `u_i = sum_k w_ik u_k` and `p_i = sum_k w_ik p_k` (plus the last `delta` on the background) from
  the old values of its new donors (calculated now, so calculated or fringe before), and every hole cell `V_hole`;
- an internal face keeps its old flux and its old-level Rhie-Chow defect (item 2) when one of its cells was calculated and
  neither was a hole; any other internal face gets the face interpolation of the refreshed velocity on the new geometry and
  a zero defect (as F-250's new interface faces, ADR 0216 M4);
- the old velocity of the transient term is the old value of each cell (calculated or fringe before, so defined).

With every cell calculated and no change of kinds the step is exactly the library's ALE step of items 1 and 2 (check Q0).

**M10. Motion.** The background does not move; its faces sweep no volume. The component moves rigidly from `P_old` to
`P_new`; `ale::move_mesh` gives the mesh flux and the new geometry; its moving walls get `ale::set_follow_conditions` (the
same calls as BF). The TR motion: the displacement along x `s(t) = -(t/2 - sin(pi t)/(2 pi))` for `t <= 1` (speed
`-(1 - cos(pi t))/2`) and `s(t) = -(1/2 + (t - 1))` after.

**M11. Force.** `patch_force_moment` on the cylinder wall patch, `rho = 1`, for both runs. The in-line force `F_x` is the
drag (the cylinder moves in `-x`), `C_D = F_x / (U^2 D L / 2)`.

## Settings and code

**S1. Library (`rcfd-mesh`, new module `overset`, additive).** `compose(background, component, prefix) -> Result<Composite>`
(M1), `Composite::points_at(&Pose)`, `CellLocator` (M3), `linear_weights` (M4), `OversetSpec { outer_patches, hole }`,
`Overset::new(composite, spec)`, `Overset::connectivity(&moved_mesh, &pose) -> Result<Connectivity>` (M2, M3, M4),
`Connectivity { kind, stencils, .. }` and `CellKind { Calculated, Fringe, Hole }` (all fields public, so a test can alter
them for the negative controls). `gen.rs` gets one new function, `rectilinear_2d(xs, ys, depth)` (a one-cell-deep grid of
the given node coordinates, patches `xmin`, `xmax`, `ymin`, `ymax`, `frontAndBack`); no existing function is edited.

**S2. Library (`rcfd-fv`, new module `overset`, additive).** `OversetSolver` (geometry, `u`, `p`, `phi`, time, the
settings, the mass mode, the coupling and the current connectivity): `new(geom, u, p, settings, mass, coupling)`,
`prepare(dt, pose, moving_walls) -> Result<OversetStep>` (M10, M2 to M4; it sets the follow conditions on `u` as BF's loop
does), `advance(dt, step) -> Result<OversetInfo>` (M5 to M9), `step(dt, pose, walls)` (both), `advance_mesh(dt,
mesh_step, wall_dpdn)` (a plain moving mesh, every cell calculated: the equivalence path of Q0). `MassMode { Conservative,
Interpolated }`. `OversetInfo` per step: the continuity of the calculated cells (the library's measure), the Courant
number, the counts of hole and fringe cells, the number of cells whose kind changed, `delta`, `Q_G`, the pressure
iterations and the number of linear solves that did not converge. `new` refuses, with `Err`: a time scheme other than
implicit Euler, `n_outer != 1`, non-orthogonal correctors, a convection scheme other than upwind, central or blended,
`amg_pressure`, a linear device other than the CPU, the fractional step, the full viscous stress, the lag check, the
adaptive correctors, the limiter, the gradient options, the consistent correctors and the component diagonal.
`pimple.rs`, `moving.rs`, `ale.rs` and every existing function are not edited.

**S3. Linear solvers.** PS below for momentum and pressure. A system with fringe or hole rows is solved by BiCGStab with
ILU(0) on its compressed rows; one without by the library's call (item 1).

**S4.** No change to `rcfd-api`, `rcfd-cli`, the Studio, the case schema, `baseline_hashes.rs`, `case_hashes_w48u.rs` or a
golden.

**S5. Case tree.** Not changed; no key, generator or schema entry. The wiring is Q1. `docs/dynamic-mesh.md` and the README
get prose only.

## Set-ups

PS (solver settings of every run): `PimpleSettings::piso(nu)` with central convection, two correctors, no non-orthogonal
correctors, implicit Euler, `ddt_correction` on, and the momentum and pressure controls `tol_abs = 1e-15`, `tol_rel =
1e-12`, at most 3000 iterations. "Every step" means the largest value over the steps of the run. `mean(V)` is the mean
cell volume of the component.

- **BF** (body-fitted): `cylinder_ogrid(96, 66, 0.5, 20.46, Expansion(1.08^65), 0.1)` (radial cells `0.01 * 1.08^i`; 6336
  cells), translating rigidly by `s(t)` (M10; the whole mesh, from its reference points every step); `cylinder` follows the
  mesh (`set_follow_conditions`, its pressure gradient to the step), `farfield` `u = 0`; `p` zero gradient, the soft pin at
  cell 0; `nu = 0.025`; start at rest; `dt = 0.02`, 400 steps (`t = 8`), `PimpleSolver::step_ale_with_walls`.
- **TR** (overset): background `rectilinear_2d` with x nodes: 30 cells on `[-25, -8]` growing away from `-8` (the cell at
  `-8` of size 0.1, `expansion_for_end_cell`), 125 uniform cells of 0.1 on `[-8, 4.5]`, 30 cells on `[4.5, 22]` (the cell at
  4.5 of size 0.1); y nodes: 30 on `[-20, -2.5]`, 50 uniform on `[-2.5, 2.5]`, 30 on `[2.5, 20]`; depth 0.1 (20,350 cells).
  Component `cylinder_ogrid(96, 30, 0.5, 1.6328, Expansion(1.08^29), 0.1)` (the inner 30 layers of BF, 2880 cells), prefix
  `c_`, outer patch `c_farfield`, hole `|x_ref| < 1`. Pose: translation `(2 + s(t), 0, 0)` (from `x = 2` to `x = -5.5`).
  Background `xmin`, `ymin`, `ymax`: `u = 0`; `xmax`: `p = 0`, velocity zero gradient; `c_cylinder` follows the mesh. Mass
  mode conservative (open: `delta` from `Q_G = 0`). `nu = 0.025`, start at rest, `dt = 0.02`, 400 steps.
- **U** (uniform flow): background `rectilinear_2d` 50 x 50 uniform on `[-2.5, 2.5]^2`, depth 0.1; component
  `rectilinear_2d` 16 x 16 on `[-0.6, 0.6]^2`, depth 0.1, prefix `c_`, outer patches the four sides, hole
  `max(|x_ref|, |y_ref|) < 0.25`. Pose: rotation by `t` about the z axis through the component centre, then translation `(t,
  0.5 t, 0)`. `U = (1, 0.5, 0)`; `xmin`, `ymin`, `ymax`: `u = U`; `xmax`: `p = 0`; start `u = U`, `p = 0`; `nu = 0.01`;
  conservative; `dt = 0.01`, 80 steps (the component turns by 0.8 rad and its centre moves by 0.89).
- **TG_L**, `L = 1, 2` (Taylor-Green vortex): background `rectilinear_2d` `40L x 40L` uniform on `[0, pi]^2`, depth 0.1;
  component `rectilinear_2d` `20L x 20L` on `[-0.6, 0.6]^2`, prefix `c_`, outer patches the four sides, hole
  `max(|x_ref|, |y_ref|) < 0.25`; pose: rotation by `t`, translation `(pi/2 - 0.3 + 0.5 t, pi/2 - 0.2 + 0.3 t, 0)`. Exact
  solution `u = (-cos x sin y, sin x cos y) e^{-2 nu t}`, `p = -(cos 2x + cos 2y) e^{-4 nu t} / 4`, `nu = 0.1`; the four
  background sides `u` = the exact value at the face centres at the new time, set before every step; `p` zero gradient
  (closed; `dp/dn = 0` is exact on these sides), the pin at cell 0 (a background corner cell); start: the exact `u` and
  `p` at the cell centres; conservative (closed: `p_r` exact); `dt = 0.0025`, 200 steps (`t = 0.5`). Reference **TGB_L**:
  the background alone with `PimpleSolver::step`, the same settings and conditions. Error `e = max_c |u_c - u_ex(x_c,
  0.5)| / e^{-2 nu 0.5}` over the calculated and fringe cells.

## Registered checks

All tests run in release. The mesh checks are unit tests in `crates/rcfd-mesh/src/overset.rs`, the flow checks are in
`crates/rcfd-fv/tests/overset_f134.rs`, the record in `overset_f134_records.rs`.

### K. The composite mesh, the donors and the weights (unit, `rcfd-mesh`)

TR's meshes; "the poses" are the translations `(2 + s, 0, 0)` with `s` in `{0, -0.37, -1.234, -3.9, -7.5}`.

| Id | Check | Limit |
| --- | --- | --- |
| K1 | `compose`: cell, internal-face and boundary-face counts the sums of the parts; every cell centre and volume equal to that of its part; the patch names (background, then the component's with the prefix) and sizes | exact |
| K2 | (i) every cell centre of each part located by that part's locator is its own cell; (ii) 2000 points of a fixed linear congruential sequence in the bounding box of each part: the locator's answer equals a search over all cells of the part with the same containment rule (`None` for both outside every cell, for example inside the cylinder) | exact |
| K3 | at the poses, every stencil: `abs(sum w - 1) <= 1e-13` and `abs(sum_k w_k f(x_k) - f(x)) <= 1e-12 max abs(f)` for `f = 3 + 2x - 5y`; at least three cells | as stated |
| K4 | at the poses: (i) no calculated cell has a hole face neighbour; (ii) every stencil cell is calculated and in the other part, the first is the cell that contains the fringe centre; (iii) every background cell whose centre is within 0.5 of the cylinder centre is a hole; (iv) the component fringe cells are exactly the 96 cells of the outer ring; (v) at least one hole and one background fringe cell | exact |
| K5 | refusals: `compose` with a duplicated patch name (prefix empty; both parts have `frontAndBack`); an outer patch that does not exist; a pose with the cylinder centre at `(0, 19)` (the component crosses the background boundary) | `Err` each, naming `frontAndBack`, the patch, `donor` |

### Q. Equivalence with the library step

| Id | Check | Limit |
| --- | --- | --- |
| Q0 | BF for 20 steps with `OversetSolver::advance_mesh` (every cell calculated) against `PimpleSolver::step_ale_with_walls`: the largest difference of `u` (over `U`), `p` (over `U^2`) and `phi` (over `max abs(phi)`) after the 20 steps; printed with it, no limit: whether they are equal bit for bit, and the R1 hash of the overset run | `<= 1e-12` each |

### U. Uniform flow through a moving, rotating component

| Id | Check | Limit |
| --- | --- | --- |
| U1 | U, every step, over the non-hole cells: `max abs(u_c - U) / abs(U)` and `max abs(p_c) / abs(U)^2` | `<= 1e-10` each |
| U2 | U, every step, the GCL residual of the component cells `max_c abs(V1_c - V0_c - dt sum_f sigma_cf m_f) / mean(V)` | `<= 1e-12` |
| U3 | U, every step, the continuity of the calculated cells and `abs(Q_xmax - U_x A_xmax) / (U_x A_xmax)` | `<= 1e-9`, `<= 1e-10` |
| U4 (set-up) | U, the number of background cells whose kind changes, summed over the steps | `>= 20` |
| N2 (negative control) | U with every stencil weight multiplied by 1.001 after `prepare`: the largest `max abs(u_c - U) / abs(U)` | `>= 1e-6` |

### T. Accuracy: the Taylor-Green vortex across a moving component (closed form)

| Id | Check | Limit |
| --- | --- | --- |
| T1 | the observed order `log2(e(TG_1) / e(TG_2))` | `>= 1.7` |
| T2 | `e(TG_L) <= 2 e(TGB_L)` for `L = 1, 2` | as stated |
| T3 | TG_1 and TG_2, every step: the continuity of the calculated cells (the closed-domain conservative pressure, M7) | `<= 1e-9` |
| N3 (negative control) | TG_1 with the component mesh flux set to zero after every `prepare` (the ALE convection in the wrong frame) | `e >= 2 e(TGB_1)` |
| N4 (negative control) | TG_1 with every stencil replaced by its donor cell with weight 1 after every `prepare` (first-order interpolation) | `e >= 2 e(TGB_1)` |

Printed, no limit: `e(TGB_L)` and its observed order, `delta` per level.

### A. The acceptance: a cylinder moving through a background mesh

Set-ups TR and BF, run in the same test. `Q_out` is the net flux through `xmax` after each step, `U D L = 0.1`.

| Id | Check | Limit |
| --- | --- | --- |
| A1a (acceptance, mass) | TR, every step: the continuity of the calculated cells | `<= 1e-9` (ADR 0101) |
| A1b (acceptance, mass) | TR, every step: `abs(Q_out) / (U D L)` | `<= 1e-9` |
| A2 (acceptance, drag) | every step `n` with `t_n` in `[2, 8]` (301 steps): `abs(F_x,TR - F_x,BF) / abs(F_x,BF)` | `<= 5e-2` |
| NM (negative control) | TR with `MassMode::Interpolated`, the first 100 steps: the largest `abs(Q_out) / (U D L)` | `>= 1e-6` |

Printed, no limit: `C_D` of both runs at `t = 2, 4, 6, 8`, the mean of `C_D` and of the relative difference over the window,
the step of the largest difference, the range of `delta / (U^2 / 2)`, the largest continuity of BF, the pressure iterations
per step and the wall time of both runs.

### X. Set-up of TR

| Id | Check | Limit |
| --- | --- | --- |
| X1 | every step: every background cell whose centre lies within 0.5 of the cylinder centre is a hole | exact |
| X2 | every step: 96 component fringe cells, at least one hole and one background fringe cell | exact |
| X3 | the number of background cells whose kind changes, summed over the 400 steps | `>= 500` |
| X4 | every linear solve of TR and BF converged (the solver's `converged` flag) | all |
| S0 (smoke) | TR for 5 steps: finite fields; the wall time per step is printed (run before the registered checks, to size the test) | finite |

### E. Refusals

| Id | Check | Limit |
| --- | --- | --- |
| E1 | `OversetSolver::new` with BDF2, `n_outer = 2`, one non-orthogonal corrector, linear-upwind convection, the consistent correctors | `Err` each |
| E2 | TR at pose `s = 0`, a step to `s = -1.5`: refused ("uncovered"), and the solver's `u`, `p`, `phi` and time unchanged bit for bit | as stated |
| E3 | `advance` with a step prepared on U's composite on a solver of TR | `Err` |

### B. Defaults

| Id | Check | Limit |
| --- | --- | --- |
| D-R | the record R1 after the code | equal (Windows; printed elsewhere, ADR 0137) |
| D-S | the existing suites, unedited: the `rcfd-mesh` unit tests (`gen`, `motion`), `rcfd-fv` `ale_flows`, `ale_gcl`, `sliding_interface_f250`, `layering_f257`, `cylinder`; `rcfd-api` `baseline_hashes`, H1 of `case_hashes_w48u`, `dynamic_mesh` | pass (the gate runs every test) |

### Mutations (scratch, not committed)

Each is applied alone to the final tree, the named tests are run once, and they must fail; the file is restored and
checked byte for byte against a kept copy. Their numbers go in the addendum. `scripts/limit_audit.py` is run on the new
test files and every hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `linear_weights`: inverse-distance weights (only `sum w = 1`) | K3 |
| MU2 | `advance`: `delta` left at zero in the conservative mode | A1b |
| MU3 | `prepare`: the component mesh flux set to zero | T2 |
| MU4 | `advance`: `HbyA` of the fringe cells from their own matrix rows, not interpolated | U1 |
| MU5 | `CellLocator::locate`: the cell with the nearest centre, no containment rule | K2 |
| MU6 | `connectivity`: no fringe layer in the background (holes only) | K4 |

### The gate

`cargo fmt` per package, `cargo clippy --workspace --all-targets -- -D warnings`, and every test in release on a
`git archive` copy of the gated commit, one process per test, three jobs (`scripts/gate_test.py`), in the target dir
`E:/projects/cfd/w49i-work/target-f134-gate`, taken in turn through the machine's gate lock (the 5 s poll of
`gate-f291/outer2.sh`), with the free physical memory at the start disclosed (start threshold 19 GB). A failure of a known
flaky test is run again alone on the same build and reported with its name.

### Classes

D-R and D-S are preconditions of landing: a miss means the change is not merged and the cause is recorded. A miss of any
other limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. A set-up check
(U4, X1 to X4, S0) or a negative control (N2, N3, N4, NM) that does not hold, or a mutation that does not make its check
fail, means the check is not shown to be able to fail: it is reported as a defective set-up, corrected by an amendment
before any re-run, and never counted as a pass.

**Decision rule.** The acceptance checks are A1a and A1b (mass) and A2 (drag), with D-R and D-S. F-134 is set **done**,
at the library scope registered in reading 4, when every registered check is met, every set-up check and negative control
held and every mutation failed its tests; the note says that no case can express an overset mesh yet (Q1). A miss of any
registered check (acceptance or not): **partial**, with the miss named. The overset step does not run at all: todo.

## Not delivered

The case-tree, CLI and Studio wiring (Q1); more than one component, components that overlap each other, donor chains
(a fringe cell whose donors are fringe cells) and a moving background (Q2); hole cutting from the component's wall
geometry instead of a user function (Q3); interpolation of higher than linear order and an interpolation that conserves
fluxes face by face (Q4); more than one fringe layer; BDF2, outer iterations, non-orthogonal correctors, linear-upwind or
TVD convection, turbulence, energy, scalars, a face viscosity, body forces and sources on the overset path (refused); the
compressible solver; parallel runs.

## Open questions and choices for the maintainer

- **Q1. The case-tree wiring.** Not in this registration. Options: (a) a follow-up item and registration: an optional table
  (for example `[mesh.overset]` with the component mesh, its motion, the outer patches and a hole shape) whose absence keeps
  every case bit-identical, and the transient loop in `sim.rs` calling `OversetSolver`; (b) keep the library only.
  Recommendation (a), as a new roadmap item (the maintainer's decision; no item exists for it).
- **Q2. Several components.** Options: (a) a separate item (donor chains, overlap priority, a moving background);
  (b) extend F-134. Recommendation (a).
- **Q3. Hole cutting.** Options: (a) the user function, as registered; (b) cut by the component's wall surface plus a
  distance. Recommendation (a) now, (b) with Q1.
- **Q4. Conservation.** Options: (a) a global offset `delta` that makes the mass of the domain exact (M7, as registered);
  (b) face-by-face conservative interpolation of the fluxes across the overlap. Recommendation (a): it keeps the
  continuity of every calculated cell and the mass of the domain exact with one scalar per step.

## Amendment 1 (2026-11-07, written after the smoke run S0 and before any registered check ran)

S0 (registered as the run that sizes TR) ran TR for 5 steps with the code as registered. Its own check held (finite
fields). It showed 2.6 s per step and, in every step, one linear solve that did not converge: the response `p1` to the
unit offset (M7, solved from zero), on which BiCGStab with ILU(0) stopped at its cap of 3000 iterations with a residual of
`1.4e-3` (step 1) and `2.6` (step 2) against `4.5e-2` at the start. The largest continuity of the five steps, `1.7e-9`,
followed from it.

Diagnostic (disclosed; a temporary switch of the solver pair in the source, removed afterwards; steps 1 to 5 of S0, once
each), for the overset pressure systems:

| Pair | Iterations per solve | Converged | Time per step |
| --- | --- | --- | --- |
| BiCGStab, ILU(0) (as registered) | 175 to 187, and 3000 for `p1` | no (`p1`) | 2.6 s |
| GMRES(50), ILU(0) | 830 to 1350 | yes | 4.6 to 5.3 s |
| BiCGStab, one AMG cycle | 1 to 31 | yes | 0.50 s |
| GMRES(30), one AMG cycle | 2 to 21 | yes | 0.42 s |

`delta` of step 1 was `3.769e-7` with each of the three converged pairs and `2.926e-7` with the unconverged `p1`.

Change, registered here before any registered run: **S3 is amended.** A pressure system with fringe or hole rows is solved
by restarted GMRES(30) right-preconditioned with one cycle of the library's aggregation AMG (default settings) of its
matrix, the pair of `solve_pressure_implicit_non_orth`. The matrix and its hierarchy are built once per step, as `p1`
(they do not change between the correctors, M7). A momentum system with fringe or hole rows stays on BiCGStab with ILU(0)
(4 to 11 iterations in S0). The systems without such rows keep the library's calls. No limit, set-up or other check
changes.

X4 for BF: `PimpleSolver` does not return the converged flags of its solves. X4 for BF is read as "the pressure
iterations of every step below 3000", which excludes a pressure solve that stopped at its cap; the momentum solves of BF
are not covered.

## Amendment 2 (2026-11-07, written after the registered run of T and before any corrected run)

The registered runs of K1 to K5, D-R, Q0, U1 to U4, N2 and E1 to E3 met every limit and held every negative control (the
numbers are in the addendum). The registered run of T and N3/N4 (once, release, 27 s and 7 s) met T2 at level 1 (`1.404`
against 2) and T3 (`5.2e-14`), held N3 (`220 e(TGB_1)`) and N4 (`57.2 e(TGB_1)`), and missed:

| Id | Limit | Measured |
| --- | --- | --- |
| T1 | `>= 1.7` | `0.747` (`e(TG_1) = 1.079e-3`, `e(TG_2) = 6.43e-4`) |
| T2 at level 2 | `<= 2` | `3.313` (`e(TGB_2) = 1.94e-4`) |

The background alone converges at order `1.99` (`7.68e-4`, `1.94e-4`).

Diagnostic (disclosed; a scratch test, not committed, once): TG_L with the registered `dt = 0.0025` and with `dt =
0.00125` (400 steps), the component moving as registered or held at its start pose, conservative and interpolated:

| Run | `dt = 0.0025` | `dt = 0.00125` |
| --- | --- | --- |
| TG_1 moving, conservative | `1.079e-3` (`1.40 e_B`) | `1.041e-3` (`1.35 e_B`) |
| TG_1 at rest, conservative | `1.496e-3` (`1.95 e_B`) | `1.503e-3` (`1.95 e_B`) |
| TG_2 moving, conservative | `6.43e-4` (`3.31 e_B`) | `3.83e-4` (`1.98 e_B`) |
| TG_2 at rest, conservative | `3.52e-4` (`1.82 e_B`) | `3.51e-4` (`1.82 e_B`) |
| interpolated (no `delta`), all eight runs | `1.04e-3` to `3.06e-3` (`3.1` to `6.5 e_B`), continuity `2.7e-3` to `7.6e-3` | |

At rest the overset error converges at order `2.09`. The excess of the moving component at level 2 halves with the time
step. It is the time error of the cells that move: backward Euler errs by about `(dt/2) |d^2 u/dt^2|` along the path of a
cell, and on a cell moving with the speed `w` the derivative along its path is `du/dt + (w . grad) u`; with `|w|` from 0.6
to 0.9 here that gives an error of the size of the spatial error of level 2. The registration judged the time error from
the background, where only `du/dt = -2 nu u` acts. T1 and T2 at level 2 as registered measure that time error together
with the spatial one: a defective set-up. (Printed in the same diagnostic: without `delta`, the closed domain is not
consistent and the soft pin takes the defect, a continuity of `3e-3` to `8e-3`, as M7 says.)

T1 and T2 at level 2 stay as registered, as the ignored test `t_known_miss_order_and_fine_level_ratio`; the gate-run test
of the registered set-up asserts T2 at level 1 and T3. Corrected checks, registered here before they run, each run once:

| Id | Set-up | Check | Limit |
| --- | --- | --- | --- |
| T1c | TG_1 with `dt = 0.0025` (200 steps), TG_2 with `dt = 0.000625` (800 steps), so that the time error falls with `h^2` as the spatial error does | `log2(e(TG_1) / e(TG_2))` | `>= 1.7` |
| T2c | the same, with TGB_L at the same steps | `e(TG_L) <= 2 e(TGB_L)` for `L = 1, 2` | as stated |
| T3c | the same | every step the continuity of the calculated cells | `<= 1e-9` |

N3 and N4 keep the registered set-up (level 1, `dt = 0.0025`). MU3 (must fail T2) is read as failing T2 at level 1 or
T2c. The decision rule is unchanged: T1 and T2 at level 2 are missed as registered, so F-134 cannot be done under it.

## Addendum 1: what was built and the results (2026-11-07)

Measured against the limits registered above and in amendments 1 and 2; no limit was changed. Release builds on Windows.
The registration is 463d775 (with the record R1); the code and the tests of the registered runs, with amendment 1, are
25ee122; amendment 2, its corrected checks and the known-miss tests are 9d52fec. Each registered check ran once on the
code it is committed with (the runs are deterministic; none was repeated to change a result). D-R ran first, then the K
checks, then Q0, U, N2 and E, then T with N3/N4; amendment 2 was written after T and before the corrected run; A and NM
ran after T (they do not depend on it), and the corrected T checks last.

**Built as registered** (S1, S2, S4, S5; S3 as amended by amendment 1).

- `crates/rcfd-mesh/src/overset.rs` (new): `compose`, `Composite` (`points_at`, `background_cells`, `component_cells`),
  `pose_inverse_apply`, `containment_margin`, `CellLocator` (`locate`, `locate_brute_force`), `linear_weights`,
  `CellKind`, `Stencil`, `Connectivity`, `HoleFn`, `OversetSpec`, `Overset` (`new`, `connectivity`, `neighbours`, the two
  locators), with the unit tests K1 to K5 and a pose-inverse test. `crates/rcfd-mesh/src/gen.rs`: `rectilinear_2d`
  added; nothing else in it is edited.
- `crates/rcfd-fv/src/overset.rs` (new): `MassMode`, `OversetCoupling`, `OversetStep`, `OversetInfo`, `OversetSolver`
  (`new`, `prepare`, `step`, `advance`, `advance_mesh`). `pimple.rs`, `moving.rs`, `ale.rs` and every other existing
  function are not edited.
- Tests: `crates/rcfd-fv/tests/overset_f134.rs` (14 tests: 12 run, 2 ignored known misses) and the record file of the
  registration (1). After the registered run of T, its test was split as amendment 2 says: the gate-run test of the
  registered set-up asserts T2 at level 1 and T3; T1 and T2 at level 2 are in `t_known_miss_order_and_fine_level_ratio`;
  the corrected test asserts T2c and T3c, and T1c is in `t_corrected_known_miss_order`. The helpers of the TG runs take the
  step count since then; `0.5 / 200` is the registered `0.0025`, so the registered runs are unchanged. No existing test
  was edited.
- Documents: this ADR, `docs/dynamic-mesh.md` and the README's gap list (prose only), and the F-134 entries of
  `docs/ROADMAP.md` and `docs/roadmap.json` (partial, by the decision rule).
- Not changed: `rcfd-api`, `rcfd-cli`, the Studio, the case schema; `baseline_hashes.rs`, `case_hashes_w48u.rs` and the
  goldens.

### Results

Run times on one test thread: the A test with NM 174 s (TR 129 s, BF 24 s), T 34 s, N3/N4 7 s, the corrected T 85 s, the
rest under 6 s; the K unit tests 2.3 s.

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| K1 composite | exact | 20,350 + 2880 cells; centres, volumes, patches equal | met |
| K2 locator | exact | every centre found; 2000 points per part equal to the full search (background 2000 inside; component 1212 inside, 788 outside) | met |
| K3 weights, five poses | `<= 1e-13`, `<= 1e-12` | `abs(sum w - 1)` `6.7e-16`; linear field `5.0e-17` of `max abs(f)`; smallest stencil 5; weights in `[-0.034, 0.997]` | met |
| K4 kinds, five poses | exact | holds; 316 holes, 58 or 60 background fringe and 96 component fringe cells | met |
| K5 refusals | named | `frontAndBack`, `nope`, `no donor` | met |
| Q0 overset step with every cell calculated against the library | `<= 1e-12` | `0` for `u`, `p`, `phi`: equal bit for bit, hash `af2c9e6bd3e5596d` (that of R1) | met |
| U1 uniform flow: velocity, pressure | `<= 1e-10` each | `1.40e-14`, `6.21e-13` | met |
| U2 GCL of the component | `<= 1e-12` | `5.0e-16` | met |
| U3 continuity, outflow | `<= 1e-9`, `<= 1e-10` | `1.58e-14`, `2.4e-15` | met |
| U4 (set-up) kind changes | `>= 20` | 245 | held |
| N2 (negative control) weights times 1.001 | `>= 1e-6` | `1.60e-3` | held |
| T1 observed order, as registered | `>= 1.7` | `0.747` | **missed** (amendment 2) |
| T2 level 1, as registered | `<= 2` | `1.404` | met |
| T2 level 2, as registered | `<= 2` | `3.313` | **missed** (amendment 2) |
| T3 continuity, as registered | `<= 1e-9` | `5.2e-14` | met |
| T1c observed order, `dt` proportional to `h^2` | `>= 1.7` | `1.618` (`e(TG_1) = 1.0785e-3`, `e(TG_2) = 3.514e-4`) | **missed** |
| T2c level 1, level 2 | `<= 2` | `1.404`, `1.821` (`e(TGB_2) = 1.930e-4`) | met |
| T3c continuity | `<= 1e-9` | `6.1e-14` | met |
| N3 (negative control) zero mesh flux | `>= 2 e(TGB_1)` | `220 e(TGB_1)` | held |
| N4 (negative control) donor cell only | `>= 2 e(TGB_1)` | `57.2 e(TGB_1)` | held |
| A1a (acceptance, mass) continuity, 400 steps | `<= 1e-9` | `8.1e-17` | met |
| A1b (acceptance, mass) net outflow / `U D L`, 400 steps | `<= 1e-9` | `7.5e-15` | met |
| A2 (acceptance, drag) every step of `[2, 8]` | `<= 5e-2` | `6.25e-3` (largest at step 399, `t = 7.98`) | met |
| NM (negative control) interpolated overlap, 100 steps | `>= 1e-6` | `2.69e-3` | held |
| X1 holes inside the body | exact | every step | held |
| X2 fringe and hole counts | exact | every step | held |
| X3 kind changes | `>= 500` | 6300 | held |
| X4 converged solves | all | TR: no unconverged solve; BF: at most 310 pressure iterations in a step | held |
| S0 (smoke) | finite | finite (2.6 s per step before amendment 1) | held |
| E1 refused settings | `Err` each | BDF2, `n_outer = 2`, a non-orthogonal corrector, linear upwind, the consistent correctors | met |
| E2 uncovered hole cell | `Err`, state unchanged | "cell 8452 was a hole and is uncovered", `u`, `p`, `phi`, time unchanged | met |
| E3 step of another composite | `Err` | "the step does not fit the solver's mesh" | met |

Printed, no limit:

- A: `C_D` overset / body-fitted at `t = 2` `2.0005` / `2.0031`, `t = 4` `1.8226` / `1.8267`, `t = 6` `1.7228` / `1.7292`,
  `t = 8` `1.6675` / `1.6739`; mean over `[2, 8]` `1.7877` / `1.7933`, mean relative difference `-0.32 %`; `delta / (U^2/2)`
  in `[-2.0e-3, 7.4e-3]`; continuity of BF `6.1e-16`; 60 pressure iterations per TR step (three solves).
- T (registered): background alone `7.68e-4`, `1.94e-4` (order 1.99); `delta` in `[7.5e-4, 2.9e-3]` (level 1) and
  `[4.3e-4, 1.2e-3]` (level 2). Corrected: background alone `7.68e-4`, `1.93e-4` (order 1.99).

**Reading.**

- The acceptance is met. The cylinder moves 7.5 diameters through the background while 6300 background cells change kind,
  every calculated cell keeps its continuity to `1e-16`, and the domain keeps its mass to `7.5e-15` of `U D L`. The plain
  interpolated overlap loses `2.7e-3` of `U D L` through the outlet (NM), and with `delta` forced to zero (MU2) `8.5e-3`:
  the offset of M7 is what conserves the mass. The drag stays within `0.62 %` of the body-fitted run at every step of the
  window, `0.32 %` below it on average. The difference grows slowly with the distance travelled (`-0.13 %` at `t = 2`,
  `-0.38 %` at `t = 8`) as the wake moves from the component onto the background, whose resolution and outer boundary
  differ from those of BF.
- The overset step is the library's step: with every cell calculated it gives the bits of R1 (Q0), and a uniform flow
  through a component that moves and turns, with the hole and the fringe rebuilt every step, stays uniform to `1e-14`.
- T1 and T2 at level 2 as registered missed through the time error of the moving cells (amendment 2). With the step
  proportional to `h^2` the overset error is within 1.82 times that of the background alone at both levels (T2c), but its
  observed order is `1.62` against `1.7` (T1c missed). A diagnostic after the corrected run (disclosed; a scratch test,
  not committed, once) located the largest error: at both levels, moving and at rest, it lies in a component fringe cell
  (an interpolated cell of the outer ring). The largest background error converges from `8.94e-4` to `2.27e-4` (order
  1.98), the root mean square over the calculated background cells from `5.60e-4` to `1.42e-4` (order 1.98), and at rest
  the largest error from `1.496e-3` to `3.51e-4` (order 2.09). The largest component error (`1.0785e-3`, `3.514e-4`)
  depends on where the fringe centres fall in their donor stencils, which differs between the levels and the poses. It is
  not diagnosed further here (Q5).

### Mutations

Each was applied alone to the code of 9d52fec by an exact text replacement (`E:/projects/cfd/w49i-work/f134-mut/mutate.py`,
which checks that the replaced text occurs once), the named test was run once in its own target dir, and the file was
restored and compared byte for byte with the kept copy (equal in all six).

| Id | Must fail | Result |
| --- | --- | --- |
| MU1 inverse-distance weights | K3 | failed: linear field `1.18e-3` of `max abs(f)` (`sum w - 1` stays `3e-16`) |
| MU2 `delta` left at zero | A1b | failed: `8.49e-3` of `U D L` |
| MU3 component mesh flux zero | T2 | failed: T2 at level 1 `220` (and the printed T2 at level 2 `733`) |
| MU4 fringe `HbyA` from the own rows | U1 | failed: velocity `3.13e-2`, pressure `0.469` |
| MU5 nearest centre, no containment | K2 | failed: a point located in cell 549 against 550 by the full search |
| MU6 no background fringe layer | K4 | failed: "calculated cell 8267 next to a hole" |

`scripts/limit_audit.py` reports two hits in the new test file, both class (iii):
`n2_uniform_flow_negative_control_perturbed_weights` and `n3_n4_taylor_green_negative_controls` assert their `>=`
expectations through `Checks::at_least`, which the scanner does not see; both are false positives. It does not scan the
unit tests in `src`.

Disclosures:

- Before the registered runs the new code and tests were compiled without running, laid out by rustfmt, and `cargo
  clippy -p rcfd-mesh -p rcfd-fv --all-targets -- -D warnings` was run (clean). S0 ran before amendment 1, as registered.
- The diagnostics of amendments 1 and 2 and the one in the reading above ran from a temporary switch in the source and
  from scratch tests, removed afterwards; their logs are in `E:/projects/cfd/w49i-work` (`f134-diag-*.log`).

### Further open questions

- **Q5. The order of the overset error.** Options: (a) keep T1c as a recorded known miss (T2c holds at both levels and
  the background converges at order 2); (b) register, with the follow-up of Q1, an order check on a third level (TG_3) or
  on a norm that does not pick single fringe cells, with the fringe-cell error printed; (c) interpolation of higher
  order. Recommendation (b).
- **Q6. The status of F-134.** The acceptance (A1, A2) is met; T1 and T2 at level 2 as registered, and T1c, are missed.
  Options: (a) partial, as the decision rule says; (b) done, if the maintainer does not count the accuracy checks outside
  the acceptance. Recommendation (a), with the wiring (Q1) and Q5 as the remaining work. The roadmap follows the decision
  rule until the maintainer decides.

### Defaults

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| D-R record R1 after the code | equal | `af2c9e6bd3e5596d`: equal | met |
| D-S the existing suites, unedited | pass | in the gate (below) | see the gate |
