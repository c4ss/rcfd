# ADR 0637: F-303 registration: the sliding rotor-stator interface in the case tree, its loop and a non-axisymmetric check

Status: **REGISTERED 2026-11-16, written before any code of F-303 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library, tree or test code of F-303 was written and nothing was run; the code of the base was
read only, and the arithmetic below was done in `w49i-work/f303-scratch/arith.py` (disclosed). Results go in the addenda at
the end; nothing above the addenda is changed after a run.

Roadmap item F-303 (deps F-250), proposed by [ADR 0216](0216-f250-registration-sliding-interface-on-a-cylinder.md) (F-250)
questions Q1 (the case-tree wiring) and Q7 (a flow check that sees the rotor mesh flux). Parents: ADR 0216 (the sliding mesh
M1 to M7, the loop SL, the pair P, the checks S, U, R, T, D and the mutation MU4 that no flow check caught),
[ADR 0629](0629-f304-registration-planar-and-sector-sliding-interfaces.md) (F-304) and
[ADR 0634](0634-f704-registration-sliding-beyond-one-period-re-imaging-of-the-rotor.md) (F-704; its Q3: the periodic key with
`mesh_turned`), [ADR 0112](0112-wave44b-cylindrical-interface-preflight-sizing-case-tree.md) (the generator
`annulus_3d_cylindrical_interface` and its 70 deg pre-flight), [ADR 0266](0266-f350-registration-strong-body-flow-coupling-case-key.md)
and [ADR 0257](0257-f259-registration-tree-wiring-of-prism-and-incompressible-layering-and-moving-characteristic-faces.md) (optional tables with no hash-table or golden edit). Number 0637
given by the lead. Branch `f303-work` from 20340be6 (the F-704 tip, on F-304), worktree `w49i-work/f303`, scratch
`w49i-work/f303-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not
edited; defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no
new dependency, no unsafe code; clean-room. The roadmap files are not edited. Every choice the maintainer would normally be
asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed
as open.

Clean-room: the loop is ADR 0216's (derived there from the swept volumes and the GCL of ADR 0033); the new check rests on
the frame invariance of a flow whose walls are at rest (first principles, below). No source was read and no vendor code,
text or install was used. Reference values are the repository's own runs (the same case on the static joined mesh).

## What F-303 asks

The acceptance, binding:

> A case file runs a sliding rotor-stator interface from the CLI; an optional table or hash_omit_default keys carry it; a registered non-axisymmetric flow check sees the rotor mesh flux (ADR 0216 question 7).

The note: from ADR 0216 questions 1 and 7, recommendations taken; golden edits, if any, are listed for the maintainer.

Clauses:

- **c1** a case file with a sliding rotor-stator interface runs from the CLI (`rcfd run`), the rotor turning and the
  interface rebuilt every step;
- **c2** the keys are an optional table (or `hash_omit_default` keys): a case that does not write them is bit-identical,
  no hash table and no golden is edited;
- **c3** a registered flow check on a non-axisymmetric flow through the turning rotor that sees the rotor mesh flux: it is
  met by the method and fails when the rotor mesh flux is dropped (ADR 0216 MU4, which no F-250 flow check caught).

## What the code read shows (base 20340be6, read only)

1. **The generator** `annulus_3d_cylindrical_interface` (`sim.rs`, `cylindrical_interface_mesh`): rotor block
   `annulus_3d(nx, ny, nz, r_in, r_interface, lz, 0, ga)`, stator block `annulus_3d(nx_right, ny_right, nz_right, r_interface,
   r_out, lz, 0, gb)` (0 = the rotor's count), joined by `join_non_conformal_cylindrical(a, "outer", b, "inner", 0, z, 1e-8)`,
   cell zone `rotor` = the rotor cells, then ADR 0112's pre-flight (worst interface non-orthogonality `<= 70 deg`). Patches of
   the joined mesh: `inner` (rotor), `outer` (stator), `zmin`, `zmax` (rotor faces first, then stator faces, merged). Cell
   index of `annulus_3d`: `ir + n_r (it + n_theta k)`, node angle `2 pi it / n_theta`.
2. **`rcfd_mesh::sliding::SlidingInterface::cylindrical(rotor, "outer", stator, "inner", 0, z, 1e-8)`**; `mesh_at(0)` joins the
   rotor clone unrotated, so its mesh is the generator's join before the zone; `mesh_at(theta)` takes any finite angle.
   `SlidingMesh::mesh` is public (a zone can be added), `is_rotor_face(f)`, `report.interface_faces`.
3. **`rcfd_fv::sliding::sliding_step`**, **`rotating_wall_conditions(u, step, old_geom, patches, omega_w, dt)`** (one
   `omega_w` for every face of the listed patches) and **`PimpleSolver::step_sliding`** (refuses BDF2, a set `nu_face`,
   `body_force`, `source_force`, `sink`, `face_jump`, another patch list).
4. **The tree.** `Simulation::new` builds the mesh (`build_mesh`, then `[mesh.transform]`) and calls `from_geometry`
   (ADR 0262 D3); `[mesh.adapt]` branches off before the mesh through `sim/adapt.rs` (`present`: the data holds the table).
   The transient step's flow advance is `match &mut self.dynmesh { Some(dm) => dm.step(s, dt), None if ... => ..., None =>
   s.step(dt) }`, after which `self.geom` is the solver's when the mesh moved. The static checks of `rcfd validate` run
   `dynmesh::check`, which starts with `adapt_check`. `transient_restartable` (restart files) and `into_cosim_flow` list the
   models they exclude.
5. **Keys without a hash or golden edit** (ADR 0266 read 6, ADR 0257): an optional table (`Schema::object().optional()`)
   stays out of the data of every case that does not write it, so the settings hash, `to_toml`, `to_json_string` and the mesh
   of every existing input are unchanged and `case_hashes_w48u.rs` H1 needs no `DEFAULT_KEYS` entry. The MCP goldens
   (`crates/rcfd-mcp/tests/golden`) hold no case-schema key and no generator name (read: no occurrence of
   `annulus_3d_cylindrical_interface` or `box_3d`). No test compares the schema's JSON with a recorded text. `rules_w19c` E2
   evaluates the TOML literals of test sources that hold `[mesh]` or `generator` with the setup rules (no error-level
   diagnostic, warnings only SV101 / SV103).
6. **Boundary keys**: `boundaries.<patch>.velocity` (`fixed` default; "fixed with value 0 is a no-slip wall"),
   `velocity_value`, `velocity_expression_x/y/z`, `kind` (catalogue, `none` default).

## Design (derived; additive)

**D1. The table `[mesh.sliding]`** (optional, in the `mesh` object beside `box_3d` and `adapt`), ADR 0216 Q1 (a):

- `angular_velocity` (float, `Dimension::ANGULAR_VELOCITY`, default 0): `Omega` [rad/s] of the rotor block about `+z`
  through the origin (the generator's axis), right-handed;
- `rotor_walls` (`"rotating"` default, or `"rest"`): the no-slip walls of the rotor block turn with it (a hub) or are at
  rest in the laboratory frame.

Absent table: the generator's frozen join as before. Present: the generator must be `annulus_3d_cylindrical_interface`
(`mesh.source = "generator"`); the rotor block and its interface are those of read 1.

**D2. The mesh.** The blocks of read 1 are built by one helper shared with `cylindrical_interface_mesh` (the same calls in the
same order, extracted; the frozen join is unchanged). The sliding run's mesh at angle 0 is `SlidingInterface::mesh_at(0)` with
the zone `rotor` added and ADR 0112's pre-flight; it is the generator's mesh (L0). Every later mesh is `mesh_at(theta)` with
the zone added.

**D3. The loop** (ADR 0216 SL in the tree): before step `n + 1`, with the solver at time `t` and step `dt`, `theta = Omega (t +
dt)`; `new = mesh_at(theta)`; `st = sliding_step(&s.geom, &cur, &new, &s.phi, &s.u.values, dt)`; the wall conditions (D4);
`s.step_sliding(dt, st, dpdn)`; `cur = new`; `self.geom = s.geom`. A step whose interface non-orthogonality exceeds 70 deg
(ADR 0112's measure on the new mesh) stops the run with a located message (`mesh.sliding`). Monitors pushed after each step
(new names, only with the table): `sliding_angle`, `sliding_interface_flux` (`sum_{f in I} phi_f`), `sliding_gcl`
(`SlidingStep::gcl_residual`), `sliding_non_orthogonality` (deg).

**D4. Boundary conditions on the rotor.** A patch with at least one rotor face (owner in the rotor block) is classified once,
at construction: velocity `fixed`, `velocity_value = [0, 0, 0]`, no expression, `kind = none`: a **rotor wall**; `fixed` with a
non-zero uniform value, `symmetry`, `zero_gradient`: kept as written (laboratory-frame values; the rotor faces of `zmin`,
`zmax` move in their own plane and those of `inner` along a circle, so a uniform value stays valid); anything else on such a
patch (`moving_wall`, `shear`, `partial_slip`, `marangoni`, `freestream`, an expression, a catalogue kind): refused, located
at the patch's key. Every step, each rotor wall gets ADR 0216 M6 face by face: on a rotor face `u_b = (w a x (x_f - o))_t +
n_f m_f / |S_f|` with `w = Omega` (`rotor_walls = "rotating"`) or `0` (`"rest"`), on a stator face of the same patch `u_b = 0`
(`m_f = 0` there), and `dp/dn = -n . (u_b,new - u_b,old) / dt`. This is a new library function
`rcfd_fv::sliding::rotor_wall_conditions(u, step, old_geom, new: &SlidingMesh, patches, omega_w, dt)`, equal to
`rotating_wall_conditions` on a patch of rotor faces only (W1). Stator-only patches are not touched.

**D5. Refusals** (located diagnostics; the solver path of ADR 0216 has no other terms). Static (`rcfd validate`, through
`dynmesh::check`): another generator or a mesh file (`mesh.generator`), `solver.algorithm` other than `piso` / `pimple`,
`solver.time_scheme` other than `euler`, `dynamic_mesh.enabled`, `solver.time_advance = "fractional_step"`. At
`Simulation::new`: a rotation of `[mesh.transform]`; energy, a turbulence model, scalars, chemistry, rheology, sources, zone
sources, lumped components, radiation, conjugate heat transfer, the electric potential, phase change, particles, a mixing
plane, expression or user-function boundary values, Marangoni walls, user functions, a body force, catalogue conditions,
adaptive step control (`solver.error_tolerance > 0`), `solver.corrector_control = "adaptive"`; the D4 refusals. Restart files
are refused (`transient_restartable` false with the table) and a co-simulation participant with the table is refused
(`into_cosim_flow`).

**D6. What a case without the table runs.** `Simulation::new` reaches the sliding module only through `present` (the data
holds `/mesh/sliding`); the new struct field is `None`; the new match arm of the step is guarded by it; the extracted helper
makes the same calls; `transient_restartable` and `into_cosim_flow` gain a term that is false. So every existing input runs the
same operations (D1 to D3 of the checks below).

## The non-axisymmetric check (c3), derived

**N1. Frame invariance.** With every wall at rest in the laboratory frame (`rotor_walls = "rest"`, the stator's walls fixed
0, the planes `zmin`, `zmax` symmetry), the flow does not depend on how the rotor mesh moves: the sliding run approximates
the same solution as the run on the static joined mesh (the case without the table). If the rotor turns by a whole number
`k` of rotor pitches, the final sliding mesh is the static mesh with the rotor cells relabelled (rotor cell `(ir, it)` sits at
the static cell `(ir, it + k)`, read 1; up to the rounding of the rotated points), so the two final states can be compared
cell for cell with no interpolation (ADR 0216's reading B, R2).

**N2. Why a uniform or axisymmetric flow is blind to the rotor mesh flux, and this one is not.** The ALE step convects with
`phi - m`. If the rotor faces' `m` is dropped (`W_f = 0`; the rotor cells off the interface keep their GCL, the motion being
rigid, and the interface closure of ADR 0216 M3 still closes the rest), the rotor's cell values are carried with the cells:
after a turn `alpha` the value held at the laboratory point `x` is the old value of `Rot(-alpha) x`, its Cartesian
components unturned. A uniform field is unchanged by that. An axisymmetric swirl `f(r) e_theta` is changed in its Cartesian
components, but the change is a rotation of each vector, whose divergent radial part the pressure projection removes and
whose swirl part is again `f(r) e_theta` to first order; ADR 0216's MU4 diagnostic found T and D moved by `2e-5` and `4e-4`
only. A flow whose pattern has an azimuthal wave number `m >= 1` is displaced by `alpha` in the rotor against the stator and
against the static solution, which no projection undoes.

**N3. The field.** The plane strain `u = G (x, -y, 0)` (`m = 2` in the pattern; divergence-free; Cartesian components `G r cos
theta`, `-G r sin theta`), `G = 0.5`, `U0 = G r_out = 1` (the velocity scale). It is not tangent to the walls (`u_r = G r cos
2 theta`); the first pressure projection removes the normal part, and the field then decays from the walls inward with
`nu = 0.01`. With the rotor mesh flux dropped, a rotor cell at `x` holds after a turn `alpha` the velocity of `Rot(-alpha)
x`: `u_x` changes by about `G y alpha`, so the change is about `G r alpha` (estimate below).

**N4. Its estimate.** `alpha = 6` rotor pitches `= 0.2618` rad: `G r alpha = 0.164 U0` at `r = 1.25` (mid-rotor) and `0.196
U0` at `r = 1.5` (the interface). The slowest viscous decay over the run, `exp(-nu (pi / (r_out - r_in))^2 alpha) = 0.974`
(arith.py), leaves it of that size. The sliding run against the static run: ADR 0216 R2 measured `2.36e-4 Omega r_o` for the
rigid rotation, whose Cartesian components have the same `m = 1` variation along the interface as this field's, with an
interface speed `1.5 = 0.75 Omega r_o`; scaled to this field's interface speed `G R = 0.75 U0` that is about `2.4e-4 U0`; the
transient start (an impulsively blocked wall flow, boundary layers `sqrt(nu t) = 0.05` below one radial cell `1/16`) is the
same in both runs. The limit `1e-2 U0` leaves a factor of about 40 over that estimate and a factor of 16 below the control's
estimate.

## Arithmetic before this registration (disclosed, `f303-scratch/arith.py`)

- Rotor pitch `2 pi / 144 = 0.0436332`, stator pitch `2 pi / 120 = 0.0523599`; `dt = pitch_rotor / 8 = 0.00545415391248228`;
  48 steps turn `0.261799388` rad = 6 rotor pitches = 5 stator pitches.
- A rotor node meets a stator node at angles `m 2 pi / 720` (ADR 0216 amendment 2 (c)); step ends `j 2 pi / 1152`; equal when
  `5 j = 8 m`, i.e. only at whole rotor pitches (steps 8, 16, ..., 48), as at angle 0, where the join drops the zero-area
  overlaps (ADR 0216 R2 record).
- Courant number about `U0 dt / (r_in pitch) = 0.125`; mesh Courant number `Omega dt / pitch = 0.125`.

## Set-ups

The pair **TP** in the tree: `[mesh] generator = "annulus_3d_cylindrical_interface"`, `nx = 144`, `ny = 8`, `nz = 1`,
`nx_right = 120`, `ny_right = 8`, `nz_right = 1`, `r_in = 1`, `r_interface = 1.5`, `r_out = 2`, `lz = 0.1` (ADR 0216's P as a
3-D block one cell deep: 2112 cells, interface non-orthogonality about 49 deg, ADR 0216 X0). Solver **TS**: `algorithm =
"piso"`, `convection = "central"`, `non_orthogonal_correctors = 6` (ADR 0216 amendment 2), `time_scheme = "euler"` (default),
`momentum_rel_tol = 1e-13`, `pressure_rel_tol = 1e-13`, `pressure_abs_tol = 1e-16`, `momentum_max_iter = 5000`,
`pressure_max_iter = 5000`, `dt = 0.00545415391248228`, `end_time = 1` (the runs are driven by a step count). `[physics] nu
= 0.01`. Boundaries: `zmin`, `zmax` `velocity = "symmetry"`; `inner`, `outer` default (fixed 0, walls).

- **NA** (non-axisymmetric, c3): TP, TS, `[mesh.sliding] angular_velocity = 1, rotor_walls = "rest"`; `[initial]
  velocity_expression_x = "0.5 [1/s] * x"`, `velocity_expression_y = "-0.5 [1/s] * y"`; 48 steps.
- **NA0** (its reference): NA without the table (the frozen join, the existing path); 48 steps.
- **NAL** (the library loop): the solver of a Simulation of NA (`pimple_solver_mut`, never stepped by the tree), advanced 48
  steps by ADR 0216's loop SL written in the test with the library calls (`mesh_at`, the zone, `sliding_step`,
  `rotating_wall_conditions(.., ["inner"], 0, dt)`, `step_sliding`), angle `Omega (t + dt)`.
- **NAN** (negative control): NAL with, after `sliding_step`, the mesh flux of every rotor face that is not an interface face
  set to zero and the interface closure solved again (`interface_balance` on the step's volumes, as `sliding_step`): ADR
  0216 MU4 as a control.
- **UT** (uniform flow, tree): TP, TS, `[mesh.sliding] angular_velocity = 1` (`rotor_walls` default); `inner` and `outer`
  `velocity = "fixed"`, `velocity_value = [1, 0.5, 0]`; `[initial] velocity = [1, 0.5, 0]`; `nu = 0.1`; 40 steps (0.218 rad,
  4.2 stator pitches).
- **WT** (rotating rotor wall, tree): NA with `rotor_walls = "rotating"`, `[initial] velocity = [0, 0, 0]`; 20 steps; and its
  library loop **WL** as NAL with `rotating_wall_conditions(.., ["inner"], 1, dt)`.
- **CL** (the CLI): NA written to a temporary case file with `end_time = 48 dt` (`0.2617993877991494`) and two reports
  (`volume_average` of `ux` and of `uy`), run by `rcfd run <case> --out <dir> --quiet`; the same text in process.

## Registered checks

All in release. "Every step" is the largest value over the steps. `U0 = 1` in NA, `|U|` in UT. Tests: the tree checks in
`crates/rcfd-api/tests/sliding_case_f303.rs`, the CLI checks in `crates/rcfd-cli/tests/sliding_case_f303.rs`, the unit checks
of D4 in `crates/rcfd-fv/src/sliding.rs`.

### C. The case file from the CLI (c1)

| Id | Check | Limit |
| --- | --- | --- |
| C1 (c1) | CL: `rcfd run` exits 0; its `monitors.csv` last `sliding_angle` equals `Omega end_time`; its last `ux_mean`, `uy_mean` report values against the in-process run | exit 0; `abs(angle - 0.2617993877991494) <= 1e-12`; relative `<= 1e-12` (as ADR 0257 X3) |
| C2 | CL: `rcfd validate` of the case with `generator = "annulus_3d"` (other keys kept) | exit 3, the message naming `mesh.sliding` |
| C3 (set-up) | NA: the interface owner-neighbour pairs differ between the first and the last mesh of the run in at least one step, and the final `sliding_angle` is at least one stator pitch | as stated |

### K. The keys (c2)

| Id | Check | Limit |
| --- | --- | --- |
| K0 | NA0's case (no table): the data has no `/mesh/sliding`, `to_toml` has no `sliding`; NA's case: `mesh.sliding.rotor_walls` reads `rest`, the defaults read `angular_velocity = 0`, `rotor_walls = "rotating"` when the table is written empty; an unknown key in the table is a located parse error | as stated |
| L0 | the sliding Simulation's mesh at angle 0 against the frozen generator's mesh (NA0) | `content_hash` equal |
| D1 | `cargo test -p rcfd-api --release --test baseline_hashes` (shared files `sim.rs`, `dynmesh.rs`, `restart.rs`) | pass (unedited) |
| D2 | unedited suites: `rcfd-api` `case_hashes_w48u` (H1), `cylindrical_interface_w44b` (T1 to T4, the generator's mesh and runs), `cylindrical_interface_w43b`, `dynamic_mesh`, `rules_w19c` (E2 with the new literals); `rcfd-fv` `sliding_interface_f250`, `sliding_interface_f250_records` (R1 to R3), the `rcfd-fv` library tests | pass |
| D3 | NA0 run twice in process: equal bit for bit (the frozen path is deterministic, so the NA0 used by NA1 is the existing path's result) | equal |

### L. The tree loop is the library loop

| Id | Check | Limit |
| --- | --- | --- |
| L1 | NA against NAL after 48 steps: `max_c abs(u_tree - u_lib) / U0`, `max_f abs(phi_tree - phi_lib) / (U0 max abs(S_f))` | `<= 1e-12` each |
| L2 | WT against WL after 20 steps, as L1 | `<= 1e-12` each |

### N. The non-axisymmetric flow through the turning rotor (c3)

| Id | Check | Limit |
| --- | --- | --- |
| N0 (set-up) | NA: every rotor cell centre `(ir, it)` of the final mesh within `1e-9` of the NA0 centre of `(ir, it + 6 mod 144)`, every stator cell centre within `1e-9` of NA0's same cell | as stated |
| NA1 (c3) | `E = max_c abs(u_NA(c) - u_NA0(map c)) / U0` after 48 steps, over all cells (the map of N0) | `<= 1e-2` (N4) |
| NA2 (c3, negative control) | NAN's `E` | `>= 5e-2` (N4: `0.16` to `0.20`) |
| NA3 | `E_NAN >= 5 E_NA` (the check separates the two) | as stated |
| M1 (mass) | NA, every step `abs(sliding_interface_flux) / (U0 A_I)`, `sliding_gcl`, `continuity` | `<= 1e-10`, `<= 1e-12`, `<= 1e-9` (ADR 0216 T5 of TC6) |
| X0 (set-up) | NA, UT, WT: every step `sliding_non_orthogonality` | `<= 70 deg` |

Printed, no limit: `E` over the stator cells alone and over the rotor cells alone, for NA and NAN; the rms of the difference;
NA0's and NA's largest `abs(u) / U0` at the end; the same `E` of a uniform-flow comparison of NAN on UT (expected blind, N2).

### U. Uniform flow through the tree

| Id | Check | Limit |
| --- | --- | --- |
| U1 | UT, after every step `max_c abs(u_c - U) / abs(U)` and `max_c abs(p_c - pbar) / abs(U)^2` (`pbar` the volume mean) | `<= 1e-10` each (ADR 0216 U3) |
| U2 | UT, every step `sliding_gcl` and `continuity` | `<= 1e-12`, `<= 1e-9` |

### W. The rotor wall conditions (unit, `rcfd-fv`)

| Id | Check | Limit |
| --- | --- | --- |
| W1 | ADR 0216's pair P3 (3-D, `annulus_3d` blocks, patches `inner`, `zmin`, `zmax`, `outer`) at 0.0123 to 0.0173 rad: `rotor_wall_conditions` on `["inner"]` against `rotating_wall_conditions` on `["inner"]`, `omega_w = 1`: the face values and `dpdn` | bit for bit |
| W2 | the same step on `["zmin"]` (rotor and stator faces): every stator face value is `[0, 0, 0]` and its `dpdn` 0; every rotor face value equals `rotating_wall_conditions`'s on that face | exact |

### E. Refusals

| Id | Check | Limit |
| --- | --- | --- |
| E1 | `Simulation::new` of NA with, one at a time: `algorithm = "simplec"`; `time_scheme = "bdf2"`; `turbulence.model = "k_epsilon"`; `[dynamic_mesh] enabled = true`; `[boundaries.inner] velocity = "moving_wall"`; an expression on `outer`; `generator = "annulus_3d"`; `[mesh.transform] rotate_z_deg = 10` | `Err` each, the message naming the key or `mesh.sliding` |
| E2 | NA: `supports_restart()` | false |

### Mutations (scratch copy, not committed)

Each applied alone to the final tree by an exact text replacement on a `git archive` copy, the named tests run once; each
must fail.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `rcfd_mesh::sliding::SlidingMesh::rotor_swept_volumes` returns zeros (ADR 0216 MU4) | NA1 |
| MU2 | the tree's wall conditions use `Omega` whatever `rotor_walls` says | NA1 |
| MU3 | the tree loop does not keep the new mesh (`cur` not replaced) | L1 or C1 (a refused or failed step counts) |
| MU4 | `rotor_wall_conditions` treats every face as a rotor face | W2 |
| MU5 | `sliding_step`'s interface closure skipped (interface mesh flux left 0; ADR 0216 MU1) | U1 |

### Decision rule

**Done**: C1 (c1), K0, L0, D1, D2, D3 (c2), NA1 with NA2 and NA3 (c3), L1, M1, U1 met; the set-up checks C3, N0, X0 and
the negative control NA2 held; every mutation caught; no golden or hash table edited. A miss of an acceptance check, or a
control or mutation that does not show its check can fail: **partial** with the miss named. The tree cannot run a sliding
case: **todo**. Every other check is supporting and reported.

## Limits (not covered)

The full annulus of `annulus_3d_cylindrical_interface` only, axis `z` through the origin; no sector or planar sliding key
(the periodic interfaces of F-304 and F-704 need a sector rotor-stator generator; Q3); one rotor; a constant angular
velocity; no turbulence, energy, scalars, sources, body force, BDF2 or the other refusals of D5; expression boundary values
refused everywhere (a rotor patch's profile would rotate with its faces; a stator patch's is not refused for a reason of the
method but for the size of this item); no restart; the Studio and the MCP tools see the table only as schema data (no
dedicated UI); the compressible solver; parallel runs. The first-order interface interpolation of ADR 0109 is unchanged.

## Open questions taken on the standing instruction

- **Q1. Where the keys go.** (a) The optional table `[mesh.sliding]` (taken: ADR 0216 Q1 (a), beside `[mesh.box_3d]` and
  `[mesh.adapt]`; the interface is a property of the generated mesh, and the table needs no hash-table or golden edit, read 5);
  (b) a new generator name (`annulus_3d_sliding_interface`; changes the generator enumeration and its documentation, and
  duplicates the generator's keys); (c) keys in `[dynamic_mesh]` (its checks and step belong to a mesh of one topology).
  Taken (a) on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
- **Q2. The default of `rotor_walls`.** (a) `"rotating"` (taken: a hub turns with its rotor, the fan case); (b) `"rest"`.
  Taken (a) on the standing instruction; the checks write the key.
- **Q3. Periodic sliding in the tree** (ADR 0634 Q3 (a): F-303 uses `mesh_turned` directly). F-303's acceptance names the
  rotor-stator interface; the periodic join needs a sector rotor-stator generator that the tree does not have. (a) A follow-up
  item (a sector rotor-stator generator and the `mesh_turned` loop under the same table); (b) in F-303. Taken (a) on the
  standing instruction; proposed in the roadmap entries.
- **Q4. The non-axisymmetric check's form.** (a) Frame invariance against the static joined mesh (taken: a closed comparison
  of the repository's own runs, cell for cell after whole pitches); (b) ADR 0216 Q7's radial source flow against the
  potential flow (axisymmetric in its pattern; caught MU4 in a diagnostic, `0.22`, but needs a per-face inlet on the rotor's
  `inner`, which D4 does not allow). Taken (a) on the standing instruction.
