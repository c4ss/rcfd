# ADR 0213: F-257 registration: prism layering and layering in the incompressible PIMPLE path

Status: **REGISTERED 2026-11-07**, written before any code of F-257 and before any run of its checks. Not yet accepted by
the maintainer. Before this text one thing was run on main c398654, with the code of main as it is: the record tests L1
and L2 (the new file `crates/rcfd-fv/tests/layering_f257_records.rs`, no source change), twice, which made the records
below. Results go in the addendum at the end.

Parents: [ADR 0090](0090-wave38c-dynamic-mesh-layering.md) (F-230: the planar layer stack `LayerStack`, its thresholds and
conservative remap D3, the compressible ALE step, the piston set-up P and its limits, the "not delivered" list that F-257
comes from), [ADR 0033](0033-wave20b-dynamic-mesh.md) (F-063: the incompressible ALE step D2, the moving-wall conditions
D3, the piston channel G5 and its corrected form G5b), [ADR 0191](0191-f259-registration-case-tree-and-cli-wiring-of-layering-and-compressible-ale.md)
(F-259: the case tree runs ADR 0090's stack; its refusals E6 and E7 and its W7 name F-257),
[ADR 0207](0207-f258-registration-compressible-ale-boundaries-and-mesh-speed-time-step.md) (F-258: the sibling item of
the same split, its records R1 to R4), [ADR 0137](0137-linux-gate-platform-values.md) (platform values). Roadmap item
F-257 (depends on F-230 and F-063).

Number: the lead gave 0213 to F-257. Main c398654 holds 0209 as its highest; 0210 is on the branch
`f294-transient-keps-sa`, 0211 on `f244-six-dof-validation` and 0212 on `f296-pair-diffusivity`. No branch holds a 0213.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md), ADR 0191, ADR 0207):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded, and the
  cause is diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as
  an ignored record;
- every new code path is opt-in: a run that does not call the new types and functions runs as before, bit for bit (L1, L2
  and the existing records);
- no case-schema key is added (S3), so neither `crates/rcfd-api/tests/baseline_hashes.rs` nor the record R0
  (`case_hashes_w48u.rs`) is touched, no golden is re-recorded, and no existing test is edited;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no commit trailers.

Clean-room: the layer stack, the remap and the face-flux map below are derived from the definitions of ADR 0033 and ADR
0090 (cell totals, swept volumes, the discrete continuity of the face flux). No source was read for this work, and no
vendor code, text or install was used.

## What F-257 asks

The roadmap entry: dynamic mesh, prism layering and incompressible layering. Every limit not stated there is registered
here, before the run. Layering extensions:

- (B1) the same piston with prism (extruded triangle) layers meets the ADR 0090 limits (`p V^gamma` 2e-2, per-event
  conservation 1e-13, mass drift 1e-12);
- (B2) layering in the incompressible PIMPLE path: the ADR 0033 G5 piston channel run long enough for at least 5 layer
  removals keeps per-step continuity `<= 1e-9`, outlet flux equal to the piston swept flux to 1e-10 per step and volume
  conserved per layer event to 1e-13.

Out of scope there: sliding interfaces (F-250), elastic-analogy smoothing (F-063), six-DOF coupling (F-244), more than one
moving layered boundary, non-planar layer surfaces, turbulence or energy in the incompressible moving-mesh path. The note:
split from F-247 on 2026-10-22 (user decision), size M.

**Reading of the acceptance (registered here).**

1. "The same piston" is ADR 0090's set-up P (40 layers of `h0 = 0.025`, the cross-section `0.1 x 0.1`, slip walls, the
   gas, the solver settings, the motion with `T = 16`, the schedule) with the cross-section's 2 x 2 squares cut into 8
   triangles, so every cell is a prism (P-Δ below). "The ADR 0090 limits" are its A1, A2 and B1 to B5 (the three named
   in the acceptance, plus the volume B3, the event counts B4 and the event order B5 of the same table).
2. "The ADR 0033 G5 piston channel" is G5's channel, motion, fluid and solver set-up with G5b's corrected sides
   (zero-gradient velocity; G5 as first registered had `Symmetry` sides, ADR 0033 addendum miss 3), on a layered mesh
   instead of a smoothed one, mirrored in `x` (G5L below): the layer stack moves its last plane, so the piston is `xmax`
   moving towards `-x` and the outlet is `xmin`. Reflection in `x` maps G5 onto G5L; only the orientation changes.
3. "Run long enough for at least 5 layer removals": to `t = 0.6` (120 steps), where 7 removals are expected (G5L).
4. "Per-step continuity" is the `continuity` of the step's `StepInfo` (`max_c |sum_f phi_f| / max_f |S_f|`), as in G5
   (i); "outlet flux equal to the piston swept flux to 1e-10 per step" is G5 (ii); "volume conserved per layer event to
   1e-13" is the relative change of `sum V` over the event.
5. The acceptance is library work. The case tree is not in it: its wiring (lifting ADR 0191's refusal E6, a cross-section
   for `box_3d`) is the open part of F-259 (ADR 0191 W7 and its decision record Q3) and is not built here (S3, Q1).

## What the code read shows (main c398654, read only)

1. **`rcfd_mesh::layering::LayerStack`** (`crates/rcfd-mesh/src/layering.rs`) is a box stack only: `mesh()` builds
   `box_mesh([n, ny, nz], [n, ly, lz])` and maps plane `i` to `xs[i]` with `warp`. Cell `(i, j, k)` is `i + n (j + ny k)`.
   `adjust()` applies ADR 0090 D3 (removal below `a_min h0` when `n > 2`, addition above `a_max h0` with the new plane
   at `xs[n-1] + h0`) and returns the cell source map `LayerChange`; `remap` carries cell densities by totals. It knows
   nothing of faces.
2. **The incompressible ALE step** (`crates/rcfd-fv/src/pimple/moving.rs`): `step_ale_with_walls(dt, new_geom,
   mesh_flux, wall_dpdn)` refuses a new geometry whose cell or face count differs from the current one and any time scheme
   but implicit Euler. The step uses `hist_u[k].old` (the old level of the transient term, `ddt::add_ddt`), `self.phi`
   (the convecting flux and the Rhie-Chow defect) and `self.u.values`; `phi_hist` and the older levels are read only off
   the moving path or by BDF2.
3. **`PimpleSolver`** (`crates/rcfd-fv/src/pimple.rs`) sizes `u`, `p`, `phi`, `hist_u`, `phi_hist` and the optional
   `nu_face`, `body_force`, `source_force`, `sink` and `face_jump` to the mesh. Nothing replaces its geometry with another
   topology; `restore` and `reinitialise` keep the geometry. Its boundary conditions live in the fields, one per patch
   (`field.rs`); a condition with per-face data (`bc::Profile::PerFace`, installed by `ale::set_follow_conditions` on the
   following patches) indexes the faces of its patch locally.
4. **`ale::set_follow_conditions`** evaluates the current condition of the following patch on the new geometry (`before =
   u.boundary_values(&step.geom)`) for the wall acceleration, so it needs the same patch face count as the last step.
5. **`Mesh::from_cell_faces`** builds any polyhedral mesh from cells given as point loops: it orients the faces, orders
   the internal faces by `(owner, neighbour)` with the owner the lower cell index, puts the boundary faces of each
   `BoundarySpec` in the given order and leftover boundary faces in a trailing `defaultFaces` patch. It is deterministic.
6. **`CompressibleSolver::replace_mesh(geom, u)`** installs any topology with the same patch list (ADR 0090); the
   compressible solver needs no face data across an event.
7. **The case tree** refuses `dynamic_mesh.deformation = "layering"` with `piso` / `pimple` (ADR 0191 E6) and with a
   generator other than `box_3d` (E7); `box_3d`'s stack is `LayerStack`.

## Run before this registration (disclosed)

The test file `crates/rcfd-fv/tests/layering_f257_records.rs` was written with no source change and run twice on main
c398654 (Windows, release). Both runs printed the same values:

| Record | Run | Hash |
| --- | --- | --- |
| L1 | ADR 0033 G5b as a library loop: `box_mesh([20, 2, 2], [1, 0.2, 0.2])`, the `xmin` piston at `a0 t^2 / 2` (`a0 = 2`), Laplacian smoothing, `xmax` at `p = 0`, zero-gradient sides, PISO, central convection, tight solvers, `dt = 0.005`, 100 steps; the bits of `u`, `p`, `phi`, the time and the step count | `affff0bb34a248e2` |
| L2 | `LayerStack::uniform(6, 1, [2, 1], [0.1, 0.1], 0.4, 1.6)`, the piston moved from 1 to 0.3 and back to 1.3 in steps of 0.01 with one `adjust` after each move (4 removals, 6 additions); every event's kind, planes and cell sources, the final mesh's content hash and points | `52af54712423ce0a` |

No other run was made.

## The model (derived)

**M1. The extruded stack.** A cross-section is a planar mesh in `(y, z)`: points `q_j`, cells `c` (polygons) and boundary
edges named by patch. The stack has planes `x = xs[k]`, `k = 0..n`; the 3D point `j + np k` sits at `(xs[k], q_j)` (`np`
the number of section points); cell `i + n c` is the prism over section cell `c` between planes `i` and `i + 1` (a
hexahedron over a quadrilateral, a triangular prism over a triangle). Its faces are the plane faces (section cell `c` on
plane `k`) and the side faces (section edge `e` in layer `i`, a planar quadrilateral). Patches: `xmin` (plane 0), `xmax`
(plane `n`, the piston), then the section's patches. Only the piston plane moves; ADR 0090 D3 decides the events, with the
same thresholds, the same new plane and the same cell source map `LayerChange` (cell `i + n c`, as `LayerStack`'s
`i + n (j + ny k)` with `c = j + ny k`).

**M2. Face identity.** A face is identified by its points: all on plane `k`, it is the plane face `(k, c)` with `c` the
section cell of its owner; on planes `k` and `k + 1`, it is the side face `(k, e)` with `e` its section edge. The owner of
a plane face is the lower layer, and the owner of an internal side face is the section cell of lower index (cell index
`i + n c` and `from_cell_faces`' ordering, read 5), so a face keeps its orientation through an event.

**M3. The face flux across an event.** With `n` the old and `n'` the new number of layers:

- removal (`n' = n - 1`: the old plane `n - 1` is deleted, the old layers `n - 2` and `n - 1` merge into the new layer
  `n' - 1`): a new plane face `(k, c)` takes the old `(k, c)` for `k < n'` and the old `(n, c)` for `k = n'`; a new side
  face `(i, e)` takes the old `(i, e)` for `i < n' - 1`, and for `i = n' - 1` the sum of the old `(n - 2, e)` and
  `(n - 1, e)`;
- addition (`n' = n + 1`: a plane is inserted at index `n`, `h0` above the old plane `n - 1`, and the old layer `n - 1`
  splits into the new layers `n - 1` of height `h0` and `n` of height `h - h0`): a new plane face `(k, c)` takes the old
  `(k, c)` for `k < n` and the old `(n, c)` for `k = n + 1`; a new side face `(i, e)` takes the old `(i, e)` for
  `i < n - 1`, `h0/h` of the old `(n - 1, e)` for `i = n - 1` and `(h - h0)/h` of it for `i = n` (the cell fractions of
  `LayerChange`); the faces of the new plane `n` close the balance of their owner (the new layer `n - 1` cell):
  `phi_f = -sum_{g != f} sigma_g phi_g` over the owner's other faces (`sigma_g = +1` when the cell owns `g`).

So the net outward flux of a merged cell is the sum of those of its two sources (the deleted plane's faces are internal to
it), the new lower child of a split has net flux zero and the upper child the parent's, and every boundary face keeps
its value. A flux that satisfies the discrete continuity before an event satisfies it after, to round-off. A side face of
a layer between two `x` planes has an area vector proportional to the layer height, so the fractions reproduce the flux of
a uniform velocity on the new faces.

**M4. Cell fields of the incompressible solver across an event.** Velocity: ADR 0090's `remap` (cell totals `V u`, so the
momentum is conserved). Kinematic pressure: the same volume-weighted remap (a merged cell gets the volume mean of its
sources, exact for a linear field at the merged centroid; a split cell's children get the parent value). The old time
level of the next step is the remapped state (the event happens between two steps at a fixed time).

## Settings and code

**S1. Library (`rcfd-mesh`, module `layering`, additive).**

- `Section` (points `[f64; 2]` in `(y, z)`, cells as point loops, boundary edges per named patch) with
  `Section::rect(cross: [usize; 2], size: [f64; 2])` (`ny x nz` rectangles, cell `j + ny k`, patches `ymin`, `ymax`,
  `zmin`, `zmax`) and `Section::rect_triangles(cross, size)` (each rectangle `q = j + ny k` cut into the two triangles
  `2q` and `2q + 1` by the diagonal from `(j, k)` to `(j + 1, k + 1)` when `j + k` is even and from `(j + 1, k)` to
  `(j, k + 1)` otherwise; the same patches).
- `ExtrudedStack { xs, h0, a_min, a_max, section }` with `uniform(section, n, length, a_min, a_max)` (the requirements of
  `LayerStack::uniform`), `n_layers`, `piston_height`, `set_piston`, `adjust` (ADR 0090 D3), `points()` and `mesh()`
  (M1; built with `Mesh::from_cell_faces`, all patches `Wall`; a boundary edge without a patch is an error).
- `FaceSource` (`Sum(Vec<(usize, f64)>)`: the new face value is the weighted sum of old face values; `Closure`) and
  `ExtrudedStack::face_sources(&self, change, old, new)` (M3; called after `adjust` with the meshes before and after the
  event), `remap_face_flux(sources, phi_old, new)` (the sums first, then the closures).
- `LayerStack::adjust`'s code moves unchanged into one private function that both stacks call, so the events of the two
  stacks are the same operations; `LayerStack` and `remap` are otherwise not edited (L2, and ADR 0207's R1).

**S2. Library (`rcfd-fv`, `pimple::moving`, additive).** `PimpleSolver::replace_mesh(geom, u, p, phi) -> Result<(),
String>`: installs a geometry of another topology with one velocity and pressure per cell and one flux per face; the old
time level becomes the given state (`hist_u` and `phi_hist` restart from it, as in `new`); the time, the step count, the
history, the settings and the boundary conditions are kept. Refused, the solver unchanged: a time scheme other than
implicit Euler, a patch list that differs in number, names or kinds, sizes that do not fit, and a set `nu_face`,
`body_force`, `source_force`, `sink` or `face_jump` (their data are sized to the old mesh). A condition with per-face data
must be on a patch whose face count does not change; the stacks keep `xmin` and `xmax`.

**S3. Case tree and CLI.** Not changed. ADR 0191's refusals E6 and E7 stay; no key is added. The wiring is Q1. The
documents `docs/dynamic-mesh.md` and `docs/case-schema.md` say what the library does and that the tree does not use it yet
(prose only; no TOML block is edited).

## Registered checks

All tests run in release. The new checks are in `crates/rcfd-fv/tests/layering_f257.rs` and the unit tests of
`crates/rcfd-mesh/src/layering.rs`; the records in `crates/rcfd-fv/tests/layering_f257_records.rs`. "Relative" is
`abs(a - b) / abs(b)`.

### P. Prism layers on ADR 0090's piston (acceptance B1)

Set-up P-Δ: ADR 0090's P with the stack `ExtrudedStack::uniform(Section::rect_triangles([2, 2], [0.1, 0.1]), 40, 1.0,
0.4, 1.6)` (320 prisms, 8 per layer): every patch a slip wall (the solver's default for `Wall`), `Gas::AIR`, the state
`rho0 = 1`, `u = 0`, `p0 = 1`, `CompSettings::default()` (HLLC, van Leer, SSP-RK3, CFL 0.4), the piston
`x_p(t) = 1 - 0.3 (1 - cos(pi t / T))`, `T = 16`, to `2T`. ADR 0090's loop: `dt = max_dt()` of the current mesh, landed on
`T` and `2T` (`t + dt >= target - 1e-9`), the stack's piston at the step's end, `step_ale` with the geometry of
`mesh()`, then `adjust`; on an event the geometry of the new mesh, `remap` and `replace_mesh`.

| Id | Check | Limit |
| --- | --- | --- |
| P1 (acceptance) | `p_bar V^gamma / (p0 V0^gamma) - 1` at `t = T` (`p_bar = sum p_c V_c / sum V_c`) | `abs <= 2e-2` (ADR 0090 A1) |
| P2 (acceptance) | the same at `t = 2T` | `abs <= 2e-2` (A2) |
| P3 (acceptance) | every event: `sum V`, `sum rho V`, `sum rho E V` after against before | relative `<= 1e-13` (B1) |
| P4 (acceptance) | every step: the total mass against the initial | relative `<= 1e-12` (B2) |
| P5 (acceptance) | every step: `sum V` against `0.01 x_p(t)` | relative `<= 1e-13` (B3) |
| P6 (acceptance) | removals and additions | each `>= 5` (B4; 24 each expected) |
| P7 (acceptance) | a removal in the step after an addition, or the reverse | none (B5) |
| P0 (set-up) | after every step: 8 cells per layer, each with 5 faces (2 triangles, 3 quadrilaterals) | as stated |

Printed, no limit: the largest `abs(p_bar V^gamma / (p0 V0^gamma) - 1)` over the steps, the cell spread of `p / rho^gamma`
at `T`, the steps, the wall time, and `p_bar` at `T` against ADR 0090's hexahedral run of the same loop.

### I. Layering in the incompressible PIMPLE path (acceptance B2)

Set-up G5L: ADR 0033 G5b mirrored in `x` (reading 2): the stack `ExtrudedStack::uniform(Section::rect([2, 2], [0.2,
0.2]), 20, 1.0, 0.4, 1.6)` (80 hexahedra, `h0 = 0.05`; G5's `box_mesh([20, 2, 2], [1, 0.2, 0.2])` with another face
order). The piston `xmax` at `x_p(t) = 1 - a0 t^2 / 2`, `a0 = 2`, follows the mesh; the outlet `xmin` has `p = 0` (the
velocity's default zero gradient); `ymin`, `ymax`, `zmin`, `zmax` zero-gradient velocity (G5b); `nu = 0.01`;
`PimpleSettings::piso` with central convection and the tight solvers of G5 (`tol_abs = 1e-16`, `tol_rel = 1e-13`, 5000
iterations); `dt = 0.005`, 120 steps to `t = 0.6`. Each step: `set_piston(x_p(t + dt))`, `ale::move_mesh(&geom,
points(), dt)`, `set_follow_conditions` on `xmax`, `step_ale_with_walls`; then `adjust`, and on an event the new mesh,
`face_sources`, the remap of M4 for `u` and `p`, `remap_face_flux` for `phi`, and `replace_mesh`. Removals are expected at
the piston displacements `0.05 m - 0.02` (`m = 1..7`, steps 35, 57, 73, 85, 96, 106, 115).

The outlet flux `Q_out` is the sum of `phi` over `xmin` after the step, the piston's swept flux `m_p` the sum of the step's
mesh flux over `xmax` (G5's measure, `abs(Q_out + m_p) / abs(m_p)`).

| Id | Check | Limit |
| --- | --- | --- |
| I1 (acceptance) | every step's `continuity` | `<= 1e-9` (G5 (i)) |
| I2 (acceptance) | every step: `abs(Q_out + m_p) / abs(m_p)` | `<= 1e-10` (G5 (ii)) |
| I3 (acceptance) | every event: `sum V` after against before | relative `<= 1e-13` |
| I4 (acceptance, set-up) | removals | `>= 5` (7 expected) |
| I5 | every event: `max_c abs(sum_f phi_f) / max_f abs(S_f)` of the remapped flux on the new mesh; the `xmin` fluxes | `<= 1e-9`; unchanged bit for bit |
| I6 | every event: `sum V u` after against before (each component against `abs(sum V u)` before) | relative `<= 1e-13` |

Printed, no limit: G5 (iii)'s cell-velocity error `max abs(u_x + a0 (t_n + dt/2))` and its transverse part, and G5 (v)'s
pressure error `max abs(p - a0 x)` (the lab-frame plug flow of G5L, outlet at `x = 0`) against `2e-2 a0 L0`.

### X. Layer additions in the incompressible path (supplementary)

Set-up G5X: G5L with the piston pulled out, `x_p(t) = 1 + a0 t^2 / 2`, so the fluid enters through `xmin`. Additions are
expected at the displacements `0.05 m - 0.02` (7 to `t = 0.6`).

| Id | Check | Limit |
| --- | --- | --- |
| X1 | every step's `continuity` | `<= 1e-9` |
| X2 | every step: `abs(Q_out + m_p) / abs(m_p)` | `<= 1e-10` |
| X3 | every event: `sum V` after against before | relative `<= 1e-13` |
| X4 (set-up) | additions | `>= 5` (7 expected) |
| X5 | every event: the remapped flux's continuity on the new mesh (I5's measure) | `<= 1e-9` |

### F. The face-flux map (unit, `rcfd-mesh`)

On the stacks `ExtrudedStack::uniform(s, 5, 1.0, 0.4, 1.6)` for `s` = `Section::rect([2, 1], [0.1, 0.1])` and
`Section::rect_triangles([2, 2], [0.1, 0.1])`, one removal (piston at 0.85) and then one addition (piston at 1.2), as in
`LayerStack`'s unit test:

| Id | Check | Limit |
| --- | --- | --- |
| F1 | old face values pseudo-random in `[-1, 1]` (a fixed linear congruential sequence): after `remap_face_flux`, each new cell's net outward flux against the sum of its sources' (removal: merged and kept cells; addition: the lower child against 0, the upper child against the parent), and the sum of the values over every patch | `<= 1e-13 max abs(phi_old)` each |
| F2 | old values `u0 . S_f` with `u0 = (0.3, -0.2, 0.1)`: the remapped value against `u0 . S_f` of the new mesh, every face | `<= 1e-13 abs(u0) max abs(S_f)` |
| F1n (negative control) | F1 with a face map made from the registered one by dropping the second old face of every merged side face | the balance error of F1 `>= 1e-2 max abs(phi_old)` |
| K1 | `ExtrudedStack` with `Section::rect([2, 2], [0.1, 0.1])`, 40 layers, against `LayerStack::uniform(40, 1, [2, 2], [0.1, 0.1], 0.4, 1.6).mesh()`: patch names, kinds and sizes; cell volumes and centres by cell index | equal; relative `1e-13`, `1e-13` of the length |
| K2 | the P-Δ mesh: 320 cells with 5 faces each, `xmin` and `xmax` 8 faces each, `ymin`, `ymax`, `zmin`, `zmax` 80 each, no `defaultFaces`; the volume of every layer against `0.01 h` | as stated; relative `1e-13` |

### S. `PimpleSolver::replace_mesh`

| Id | Check | Limit |
| --- | --- | --- |
| S1 | G5L for 6 steps against G5L for 3 steps, `replace_mesh` with the current geometry and the current `u`, `p`, `phi`, then 3 steps: `u`, `p`, `phi` | bit for bit |
| S2 | refusals: `u` one cell short, the geometry of `box_2d(4, 4, 1, 1)` (another patch list), BDF2, a set `nu_face`: `Err` each, and `u` unchanged bit for bit | as stated |

### N. Negative controls

| Id | Control | Limit |
| --- | --- | --- |
| N1 | P-Δ: at every removal, the totals of a remap that gives the merged cell the density of its first source; the largest relative change of `sum rho V` over the removals | `>= 1e-10` |
| N2 | G5L: at every step from the second, `abs(Q_out + m_p(previous step)) / abs(m_p)` (the swept flux one step late) | `>= 1e-3` |

N1 shows that P3 sees a remap that does not carry the cell totals (the density next to the accelerated piston differs from
its neighbour's by the pressure gradient of the gas column). N2 shows that I2 resolves one step of the piston's
acceleration (`m_p` changes by `1 / (n - 1/2)` of itself from step `n - 1` to `n`, at least `8.4e-3` to step 120).

### D. Defaults

| Id | Check | Limit |
| --- | --- | --- |
| D1 | records L1 and L2 above, after the code | equal (Windows; printed elsewhere, ADR 0137) |
| D2 | the existing suites, unedited: `rcfd-mesh` unit tests (`layering`), `rcfd-fv` `layering_w38c`, `ale_flows`, `ale_gcl`, `compressible_ale_f258`, `compressible_ale_f258_records` (R1 to R4), `exp_rot_pab`, `mesh_smoothing`; `rcfd-api` `layering_ale_f259` (D0, K1, A-eq), `dynamic_mesh`, `inverse_volume_f290`, `baseline_hashes`, H1 of `case_hashes_w48u`; `rcfd-cli` `layering_ale_f259` | pass |

### Mutations (scratch, not committed)

Each is applied alone to the final tree, the named tests are run once, and they must fail; the file is restored and
checked byte for byte against a kept copy. Their numbers go in the addendum. `scripts/limit_audit.py` is run on the new
test files and every hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the shared event function gives a merged cell only its first source | P3 |
| MU2 | `face_sources`: a merged side face takes only the old face of the old layer `n - 1` | F1 |
| MU3 | `remap_face_flux`: a `Closure` face gets 0 | F1, X5 |
| MU4 | `face_sources`: the side-face fractions of the two children of a split swapped | F2 |

### The gate

`cargo fmt` per package, `cargo clippy --workspace --all-targets -- -D warnings`, and every test in release on a
`git archive` copy of the gated commit, one process per test, three jobs (`scripts/gate_test.py`), in the target dir
`E:/projects/cfd/w49i-work/target-f257-gate`, taken in turn through the machine's gate lock, with the free physical memory
at the start disclosed. A failure of a known flaky test is run again alone on the same build and reported with its name.

### Classes

D1 and D2 are preconditions of landing: a miss means the change is not merged and the cause is recorded. A miss of any
other limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. A set-up check
(P0, I4, X4, K1, K2) or a negative control (F1n, N1, N2) that does not hold, or a mutation that does not make its check
fail, means the check is not shown to be able to fail: it is reported as a defective set-up, corrected by an amendment
before any re-run, and never counted as a pass.

**Decision rule.** Every registered check met (P0 to P7, I1 to I6, X1 to X5, F1, F2, K1, K2, S1, S2, with D1 and D2), the
negative controls held and every mutation failed its tests: F-257 is set done (`[x]`), the acceptance still the
maintainer's (Q5). A miss of any check: partial with the miss named. Neither the prism piston nor the layered channel runs
at all: todo.

## Not delivered

The case-tree and CLI wiring (E6 and E7 stay; Q1); a piston at the first plane (Q2); a cross-section from a mesh file or
the mesher (only the two rectangular sections of S1; `Section` is public, so a caller can build others); layering in the
incompressible path with BDF2, with a face viscosity, body forces, sources or jumps (refused by `replace_mesh`); a
pressure split by the parent gradient (Q4); everything outside F-257 (the roadmap's out-of-scope list).

## Open questions and choices for the maintainer

- **Q1. The case-tree wiring.** Not in F-257's acceptance; the open part of F-259 (ADR 0191 W7). Options: (a) wire it under
  F-259's checks (located errors, a tree run equal to the library run to 1e-12, a case without the new behaviour
  bit-identical): lift E6 for `piso` / `pimple` with `box_3d`, and add a cross-section choice for `box_3d` (a key with
  `.hash_omit_default()` or an optional table, so that H1's record R0 needs no edit); (b) keep the library only.
  Recommendation (a), as a follow-up registration under F-259.
- **Q2. The piston end.** The stacks move their last plane (`xmax`); G5's piston is `xmin`, so G5L is mirrored. Options:
  (a) keep (a case puts its piston at `xmax`); (b) a stack option for the first plane. Recommendation (a) until a case
  needs (b).
- **Q3. The flux of a split.** Options: (a) side faces split in proportion to the layer heights, the new plane closing the
  lower child (M3, as registered; exact discrete continuity, no solve); (b) a local pressure projection on the two
  children. Recommendation (a).
- **Q4. The pressure at an event.** Options: (a) the volume-weighted remap (M4, as registered; the next step's correctors
  solve the pressure again); (b) children of a split take the parent value plus the parent gradient times the offset of
  their centres. Recommendation (a).
- **Q5. Status of F-257.** If every check is met, the acceptance as written is met in the library. Options: (a) done, with
  the wiring tracked by F-259; (b) partial until the tree uses it. Recommendation (a), as for F-258. The roadmap follows
  the decision rule until the maintainer decides.
