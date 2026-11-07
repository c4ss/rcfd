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

## Addendum 1: what was built and the results (2026-11-07)

Measured against the limits registered above; no limit was changed. Release builds on Windows. The registration is
8ebbba8 (with the records L1 and L2), and the code, tests and documents are 0cbaf33. Each registered check ran once, on
the code of 0cbaf33 before its commit; the runs are deterministic and none was repeated to change a result. Every check
met its limit, the set-up checks and the negative controls held, and every mutation made its check fail.

**Built as registered.** S1 to S3 as written.

- `crates/rcfd-mesh/src/layering.rs`: `Section` (`rect`, `rect_triangles`), `ExtrudedStack` (`uniform`, `n_layers`,
  `piston_height`, `set_piston`, `adjust`, `points`, `mesh`, `face_sources`), `FaceSource` and `remap_face_flux`; the
  private `apply_thresholds`, which holds the code of `LayerStack::adjust` unchanged and is called by both stacks. The face
  identity of M2 is the private `FaceTag`. `LayerStack`'s other methods and `remap` are not edited.
- `crates/rcfd-fv/src/pimple/moving.rs`: `PimpleSolver::replace_mesh` (S2). Nothing else in `rcfd-fv` is edited.
- Tests: `crates/rcfd-fv/tests/layering_f257.rs` (5 tests: P with N1, I with N2, X, S1, S2), three unit tests in
  `crates/rcfd-mesh/src/layering.rs` next to its existing one (F1 with F1n and F2 in one, K1, K2) and the records file of the registration (2).
  No existing test was edited.
- Documents: `docs/dynamic-mesh.md` and `docs/case-schema.md` (prose: what the library does, that the tree does not use
  it yet; no TOML block edited), and the F-257 entries of `docs/ROADMAP.md` and `docs/roadmap.json` (done, by the
  decision rule).
- Not changed: `rcfd-api`, `rcfd-cli`, the case schema.

### Results

`layering_f257.rs`, 5 tests, 19.5 s on one thread; the unit tests 0.01 s.

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| P0 (set-up) 8 cells per layer, 5 faces each, after every step and event | as stated | held | held |
| P1 (acceptance) at `t = T` | `abs <= 2e-2` | `1.530e-5` (`V/V0 = 0.400000`, 16 layers, `p_bar = 3.606805` against the isentrope's `3.606750`) | met |
| P2 (acceptance) at `t = 2T` | `abs <= 2e-2` | `3.416e-5` (40 layers) | met |
| P3 (acceptance) per-event conservation | `<= 1e-13` | `6.66e-16` | met |
| P4 (acceptance) mass drift | `<= 1e-12` | `6.95e-13` | met |
| P5 (acceptance) volume against `0.01 x_p` | `<= 1e-13` | `1.11e-15` | met |
| P6 (acceptance) removals, additions | each `>= 5` | 24, 24 | met |
| P7 (acceptance) opposite events in consecutive steps | none | 0 | met |
| N1 (negative control) first-source remap, largest mass change over the removals | `>= 1e-10` | `3.72e-6` | held |
| I1 (acceptance) step continuity | `<= 1e-9` | `3.12e-15` | met |
| I2 (acceptance) `abs(Q_out + m_p) / abs(m_p)` | `<= 1e-10` | `1.30e-14` | met |
| I3 (acceptance) volume per event | `<= 1e-13` | `2.22e-16` | met |
| I4 (acceptance, set-up) removals | `>= 5` | 7 (no addition; 13 layers at the end) | met |
| I5 continuity of the remapped flux; `xmin` fluxes | `<= 1e-9`; bit for bit | `2.43e-15`; kept | met |
| I6 momentum per event | `<= 1e-13` | `2.29e-16` | met |
| N2 (negative control) the swept flux one step late | `>= 1e-3` | `8.37e-3` (at step 120, `2/239`) | held |
| X1 step continuity | `<= 1e-9` | `3.30e-15` | met |
| X2 `abs(Q_out + m_p) / abs(m_p)` | `<= 1e-10` | `1.49e-14` | met |
| X3 volume per event | `<= 1e-13` | `0` | met |
| X4 (set-up) additions | `>= 5` | 7 (no removal; 27 layers at the end) | held |
| X5 continuity of the remapped flux | `<= 1e-9` | `2.95e-15` | met |
| F1 balance, removal and addition, rectangles and triangles | `<= 1e-13` | `9.09e-16`, `2.28e-16`; `6.70e-16`, `4.47e-16` | met |
| F2 uniform-flow flux, the same four | `<= 1e-13` | `0`, `1.16e-16`; `0`, `1.64e-16` | met |
| F1n (negative control) the second old face dropped | `>= 1e-2` | `1.011` (rectangles), `1.868` (triangles) | held |
| K1 rectangle stack against `LayerStack` | patches equal; `1e-13` | equal; volumes `4.34e-16`, centres `4.46e-16` | met |
| K2 prism stack structure; layer volumes | as stated; `1e-13` | as stated; `1.78e-15` | met |
| S1 `replace_mesh` with the same mesh | bit for bit | equal | met |
| S2 refusals: one cell short, another patch list, BDF2, `nu_face` | `Err` each, state unchanged | `Err` each with its message; unchanged | met |

Printed, no limit:

- P-Δ took 12 100 steps (the prisms' smaller cells give a smaller explicit step) in 12.5 s; the largest
  `abs(p_bar V^gamma / (p0 V0^gamma) - 1)` over the steps is `3.427e-5`; at `T` the cell values of `p / rho^gamma` lie in
  `[0.999986, 1.000137]`. ADR 0090's hexahedral run of the same loop: 9192 steps, P1 `1.528e-5`, P2 `3.415e-5`;
  `p_bar` at `T` is `3.606804933` on the prisms and `3.606804870` on the hexahedra (difference `6.3e-8`).
- G5L: G5 (iii)'s cell-velocity error `4.745e-3` of the plug flow (transverse `2.9e-15`), G5 (v)'s pressure error
  `1.015e-2` against G5's `4e-2`. G5X: `7.391e-3` (transverse `5.2e-15`) and `6.691e-3`.

**Reading.** The flux-level checks of both channel runs hold to round-off, so the face map and `replace_mesh` keep the
discrete continuity and the piston's swept flux through every event. The cell velocity next to the piston lags the plug
flow by 4.7e-3 of `a0 t` on the layered mesh, against the 2.9e-4 of ADR 0033's smoothed mesh (its known miss of G5
(iii), limit `1e-6 a0 t_end`, cause not proven there). B2 registers no cell-velocity limit, and this one was printed,
not checked. The layered piston cell is down to `0.4 h0`, about half the height of a smoothed cell at the end of G5
(`0.75 h0`), which is consistent with a lag that grows as the piston cell thins. That is a reading, not a diagnosis. P4 holds with
a margin of 1.4 (ADR 0090's hexahedra: 5.4e-13).

### Defaults

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| D1 L1 and L2 after the code | equal | `affff0bb34a248e2`, `52af54712423ce0a`: equal | met |
| D2 the existing suites, unedited | pass | see below | met |

D2 before the gate, release: `rcfd-mesh` unit tests (51 passed, the new ones included); `rcfd-fv` `layering_w38c` (6
passed, 1 ignored as before), `ale_flows` (5, 5 ignored as before), `ale_gcl` (4), `compressible_ale_f258` (8),
`compressible_ale_f258_records` (R1 to R4, 4), `exp_rot_pab` (5, 1 ignored), `mesh_smoothing` (5, 2 ignored), the
`compressible` unit tests (8, 1 ignored). The `rcfd-api` and `rcfd-cli` suites of D2 were left to the gate, which runs
every test.

### Mutations

Each was applied alone to the code of 0cbaf33 by an exact text replacement (a script in a scratch folder that checks
that the replaced text occurs once), in its own target dir, the named tests were run once, and the file was restored and
compared byte for byte with the kept copy (equal in all four).

| Id | Must fail | Result |
| --- | --- | --- |
| MU1 a merged cell gets only its first source | P3 | failed: P3 `2.30e-2` (the test stopped at its first assertion, P1 `-0.306`; P2 `-0.306`, P4 `0.251`; P5 to P7 held) |
| MU2 a merged side face from one old face | F1 | failed: F1 `1.81` (rectangles) and `2.00` (triangles) at the removal; F2 `0.43` and `0.45` |
| MU3 a `Closure` face gets 0 | F1, X5 | failed: F1 `1.06` and `1.09` at the addition; X5 `1.14` |
| MU4 the side fractions of a split swapped | F2 | failed: F2 `0.267` and `0.283` at the addition (F1 held, as M3 predicts: the closure keeps both balances) |

`scripts/limit_audit.py` reports no hit in the new test files (it does not scan unit tests in `src`).

Disclosures:

- Before the registered runs the new code and tests were compiled without running, laid out by rustfmt, and `cargo
  clippy -p rcfd-mesh -p rcfd-fv --all-targets -- -D warnings` was run on them (clean). No registered check ran before its
  registered run.
- D1 was run once after the code, before the registered checks.
- The mutations used the target dir `target-f257-mut`, so the kept build of 0cbaf33 was not touched.
- Two wording corrections after the gate, documents only: the unit-test count of "Built as registered" (three new next
  to the existing one, not four) and the height comparison of the reading (half the height of G5's last smoothed cell,
  not a quarter).

### The gate (2026-11-07)

Main stayed at c398654 while F-257 was built, so no merge was needed; 0213 is on no other branch. The gate ran on a
`git archive` copy of 990a285 (addendum 1 before this section) with three jobs and one process per test
(`scripts/gate_test.py`), in the target dir `E:/projects/cfd/w49i-work/target-f257-gate`. The machine lock was taken
with the atomic `mkdir` lock and owner file of `gate-lock.sh`, polled every 5 s instead of 60 s (as `gate-f291/outer2.sh`,
disclosed). It waited from 10:21 (UTC) behind the F-244 gate and was taken at 11:21. The start threshold of free physical
memory was 19 GB (the lead's figure for this gate; ADR 0207's gate used 20 GB), and the gate started with 20 GB free.

Result: `cargo fmt` per package 0, `cargo clippy --workspace --all-targets -- -D warnings` 0, tests **3108 passed, 1
failed, 869 ignored** (3109 test processes in 598 binaries, wall 6909 s, peak summed working set 2759 MB, largest time
against its limit 0.427). The lowest free physical memory during the gate was 10 GB (13:20), from other work on the
machine. The lock was released at the end (13:31).

The one failure is `rcfd-studio` `preview_cost_g1` `g8_overlay_frame_cost_at_the_edge_limit`, a frame-time test: median
10.136 ms over 100 frames against its limit of 8 ms (when in the gate it ran is not logged; the machine was loaded by other work). It is not on
the list of known flakes.
F-257 changes no file of `rcfd-studio` and none that it renders (`rcfd-mesh` `layering` and `rcfd-fv` `pimple::moving`
only). Run again alone on the gate's binary right after the gate, three times: passed 3 of 3 (medians 2.07, 2.32 and
2.63 ms; its control held each time). All 10 tests of this ADR (5 checks, 2 records, 3 unit tests) ran in the gate and passed, and so did the suites of D2
(`layering_ale_f259` of rcfd-api and rcfd-cli, `dynamic_mesh`, `inverse_volume_f290`, `baseline_hashes`, H1 of
`case_hashes_w48u`, and rcfd-fv's `layering_w38c`, `ale_flows`, `ale_gcl`, `compressible_ale_f258`,
`compressible_ale_f258_records`, `exp_rot_pab`, `mesh_smoothing`). F-257 adds no ignored test. This gate section is
documents only and was not gated again.

Q1 to Q5 stay open for the maintainer. F-257 is done in the roadmap by the decision rule (Q5; the recommendation is
done, the acceptance still the maintainer's).
