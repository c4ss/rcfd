# ADR 0256: F-139 registration: a co-simulation participant interface (in-process) and a mock structural participant on a non-matching wall mesh

Status: **REGISTERED 2026-11-12**, written before any code of F-139 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no check was run: the code and the documents listed under "What was read" were read, the
arithmetic under "Arithmetic before this registration" was done (a scratch script on a model problem, no flow solver), and
the unchanged tree of main was compiled once (`cargo test --release -p rcfd-fv --test strong_fsi_f251 --no-run`, to fill the
target directory of the new worktree; nothing was executed). Results go in the addendum at the end; nothing above the
addendum is changed after a run.

Parents: [ADR 0250](0250-f251-registration-strong-fluid-body-coupling.md) (F-251: `rcfd_fv::fsi::strong_step`, the
sub-iteration, its relaxation and the added-mass argument), [ADR 0252](0252-f351-registration-added-mass-deficit-wall-pressure.md)
(F-351: the wall pressure of a prescribed-gradient wall), [ADR 0211](0211-f244-registration-flow-coupled-six-dof-validation.md)
(F-244: the spring-mounted piston), [ADR 0244](0244-f237-registration-heat-rate-load-export.md) and ADR 0051 (the one-way load
mapping of `rcfd_post::structural`, consistent and conservative), [ADR 0033](0033-wave20b-dynamic-mesh.md) (the moving-mesh
step). Roadmap item F-139.

Number: the lead gave 0256 to F-139. Work from main cddd5989, worktree `w49i-work/f139`.

House rules (ADR 0130, ADR 0211, ADR 0250):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away; relaxation factors and tolerances are fixed here from the derivation;
- a defective set-up gets a new corrected test by an amendment written and committed before the corrected run, and the old
  one stays as an ignored record;
- defaults stay bit-identical: nothing existing changes its result, the interface is new and nothing calls it;
- `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables and the goldens are not edited or re-recorded; no existing
  test is edited; no case-schema key, no CLI or MCP tool, no network transport;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no unsafe code, no commit trailers, no push.

Clean-room. The protocol and every formula are derived in this text from first principles and from the needs of the model
problem. No paper, book, code, vendor document or coupling library was opened for this work, and no interface of a
third-party coupling library is copied. Names of the ideas, cited from memory (so nothing is taken on the authority of a
citation; what is used is derived below): the added-mass instability of partitioned schemes (P. Causin, J.-F. Gerbeau, F.
Nobile, Comput. Methods Appl. Mech. Engrg. 194 (2005) 4506-4527); Aitken's dynamic relaxation of the interface iteration (B.
M. Irons, R. C. Tuck, Int. J. Numer. Methods Engrg. 1 (1969) 275-277; U. Küttler, W. A. Wall, Comput. Mech. 43 (2008) 61-72);
the pairing of a consistent displacement transfer with its transpose for the loads, which conserves the virtual work (C.
Farhat, M. Lesoinne, P. Le Tallec, "Load and motion transfer algorithms for fluid/structure interaction problems with
non-matching discrete interfaces", Comput. Methods Appl. Mech. Engrg. 157 (1998) 95-114); the potential-flow added mass of a
wall mode in a channel (separation of variables for Laplace's equation, any text on hydrodynamics, e.g. H. Lamb,
"Hydrodynamics"); the elastic foundation of independent springs (E. Winkler, 1867).

## What F-139 asks

The acceptance text of `docs/roadmap.json`, binding:

> A mock structural participant exchanges wall displacement/force with rcfd and converges a coupled iteration; the protocol
> is documented

Clauses: (c1) a mock structural participant; (c2) it exchanges wall displacement and force with rcfd; (c3) a coupled
iteration converges; (c4) the protocol is documented.

Dependencies (both `done` in the roadmap). **F-085** (the solver service) provides a JSON-RPC 2.0 transport over TCP and
stdio with sessions, run/pause/step/stop, an event log and a versioned protocol document: the transport that could later
carry the calls defined here; it is not used by this item (the scope is in-process). **F-063** (dynamic mesh) provides what
the fluid side of the exchange needs: the swept-volume moving-mesh step with the geometric conservation law, the smoothing
of an interior mesh from a wall displacement, and the wall velocity and the fixed-flux pressure condition of a wall that
follows the mesh (`ale::move_mesh`, `ale::set_follow_conditions`, `PimpleSolver::step_ale_with_walls`).

The roadmap note lists a wider scope (radial-basis mapping, an unmapped-value policy, ramping, quasi-Newton stabilisation,
extrapolated initialisation, several participants). None of it is in the acceptance; it is listed under "Limits" and goes
to follow-up items.

## What was read (no run)

1. ADR 0250 in full and `crates/rcfd-fv/src/fsi.rs`: `strong_step_with` saves the geometry, the velocity field with its
   conditions, the solver snapshot, the history, the two lag lists and the body state, and puts them back before every
   repeated iterate; `Relaxer` is Aitken's factor for a 3-vector; the convergence test is `|r|/max(|Ft|, scale) <= tol`.
2. ADR 0252 (the registration, amendment 3 and the addendum) and `sixdof.rs` `force_moment`: the default wall pressure is the
   cell value extrapolated with a least-squares gradient that sees the stored zero-gradient wall value, low by a quarter
   cell on a prescribed-gradient wall (`1/(4N)` on a plug column); `patch_force_moment_wall_gradient` replaces the normal
   part by the gradient the pressure equation used, `p_c + dn g`, second order (error `p_nn dn^2/2`). Both return the
   resultant only; the per-face loads are not available to a caller.
3. ADR 0211 (the piston SP, the time-step limit `nu dt/dx^2` of amendment 1, here `4e-4`), `ale.rs`, `docs/dynamic-mesh.md`.
4. ADR 0244, `docs/interchange-formats.md` and `crates/rcfd-post/src/structural.rs`: one-way export; pressure by overlap
   weights between two parametrisations (each target value a weighted mean, rows summing to 1: consistent), heat flux
   also in a conservative form (the integral preserved). There is no transfer of a displacement to the flow mesh and no
   two-way exchange. `rcfd-post` depends on `rcfd-fv`.
5. `PimpleSolver::snapshot`/`restore` (`pimple.rs`, read only; not edited).

## The model problem and its closed form

**P1. Geometry.** A channel `0 <= x <= L`, `0 <= y <= W`, thickness `Lz`, fluid of density `rho` at rest. The sides are
slip walls, the end `x = L` is open at `p = 0`. The wall `x = 0` is flexible: it moves along x by `d(y, t)`. It is an
elastic (Winkler) foundation with surface mass: per unit area a mass `m` and a spring `k` about `d = 0`, no stiffness
between neighbouring points:

    m d_tt + k d = p_wall(y, t).

**P2. The fluid force (small amplitude, inviscid).** For `|d| << W` the convective term is of relative size `kappa |d|`
and is dropped; the pressure of the incompressible fluid then satisfies Laplace's equation with `p_x = -rho d_tt` on `x =
0`, `p_y = 0` on the sides and `p = 0` at `x = L`. Separation of variables with the side-wall modes `cos(kappa_j y)`,
`kappa_j = j pi/W`: for `d_tt = a_j(t) cos(kappa_j y)`

    j = 0:   p = rho a_0 (L - x)                                    p_wall = rho L a_0
    j >= 1:  p = rho a_j cos(kappa_j y) sinh(kappa_j (L - x)) / (kappa_j cosh(kappa_j L))
                                                                    p_wall = rho l_j a_j cos(kappa_j y),   l_j = tanh(kappa_j L)/kappa_j

(check: `p_x(0) = -rho a_j cos`, `p(L) = 0`, `p_xx + p_yy = 0`). The fluid acts on the wall along `-x` with `p_wall`: an
added mass per unit area `rho l_j` for mode `j`, with `l_0 = L`. The added-mass operator is diagonal in the cosine modes,
and so is the foundation, so **each cosine mode is a normal mode of the coupled problem**:

    omega_j^2 = k / (m + rho l_j),        l_0 = L,   l_j = tanh(j pi L/W) W/(j pi).

This is the closed form. Mode 0 is the piston of ADR 0250 (`omega_0^2 = k/(m + rho L)`); mode 1 has the wall rocking about
mid-height and the fluid sloshing across the channel near the wall.

**P3. Numbers (fixed now).** `L = 1`, `W = 0.4`, `Lz = 0.1`, `rho = 1`, mass ratio of mode 0 `mu_0 = rho L/m = 4` (`m =
0.25`), `k = (2 pi)^2 (m + rho L) = 49.348022` (`T_0 = 1`). Then `l_1 = 0.1273239`, `mu_1 = rho l_1/m = 0.5093`, `omega_0 =
6.283185`, `omega_1 = 11.436094` (`omega_1/omega_0 = 1.8201`). Start at rest with `d(y, 0) = X (1 + 0.5 cos(pi y/W))`, `X =
0.002`: modes 0 and 1 together, so the displacement and the force are fields with two time scales, not single numbers.
`kappa_1 X = 0.016`: the dropped convective term changes a frequency at second order, `2.5e-4`. Viscosity `nu = 1e-4`: the
decay rate `2 nu kappa_1^2 = 0.012/s`, the viscous normal stress at the wall `2 nu kappa_1/(l_1 omega_1) = 1.1e-3` of the
pressure and in phase with the velocity (no first-order frequency shift), the no-slip of the moving wall acts on the first
cell layer with `nu dt/(dn dx) = 8e-4` of its inertia; the diffusion number is `4e-4`, far below the limit of ADR 0211.

## The two meshes and the mapping

**G1. The meshes.** Flow: `box_mesh([20, 8, 2], [1, 0.4, 0.1])`, the wall is the patch `xmin` with 16 faces (8 across `y`,
`dy = 0.05`) and 27 points. Structure: 6 nodes on a line across `y`, `y_i = 0.08 i`, each with one degree of freedom `q_i`
along x, the displacement between nodes linear (hat functions `N_i(y)`), lumped: node `i` carries the mass `m Lz t_i` and
the spring `k Lz t_i`, `t_i` its tributary length (`0.08`, `0.04` at the two ends). The interior nodes (`0.08, 0.16, 0.24,
0.32`) fall inside flow faces: the meshes do not match.

**G2. Displacement, structure to flow (consistent).** Flow wall point `p` at `y_p`: `d_p = sum_i H_pi q_i`, `H_pi =
N_i(y_p)`. Each row has at most two entries and sums to 1, so a rigid translation is reproduced exactly, and so is any
field linear in `y`.

**G3. Force, flow to structure (conservative).** The flow gives a force vector `F_f` per wall face (newton, on the
structure). The displacement of a face is the mean of its points, `d_f = (1/n_f) sum_{p in f} d_p = sum_i B_fi q_i`, `B = P
H`. The nodal forces are the transpose,

    f_i = sum_f B_fi F_f = sum_p H_pi sum_{f containing p} F_f / n_f.

Then, identically for every `q` and every `F`: `sum_i f_i q_i = sum_f F_f . (B q)_f` (the work on the structural mesh is
the work on the flow mesh: both are `F^T B q`), and `sum_i f_i = sum_f F_f` (the columns of `P` and the rows of `H` sum to
1). The only error is rounding. A consistent (averaging) force map would not conserve either.

**G4. The work on the flow mesh is the pressure-volume work.** With the points moving along x only and the faces
rectangles in `(y, z)`, a face's displacement is bilinear and its swept volume is the projected area times the mean of its
four points, exactly; the x component of its area vector is that projected area, exactly, before and after. So the
pressure part of `F_f . d_f` is `rho p_f` times the swept volume, the work the flow step does on the fluid through the
mesh flux. For a general motion this holds to second order in the displacement (stated, not tested here).

## The protocol (design; the document of clause c4 is `docs/cosimulation-protocol.md`)

Two participants and a coupler. Every call carries plain numbers and text and no reference into the other side, so a
transport (F-085's JSON-RPC) can carry each as one request and one reply, unchanged.

| Call | To | Carries | Returns |
| --- | --- | --- | --- |
| describe | both, once | | the interface mesh in the reference configuration: structure, node coordinates [m]; fluid, wall point coordinates [m] and the points of every wall face |
| time | both | | the participant's time [s] |
| checkpoint | both | | (the participant keeps its full state at the current time) |
| restore | both | | (the participant is again at its checkpoint, bit for bit) |
| forces | fluid | | the force of the fluid on the wall, one vector per wall face [N], at the participant's time |
| advance (structure) | structure | `dt` [s]; one force vector per node [N], held over the step | node displacements from the reference [m] at `t + dt` |
| advance (fluid) | fluid | `dt` [s]; one displacement vector per wall point [m], the position at `t + dt` | face forces [N] at `t + dt` |

Units SI. Forces are forces of the fluid on the structure, in the global axes. Displacements are measured from the
reference configuration that `describe` returned, not from the last step. The mapping (G2, G3) belongs to the coupler:
the structure never sees the flow mesh, nor the reverse.

**One coupling step of size `dt`** (the coupling time step: both participants cover the same window `[t_n, t_n + dt]`; a
participant may take smaller steps of its own inside it; the coupler checks after every `advance` that both report `t_n +
dt` within `1e-9 dt` and that they agree at the start):

1. `checkpoint` both.
2. Predictor: `f^1 = B^T forces()`, the flow's force at `t_n`.
3. Iterate `k = 1, 2, ...`: (if `k > 1`: `restore` both) `q^k = structure.advance(dt, f^k)`; `F^k = fluid.advance(dt, H
   q^k)`; `ft^k = B^T F^k`; residual `r^k = ft^k - f^k`.
4. Converged when `|r^k|_2 <= tol max(|ft^k|_2, f_scale)` (Euclidean norm over all components of all nodes). Then the step
   is complete: the structure moved under `f^k`, the flow followed that motion, and the flow's force is `f^k` within the
   tolerance. No further solve.
5. Otherwise `f^{k+1} = f^k + w_k r^k` (`w_k` fixed or Aitken's, C2) and back to 3, at most `K` iterations.
6. Not converged at `K`: by the setting, either `restore` both and stop with an error (the default), or keep the last
   iterate and report `converged = false`.
7. A participant that returns an error, a non-finite value or a wrong array length, or reports a wrong time: `restore`
   both and stop with an error that names the participant, the time `t_n`, the iteration and the participant's message.
   After an error both participants are at `t_n`, bit for bit, and the caller may retry (another `dt`, other settings) or
   stop.

A repeated step (a caller that rejects a finished step) is `restore` both and step again: the checkpoints of step 1 stay
valid until the next `checkpoint`.

**Which wall pressure (ADR 0252).** The fluid participant evaluates its face forces with the prescribed gradient of the
fixed-flux wall (`p_c + dn g`, the wall value the pressure equation used; second order), not with the default
extrapolation of `patch_force_moment` (first order there, low by `1/(4N)` on mode 0, 1.25 percent of the added mass on 20
cells, `+0.37` percent of `omega_0`). The default evaluation stays selectable. The gradient vector of the last flow step is
part of the fluid participant's state: it is carried to the next step's predictor and saved and restored with the
checkpoint. On the first step (none yet) the predictor uses the default evaluation, which is the same for a fluid at rest.
This is a new interface, so the choice changes no existing result.

## The coupled iteration: convergence and acceleration

**C1. The map and why a plain fixed-point iteration diverges (the added-mass argument of ADR 0250 for a field).** The mock
structure advances each node with RK4 under a constant force: `q_{n+1} = q_n + dt v_n + dt^2 f/(2 M_s)` up to the spring
term, `M_s = diag(m Lz t_i)`. The flow step moves the wall with the displacement over the step, so a change `df` of the
nodal force changes the wall-face velocity of this step by `B dt df/(2 M_s)`, the acceleration the pressure equation sees
by `B df/(2 M_s)`, and the returned nodal force by

    d(ft) = -B^T M_a B (1/(2 M_s)) df = -(1/2) A_s M_s^{-1} df,        A_s = B^T M_a B,

`M_a` the added-mass matrix of the wall faces (the discrete form of P2: face forces per unit face acceleration). `A_s
M_s^{-1}` has the eigenvalues `lambda_j`, the mass ratios of the coupled modes (`lambda_0 = mu_0 = 4`, `lambda_1 ~ mu_1 =
0.51`, the others smaller), so the fixed-point map has the slopes `-lambda_j/2`, and the relaxed iteration multiplies the
error of mode `j` by `q_j(w) = 1 - w (1 + lambda_j/2)`. Without relaxation (`w = 1`) mode 0 is multiplied by `-mu_0/2 = -2`
per iteration: it **diverges whenever `mu_0 > 2`**, whatever the time step, because the incompressible pressure answers
the wall acceleration at once and in full (ADR 0250 D2, D5). The loosely coupled scheme (one pass per step) is its first
iterate and grows by `sqrt(mu_0/2)` per step (ADR 0250 D3). A fixed factor converges when `w < 2/(1 + mu_0/2) = 0.667`;
`w = 2M/(2M + m_a) = 1/3` (`fsi::relaxation_from_added_mass`) annihilates mode 0 and leaves `q_j = 1 - (1 + lambda_j/2)/3`,
up to `0.66` for the light modes: convergent, slow (30 iterations to `1e-8` on the model).

**C2. Acceleration.** Aitken's factor for a vector sequence (ADR 0250 D6, derived there): `w_k = -w_{k-1} r_{k-1} . (r_k -
r_{k-1})/|r_k - r_{k-1}|^2`, one factor for all components (they are all forces along the same axis), started every step
from `initial`, magnitude capped at `max`. With several modes of different slopes it no longer lands on the fixed point at
the third iterate; the model gives 8 to 11 iterations to `1e-8`.

**C3. Settings (fixed now).** Aitken with `initial = relaxation_from_added_mass(m, rho L) = 1/3` (the added mass of the
heaviest mode, mode 0) and `max = 2`; `tol = 1e-8`; `f_scale = k X W Lz = 3.9478e-3` N (the spring force of the whole wall
at `X`); cap `K = 20`; at the cap: stop. A force error at the tolerance displaces a node by `dt^2 df/(2 M_s) <= 1e-8 X k
dt^2/(2 m) = 1e-10 X` per step.

## Arithmetic before this registration (disclosed)

A scratch script (`w49i-work/f139-scratch/model.py`, not committed; no flow solver): the pressure of the wall faces from
the five-point Laplace equation on the `20 x 8` (and `40 x 16`) grid with the wall gradient prescribed and `p = 0` at the
outlet, the wall value `p_c + (dx/2) g`; the matrices `H`, `P`, `B` of G2, G3; the lumped nodes advanced by RK4 with a
constant force; the face force of a step `-M_a (w_{n+1/2} - w_{n-1/2})/dt` with `w` the face velocity from the displacement
over the step (the model of ADR 0250 with a matrix for the added mass); the iteration of the protocol. Modal coordinates:
`Q_0 = sum_i M_i q_i / sum_i M_i`, `Q_1 = sum_i M_i c_i q_i / sum_i M_i c_i^2`, `c_i = cos(pi y_i/W)`. Frequencies by upward
crossings, turning points by the parabola, as ADR 0250.

| Model, registered case (20 x 8 flow faces, 6 nodes, `mu_0 = 4`) | Value |
| --- | --- |
| eigenvalues `lambda_j` of `A_s M_s^{-1}` | 4.0138, 0.4761, 0.1907, 0.0874, 0.0314, 0.0161 (closed form: `mu_0 = 4`, `mu_1 = 0.5093`) |
| discrete coupled frequencies over the closed form (no time stepping) | 0.998621, 1.011183 |
| run, `T_0/100`, 650 steps: `omega_m/omega` of `Q_0`, of `Q_1`; upward crossings | 0.998625, 1.011470; 6, 12 |
| the same at `T_0/200` | 0.998636, 1.011209 |
| largest ratio of successive same-side turning points, `Q_0`, `Q_1` (`T_0/100`) | 0.9284, 0.9530 |
| iterations per step, Aitken from 1/3: largest, mean; histogram | 11, 8.64; 7: 8, 8: 286, 9: 288, 10: 67, 11: 1 |
| first step, Aitken: residual norms over the first | 1, 2.3e-2, 1.3e-2, 1.3e-4, 8.3e-6, 1.9e-6, 2.7e-6, 1.1e-7, 7.3e-8, 1.7e-8, 6.0e-10 |
| first step, `Fixed(1)`, cap 10: `|r_10|/|r_1|`; ratio per iteration | 519.9; 2.0036 |
| first step, `Fixed(1/3)`: iterations to `1e-8` | 30 |
| loose (one pass per step), `T_0/100`: first step with `|Q_0| >= 10 X`; end | 27; the wall leaves the mesh at step 36 |
| fluid force off: `omega_m/omega_0` of `Q_0` | 2.236061 (`sqrt(5)`) |
| default (extrapolated) wall pressure, modelled as a quarter cell short: `Q_0`, `Q_1` | 1.00366, 1.0275 |
| `40 x 16` flow faces, 6 nodes: `Q_0`, `Q_1` at `T_0/100` | 0.999975, 1.011349 |

Reading of the 1.1 percent of mode 1: it is the structure's own discretisation (six lumped nodes with a piecewise-linear
shape for half a cosine wave), not the flow mesh (the `40 x 16` row) and not the time step (the `T_0/200` row). The
`-0.14` percent of mode 0 on the coarse flow mesh is the non-matching quadrature: `B^T` applied to a uniform pressure does
not return exactly the tributary lengths of the lumped nodes (`lambda_0 = 4.0138` instead of 4).

## Design

**S1. Where it lives: `rcfd_fv::cosim`** (new file `crates/rcfd-fv/src/cosim.rs` and the line `pub mod cosim;` in
`lib.rs`). The fluid participant is the moving-mesh flow step, the wall conditions and the wall force, all of `rcfd-fv`;
`rcfd-post` (where the one-way load export lives) depends on `rcfd-fv`, so the module cannot use it without a cycle, and
the mapping needed here is a different one (points and a transpose, not face overlaps); `rcfd-api` holds the service of
F-085, but a transport is out of scope and its `sim.rs` and `schema.rs` are not touched. The crate graph is unchanged.

- traits `Participant` (`name`, `time`, `checkpoint`, `restore`), `Structure` (`node_coordinates`, `advance`), `Fluid`
  (`point_coordinates`, `face_points`, `forces`, `advance`): the calls of the protocol table;
- `InterfaceMap` (G2, G3): built from the rows of `H` (validated: indices in range, every row sums to 1 within `1e-12`)
  and the points of every face; `displacement_to_points`, `forces_to_nodes`, `face_displacements`; the helper
  `piecewise_linear_rows(node_coordinate, point_coordinate)` for a line of nodes;
- `CouplingSettings { relaxation (fsi::Relaxation), tolerance, force_scale, max_iterations, at_cap: AtCap::{Stop, Keep} }`,
  validated as `fsi::StrongCoupling`;
- `coupled_step(fluid, structure, map, dt, settings) -> Result<CoupledStep, CouplingError>`; `CouplingError { participant,
  time, iteration, message }`; `CoupledStep { iterations, converged, residuals, residual_norms, factors, node_forces,
  node_displacements, face_forces }`;
- `FlowWall`: the fluid participant, owning a `PimpleSolver`, the wall patch, `rho`, `nu`, the `fsi::WallPressure` choice
  (default of the constructor: `PrescribedGradient`), the carried wall gradient and the caller's function from the wall
  point displacements to all mesh points (the smoothing; it stays inside the participant). Its checkpoint is that of
  `strong_step_with` (geometry, velocity field with conditions, snapshot, history, lag lists) plus the carried gradient;
- `SprungWall`: the mock structural participant (lumped nodes with a mass and a spring each, one direction, RK4 with the
  force held over the step).

**S2. `sixdof.rs`.** The loop over the wall faces of `force_moment` is given a second, public entry
`patch_face_forces(geom, u, p, rho, nu, patches, wall_dpdn: Option<&[f64]>) -> Vec<Vec3>` (the load of every face in patch
order). The per-face arithmetic is moved, not changed, and the resultants are accumulated in the same order, so
`patch_force_moment` and `patch_force_moment_wall_gradient` return the same bits (check B2). `pimple.rs`,
`pimple/moving.rs`, `immersed.rs`, `overset.rs`, `fsi.rs`, `ale.rs` and `rcfd-api` are not edited.

**S3. Tests**: new file `crates/rcfd-fv/tests/cosim_f139.rs`; heavy ones `#[cfg_attr(debug_assertions, ignore)]`.

**Set-up CW** (the model problem): P3, G1; flow `PimpleSettings::piso(1e-4)`, central, solvers `1e-16`/`1e-13`/5000 (as
ADR 0250), `xmax` at `p = 0`, the four sides zero-gradient; Laplacian smoothing from the reference mesh with the wall
points prescribed; fluid at rest on the mesh of the initial wall; `dt = T_0/100`, 650 steps; settings C3.

**Set-up SPm1** (one rigid segment): the piston of ADR 0250 exactly (`box_mesh([20, 2, 2], [1, 0.2, 0.2])`, `nu = 0.01`,
`mu = 4`, `X = 0.01`, `T/100`), the structure one node with the piston's mass and spring, `H` a column of ones.

## Registered checks

All in `crates/rcfd-fv/tests/cosim_f139.rs`, release, each run once. "Relative" is `abs(a - b)/abs(b)`.

### Acceptance

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| A1 (c3) | CW: every step converged; iterations per step, largest and mean; the last test quantity of every step | all; `<= 16` and `<= 12`; `<= 1e-8` | 11 and 8.64 (model); cap 20 |
| A2 (c2, closed form) | CW: `omega_m/omega_0` of `Q_0`; `omega_m/omega_1` of `Q_1`; upward crossings | `abs(. - 1) <= 0.005`; `<= 0.03`; `>= 6` and `>= 11` | 0.998625; 1.011470 (the six-node structure); 6 and 12 |
| A3 (the error of A2 is the discretisation of the model, not the coupling) | the same two numbers against the model's | within `3e-3` of 0.998625; within `5e-3` of 1.011470 | the flow step differs from the model by viscosity, the convective term and the PISO splitting, each estimated at `1e-3` or less in P3; ADR 0252 found `4e-5` on the plug |
| A4 (stability) | CW: every ratio of successive same-side turning points of `Q_0` and of `Q_1` | `<= 1 + 1e-3` | 0.928, 0.953 (the lag of `dt/2`, ADR 0250 D4) |
| A5 (c2: fields are exchanged) | CW at every step: the spread of the nodal displacement `max_i q_i - min_i q_i` at step 0; the largest over the run of `max_i f_i/t_i - min_i f_i/t_i` relative to `k X Lz` | `= X` within `1e-12`; `>= 0.1` | `X`; mode 1 carries a force per length of the order of `k X Lz/2` |
| SN1 (negative control of A2) | CW with `rho = 0` in the fluid participant's force: `omega_m/omega_0` of `Q_0` | `abs(. - 1) >= 0.08` | `sqrt(5) = 2.236` |
| S5 (set-up) | CW: continuity of every step; `min volume ratio`; time of both participants at the end | `<= 1e-10`; `>= 0.9`; `6.5` within `1e-9` | ADR 0250 S5 |

### Mapping

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| M1 (consistent) | G1's map: a uniform nodal displacement at the wall points; a displacement linear in `y`; row sums | each within `1e-15` relative; `1e-15` | G2 |
| M2 (conservative) | G1's map, pseudo-random face forces and nodal displacements (fixed seed, 3 components): `sum f_i` against `sum F_f`; `sum f_i . q_i` against `sum F_f . d_f` | relative `<= 1e-13` each (to the sum of magnitudes) | G3: identities, rounding only |
| M3 (in the run) | CW, every step: the work of the last iterate's forces over the step on the two meshes; per wall face the swept volume `mesh_flux dt` against `S_x` times the mean point displacement over the step | relative `<= 1e-10` (to the sum of magnitudes over the wall) | G3, G4: rounding, with the cancellation of two positions |
| MN1 (negative control of M2) | the averaging force map (each nodal force divided by its number of contributing faces): `sum f_i` against `sum F_f` | relative `>= 0.1` | a consistent map loses the resultant |

### Checkpoint, failure, equivalence, controls

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| R1 (repeated step) | CW, after 37 steps: `checkpoint` is taken by step 38; after step 38 `restore` both: flow `u`, `p`, `phi`, time, mesh points, the velocity condition values, the carried gradient; structure `q`, `v`, time, against copies taken before step 38; then step 38 again against the first take (the same items, the iteration count and the residuals) | 0 values differ, both comparisons | a restored state is a copy; the solver is deterministic (ADR 0250 R1) |
| E1 (failure) | CW first step: (a) a structure that fails at its second `advance`; (b) a fluid whose mesh function fails at its second call; (c) a structure that reports a time off by `dt/2`; (d) a structure that returns a NaN; (e) `Fixed(1)`, cap 5, `Stop`; (f) the same with `Keep`; (g) invalid settings (`tolerance = 0`, `max_iterations = 0`, `Fixed(0)`, a NaN scale) and a map with a wrong node count | (a) to (e): an error naming the participant (`structure`, `fluid`, `structure`, `structure`, `coupler`), the time 0 and the iteration (2, 2, 1, 1, 5), and all items of R1 as before the call; (f) `Ok`, not converged, 5 iterations; (g) errors with the items as before; after (a) a clean step converges | protocol 6, 7 |
| Q1 (one rigid segment) | SPm1, 100 steps: `coupled_step` against `fsi::strong_step_with(PrescribedGradient)`: `max_n abs(x - x_strong)`; `abs(v - v_strong)` at the end; steps whose iteration counts differ; final `p` | `<= 1e-8 X`; `<= 1e-8 omega X`; `<= 2`; relative `<= 1e-6` of `max abs(p)` | the same algorithm; the nodal force is summed in another order (rounding), and a tolerance of `1e-8` on the force moves `x` by `1e-10 X` a step |
| N1 (negative control: plain fixed point) | CW first step, `Fixed(1)`, cap 10, `Keep`: converged; `|r_10|/|r_1|` | false; `>= 200` | 519.9 (C1: `-2.0036` per iteration) |
| N2 (negative control: loose coupling) | CW with `max_iterations = 1`, `Keep`, at most 650 steps: the first step with `abs(Q_0) >= 10 X`, or the step of the first error if earlier | `<= 60` | step 27 |
| C3 (the added-mass factor) | CW first step, `Fixed(1/3)`, cap 60: converged; iterations | true; `<= 45` | 30 |
| B1 (defaults) | the suites `sixdof`, `sixdof_coupled`, `sixdof_flow_f244`, `strong_fsi_f251`, `added_mass_f351` of `rcfd-fv` and `f237_heat_rate`, `w27c_structural` of `rcfd-post`, release, unedited; `cargo test -p rcfd-api --release --test baseline_hashes`; `git diff main` touches only `lib.rs` (one added line), `sixdof.rs`, `cosim.rs`, the new test file, this ADR and the documentation | all pass; exactly those files | S2 moves arithmetic without changing it |
| B2 (S2) | on CW after 5 steps and on SPm1 after 5 steps: the sum in face order of `patch_face_forces` (both evaluations) against `patch_force_moment` and `patch_force_moment_wall_gradient` | bit-identical | the same operations in the same order |

Printed, no limit: turning points, iteration histograms, Aitken factors of the first three steps, the step and manner in
which N2 ends, the frequencies with the default wall pressure are **not** run (the model's 1.00366 is quoted only).

### Mutations (scratch, not committed)

Each applied alone to the final tree by an exact text replacement that must occur once; the new test file is run once and
the named tests must fail; the file is restored and compared byte for byte with a kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `FlowWall::restore` does not put back the geometry | R1 |
| MU2 | `FlowWall::restore` does not put back the velocity field and its conditions | R1 |
| MU3 | `SprungWall::restore` does nothing | R1 |
| MU4 | `forces_to_nodes` averages (divides each nodal force by the sum of its weights' count) instead of the transpose | M2 |
| MU5 | Aitken's factor with the wrong sign | A1 |
| MU6 | the convergence test is met at the first iterate (the loose coupling) | A4 or A2 (the run diverges) |
| MU7 | after a participant's failure the participants are not restored | E1 |
| MU8 | `FlowWall` evaluates the force with the default extrapolation although `PrescribedGradient` is set | A3, Q1 |
| MU9 | `piecewise_linear_rows` gives both nodes of an element the weight 0.5 | M1 |
| MU10 | the coupler does not check the participants' times | E1 |
| MU11 | `patch_face_forces` leaves out the viscous part | B2 |

`scripts/limit_audit.py` is run on the new test file and every hit is read.

### After the code

`cargo test --release -p rcfd-fv --test cosim_f139 -- --include-ignored --test-threads=1`, the suites of B1, `cargo fmt -p
rcfd-fv --check`, `cargo clippy -p rcfd-fv --all-targets -- -D warnings`. The whole-workspace suite is not run.

## Predictions (mine, before any run)

A1 11 and 8.6; A2 `0.9986` and `1.011`; A3 inside `1e-3` for mode 0 and `2e-3` for mode 1; A4 `0.93`, `0.95`; N1 500; N2
step 27, ending with the wall through the mesh near step 36; C3 30; Q1 a difference of `1e-12 X` or exactly 0 and no step
with a different count; R1, B2 exact. The check I trust least is A3 for mode 1: the model has no viscosity, no wall
no-slip, no convective term and an exact pressure solve, and mode 1 (unlike the plug) has shear at the wall and a flow
that is not exactly reproduced by two PISO correctors.

## Decision rule

- **done**: the acceptance checks A1, A2, A4, A5 are met, SN1 and S5 hold, the protocol document exists and is linked,
  M1 to M3, R1, E1, Q1, C3, B1, B2 are met, the controls MN1, N1, N2 hold, A3 is met, and every mutation fails its tests.
  The acceptance stays the maintainer's.
- **partial**: the coupled iteration converges on CW (A1) and another acceptance check is missed (kept as `*_known_miss`,
  diagnosed); or the acceptance checks are met and one of A3, M1 to M3, R1, E1, Q1, C3, B1, B2 is missed and not shown by
  an amendment to be a defective set-up; or a mutation is not caught.
- **todo**: the coupled iteration does not converge on CW, or the exchange does not run.

A set-up check (S5) or a negative control (SN1, MN1, N1, N2) that does not hold means the check is not shown to be able to
fail: reported as a defective set-up, corrected by an amendment before any re-run, never counted as a pass.

## Open choices taken

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none changes a default.

- **T1. The force is the iterated quantity, the displacement is not relaxed.** As `strong_step`, so one rigid segment is
  ADR 0250's step (Q1). Options: relax the displacement (the usual choice when the structure is the expensive solver); the
  fixed point is the same.
- **T2. Aitken, one factor.** Options: a quasi-Newton update from the residual history, which would end on an affine map
  of six modes in at most seven iterations against the model's 8.6. Not taken now: it is in the roadmap note's wider scope
  and gets a follow-up item.
- **T3. The fluid participant's force uses the prescribed wall gradient** (ADR 0252), selectable.
- **T4. At the cap the default is to restore and stop** (ADR 0250 Q3's recommendation for a driver); `Keep` is a setting.
- **T5. The mapping belongs to the coupler and is built from weights**, with one helper for a line of nodes; no geometric
  search and no radial-basis map.
- **T6. The mock structure is a small library type** (`cosim::SprungWall`), so the protocol document can name a working
  example; nothing calls it.

## Limits of this registration

One wall, displacement along one axis, a planar wall with rectangular faces. Two participants. The structure is a lumped
mock integrated explicitly; no real structural solver, no transport, no case key, no CLI or MCP tool. Mapping by given
weights only: no geometric search, no radial-basis functions, no unmapped-value policy. No ramping, no extrapolated
predictor (the predictor is the flow's force at `t_n`), no quasi-Newton update, no sub-cycling test (the protocol allows a
participant to sub-cycle; the mock does not). The flow step is implicit Euler (the moving-mesh step has no other). The
added-mass factor `2M/(2M + m_a)` is that of ADR 0250's pairing. Heat transfer across the interface is not exchanged.

## Amendment 1 (2026-11-12, written while the test file was being written, before any run of any check)

**The set-up CW as registered does not realise the model problem P1: its side walls are open to a transverse flow.** The
registration copied "the four sides zero-gradient" from the piston of ADR 0250, where the flow is a plug and nothing moves
toward a side. Reading `crates/rcfd-fv/src/pressure.rs` (`pressure_bc`, `boundary_hby_a`) after the registration and before
any run: on a patch whose velocity condition is `ZeroGradient` the normal velocity is not prescribed (`fixed_u` is false),
the pressure condition stays zero-gradient, and the face flux is `HbyA . S` of the owner cell, uncorrected. For the plug
that flux is zero. For mode 1, whose fluid moves across the channel next to the wall, the sides `ymin` and `ymax` would let
it through: not the slip walls of P1, and not the closed form's `p_y = 0`. The registration should have read the flux
treatment of the side condition before reusing it for a flow that is not a plug; it did not. No run showed it.

- **CWs (replaces CW in every check: A1 to A5, S5, SN1, M1 to M3, MN1, R1, E1, N1, N2, C3, B2).** CW with the velocity
  condition `Symmetry` on `ymin` and `ymax` (zero normal velocity, zero-gradient tangential velocity; the pressure
  equation then takes a zero normal gradient and the prescribed zero flux there). `zmin` and `zmax` stay `ZeroGradient`
  as in ADR 0250 (nothing moves along z). Every limit, the model numbers of A3, the settings C3 and the mesh are unchanged:
  the model of the arithmetic already had impermeable sides.
- **The known limitation of `Symmetry` with the default averaged diagonal** (ADR 0033 addendum, `docs/dynamic-mesh.md`: the
  Dirichlet term of the normal component leaks into the tangential ones as a sink `(a_P - a_t) u_t`). Its size here: `a_P -
  a_x = nu S/(3 dn)` in a side cell, a friction rate `nu/(3 dn dy) = 1e-4/(3 x 0.025 x 0.05) = 0.027/s` in the two of eight
  cell columns next to `ymin` and `ymax`, `0.007/s` on average against `omega_0 = 6.3/s`: a force in phase with the
  velocity, `1e-3` of the inertial one, so a damping and no first-order frequency shift. (ADR 0033 met it at `nu = 0.01`,
  a hundred times larger.) The default diagonal is therefore kept (`component_diagonal = false`), and no limit is widened
  for it.
- **The registered CW** (zero-gradient `ymin`, `ymax`) stays in the file as
  `a2_registered_set_up_zero_gradient_sides_record`, ignored, with A2's limits intact, run once for the record. Expectation:
  mode 0 as CWs (the plug does not touch the sides); mode 1 with less added mass than the closed form, `omega_m/omega_1`
  above 1.03 and at most `sqrt((m + rho l_1)/m) = 1.23`.
- The one-segment set-up SPm1 (Q1, and the second half of B2) is the piston of ADR 0250, a plug: its four sides stay
  `ZeroGradient`, as registered.
- The command of "After the code" becomes `cargo test --release -p rcfd-fv --test cosim_f139 -- --test-threads=1
  --nocapture` (every registered check; the heavy tests are ignored only in a debug build), and the record separately,
  once, with `--ignored --exact`.

The other points of the set-up were gone through for the same kind of defect (a condition taken from the plug that a
two-dimensional flow breaks): the outlet (`p = 0`, zero-gradient velocity: the closed form's `p(L) = 0`; mode 1 has decayed
by `exp(-kappa_1 L) = 4e-4` there); the mesh smoothing (the sides are `Slip`, the wall points move along x only, which is
in the plane of every side); the wall itself (`set_follow_conditions` gives it no tangential velocity: a no-slip wall,
estimated in P3). Nothing else is changed.
