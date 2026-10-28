# ADR 0155: Job 48U, rotate the mesh instead of the flow: `[mesh.transform]` (rotation about z), the NACA 0012 example at an angle of attack with the flow along +x

Status: **registered 2026-10-28**, part 1 (design, set-up and every limit except the tolerance of E2), before any
registered run and before any code change; the tolerance of E2 is registered by amendment 1 after a disclosed smoke run
and before the counted runs. Job 48U (the letter after 48T; ADRs 0150-0154 are taken by jobs 48P-48T on other branches),
on local main 6867e40 (waves 48A-48O merged). House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
limits are fixed here before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit
intact; a defective set-up gets an amendment written before the corrected run; defaults of existing code stay
bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; no new crate, no unsafe code, the crate
graph stays acyclic; no commit trailers; nothing is pushed. Clean-room: our own code.

## The request (the maintainer, 2026-10-28, relayed)

"We should make it possible to rotate the mesh instead of the air flow." The NACA 0012 example
(`example-projects/wind-tunnel/cases/naca0012-aoa4.toml`) sets the angle of attack by turning the flow, in four places:
`[initial] velocity`, `boundaries.farfield.velocity_value`, `reports.cl.direction = (-sin a, cos a, 0)` and
`reports.cd.direction = (cos a, sin a, 0)`. The brief: a mesh transform in the case schema for every mesh source
(generator and file), applied at mesh build time in the one function the runner, the Studio preview, the service's
`render.png` and `rcfd run` use; vertices only; a rotation of exactly 0 is a no-op bit for bit; everything else in the
case stays in world coordinates (flow along +x, lift +y, drag +x), with a warning for mesh-relative features; periodic
patches refused unless consistent; validation, the Studio's case editor and preview; two rotated example cases with runs
made by the real solver through `scripts/make_example_projects.py`; the physics check rotated mesh against rotated flow,
with a registered tolerance, measured and reported honestly; docs; tests; the case hash of every existing case unchanged.

## Context (read at 6867e40)

* **The case hash counts defaults.** `SettingsTree::content_hash` (`crates/rcfd-core/src/settings.rs`) is FNV-1a of the
  serialised settings data, and `coerce` fills every field of an object schema with its default when the text leaves it
  out; `Case::to_toml`, `Case::to_json_string`, `--print-settings`, the service's `settings.get`, the journal's
  `case_hash`, the solver's `start` event `case_hash`, `rcfd-io`'s container settings hash and the Studio editor's save
  (`CaseEditor::to_bytes`) all serialise that whole tree. A new key with a default therefore changes the hash and the
  written text of every case (ADR 0133 question 5, the same for ADR 0124's three keys). A key the case does not set must
  not appear in the data at all.
* **Mesh building.** `rcfd_api::sim::build_mesh(case)` reads a mesh file (`mesh.source = "file"`, then `mesh.scale`
  through `gen::warp` when it is not 1) or runs one of 14 generators. Every user of a case's mesh calls it:
  `Simulation::new` (so `rcfd run`, the run service's runs, the service's sessions and `render.png`), `runctl::check_only`
  (`rcfd check`, `--check-only`), `rcfd validate`, `rcfd check-mesh`/`convert` of a case, the service's `mesh.check`,
  `mesh.partition`, `mesh.inflate` and `settings.validate`, the restart readers, the coarse pre-solve (a cloned case),
  the Studio's mesh preview (`preview.rs` `build_text`) and its start plan's cell count (`store_plan.rs`). `gen::warp`
  maps the points and rebuilds through `Mesh::from_topology`, which recomputes face centres, area vectors, cell centres
  and volumes and validates the mesh; it copies the patches unchanged.
* **Periodic patches** (`rcfd-mesh` `Patch`): a cyclic patch names its partner; `rotation: None` is a translation whose
  shift `FvGeometry::new` recomputes from the mean face centres of the pair (`rcfd-fv` `periodic_map`), `Some` is a
  stored `PeriodicRotation { origin, axis, angle }` (the `annulus_3d_sector` and `axial_fan_sector` generators, OpenFOAM
  files), applied to the face centres as stored. A rotation of the points alone leaves a stored rotation in the old frame.
* **2D meshes** have `Empty` patches normal to z; `FvGeometry::active` switches a direction off when the empty normals
  have `|n_k| > 0.99`; the Studio draws a 2D case face-on through its +z `Empty` faces.
* **Units.** A float setting without a dimension accepts a unit string through `Quantity::parse`; `"deg"` is a
  dimensionless unit of factor pi/180, so a degree key given as `"-4 deg"` would be stored as -0.0698 (read as degrees).
* **Set-up rules** (ADR 0031): `rules::check_case` evaluates `rules/setup_rules.json`; `crates/rcfd-api/tests/rules_w19c.rs`
  asserts exactly 21 rules and that its fixtures cover exactly the file's ids. `rcfd check`, `rcfd validate`, `rcfd run`
  and `check_only` show its warnings; the service's `settings.validate` (the Studio's Case > Validate) returns errors only.
* **The Studio** builds its case editor from `settings.schema` (`schema_model.rs`): an object is a group, a leaf shows its
  name, its doc as hover text, `x-rcfd-si-dimension` next to a dimensioned float, `[min, max]` next to a bounded number,
  nothing next to a list; a value missing from the settings shows the schema default; an edit goes through
  `settings.set` (`Case::set`). The preview builds through `rcfd_api::sim::build_mesh`.
* **The example projects** (ADR 0142): `scripts/make_example_projects.py` rebuilds a whole project (it deletes every run
  and runs all cases again); the staleness tests (`crates/rcfd-mcp/tests/example_projects_w48h.rs`,
  `crates/rcfd-studio/tests/example_projects_h.rs`) count 10 cases, 8 generator cases, the two mesh-file cases, 6
  wind-tunnel cases and 4 swapped G3 comparisons, and S4 reads the angle from the initial velocity. Sizes: wind-tunnel
  27 404 656 bytes (cap 35 000 000), all 31 658 565 (cap 40 000 000); an aerofoil run is about 3.71 MB. Job 48P adds
  Linux-built stores on another branch (ADR 0137 rule); this job adds the Windows set only.
* **The aerofoil runs** stop at their 2000-iteration cap (turbulence change about 4e-4, far above the 1e-6 target), so a
  rotated run and a flow-rotated run take the same number of iterations whatever their momentum residuals.

## Decisions (mine; none is a maintainer decision; the open points are questions at the end)

1. **D1 Keys.** An optional table `[mesh.transform]` with two keys:

   | key | type, default | meaning |
   |---|---|---|
   | `mesh.transform.rotate_z_deg` | number, `0.0`, range [-360, 360], degrees, a plain number | rotation of the mesh about the axis parallel to z through `rotate_origin`; positive = counter-clockwise seen from +z (looking down -z, x to the right, y up) |
   | `mesh.transform.rotate_origin` | list of 3 lengths [m], `[]` = (0, 0, 0) | a point of that axis, in the coordinates after `mesh.scale` |

   For an angle of attack alpha with the flow along +x and lift along +y the aerofoil turns **nose up**, which is a
   clockwise turn: `rotate_z_deg = -alpha`, `rotate_origin = [0.25, 0, 0]` (the quarter chord of a unit chord). Rotation
   about z only (decision, not "general axis/angle"): a tilt about another axis would turn the `Empty` planes of a 2D mesh
   away from z (the solved directions, the Studio's face-on view), and every shipped use (angle of attack, a sector's
   phase) is about z; `rotate_axis` can be added later without changing these keys. **No translation and no scale**: a
   uniform scale exists for mesh files (`mesh.scale`), a generator has its own lengths, and a translation needs the same
   world-coordinate review as the rotation for no use asked for (question 2).
2. **D2 Optional tables in `rcfd-core`.** `Schema::optional()` marks an object field that is absent from the data until a
   value inside it is set: `default_value` and `coerce` leave it out when the input has no such key; `get` of a path into
   an absent optional table gives the schema default (so readers, the Studio and `Case::set`'s named-entry logic see a
   value); `set` of a path into it creates the table with its defaults first; the JSON schema leaves it out of `required`
   and marks it `"x-rcfd-optional": true`. Nothing in the existing schema is optional, so every existing path behaves as
   before. Consequence: a case without `[mesh.transform]` has the same data, text, hash and mesh as before this job; a
   case that writes the table (even with the defaults) has its own hash, as any other edited case.
3. **D3 Display unit.** `Schema::unit("deg")` sets a display unit exported as `"x-rcfd-unit"`, for a value that has no
   SI dimension (an angle) or a list (the origin, `"m"`); the Studio's editor shows it next to the field. Values are not
   converted by it. The origin's items are lengths (`Dimension::LENGTH`), so `"25 cm"` is accepted and stored in m.
4. **D4 Where and how.** `build_mesh` builds the mesh as before (file and `mesh.scale`, or the generator) and then
   applies the transform, once, for every mesh source. The angle is reduced modulo 360 (exact); a reduced angle of 0
   (absent table, 0, -0, 360, -360) returns the mesh untouched: no call, no rebuild, bit-identical. Otherwise
   `rcfd_mesh::transform::rotate_z(mesh, deg, origin)`: x and y rotated about the origin, z copied unchanged (so the
   planes of a 2D mesh stay exactly where they were), exact cosine and sine at multiples of 90 degrees, every other
   angle through `to_radians` and `sin_cos`; the topology (points order, faces, owner, neighbour, patches with their kinds
   and periodic data, zones) is copied and `Mesh::from_topology` recomputes the derived geometry (face centres and area
   vectors, cell centres and volumes) and validates the result. Wall distance, non-orthogonality and every other derived
   quantity is computed later from that mesh as always.
5. **D5 Periodic patches.** A translational pair is kept: its shift is recomputed from the rotated patches. A stored
   periodic rotation is consistent with the mesh rotation only when its axis is parallel to z and passes through
   `rotate_origin` (the two rotations then commute and the stored transform is unchanged); otherwise the build is
   refused with a located error at `mesh.transform.rotate_origin` naming the pair, its axis and the fix. The stored
   rotation is never moved along (question 3).
6. **D6 World coordinates.** No setting of the case is rotated. Flow and force directions, gravity, body force and the
   bulk-flow direction are world quantities by intent and get no warning. Settings that place something relative to the
   geometry stay in world coordinates too and get a warning (id **MT101**, severity warning, category mesh, located at
   the setting) when the rotation is not 0: a non-empty position or axis list (`point`, `origin`, `position`, `centre`,
   `box_min`, `box_max`, `rotation_origin`, `rotation_axis`, `wall_rotation_origin`, `wall_rotation_axis`, `axis`,
   `reference_point`) anywhere in the settings; probe and moment reports (their point or origin, even at the default);
   `rotating_frame` and `mrf` sources; a boundary with `wall_rotation_rate` not 0; an expression or a profile table that
   uses x, y or z (initial, boundary, scalar, property and named expressions, `tables.<n>.inputs`); `dynamic_mesh.enabled`;
   an enabled `mixing_plane`; a `decomposition.method` other than `none`. MT101 is emitted by code in `rules::check_case`
   (after the data rules), not by a new data rule, so `setup_rules.json` and its 21-rule test are unchanged.
7. **D7 A degree is a number.** A string for `rotate_z_deg` (a unit string such as `"-4 deg"` would be read in radians)
   is refused with a located message, in a case file (`Case::check_json`) and through `Case::set` (`--set`, the
   service, the MCP tools).
8. **D8 Validation and the Studio.** `rcfd check`, `--check-only`, `rcfd validate` and `rcfd run` report MT101 and the
   periodic refusal; the Studio's Case > Validate (`settings.validate`) shows errors, so the periodic refusal but not
   MT101 (question 4). The editor shows `transform` in the `mesh` group with both keys, their units and hover texts,
   without a Studio change other than showing `x-rcfd-unit`. A case made by the case starter is edited the same way.
9. **D9 Example cases.** `example-projects/wind-tunnel/cases/naca0012-aoa4-rotated.toml` and
   `naca0012-aoa2-rotated.toml`: byte for byte the settings of `naca0012-aoa4.toml` / `naca0012-aoa2.toml` except
   `initial.velocity = [1, 0, 0]`, `boundaries.farfield.velocity_value = [1, 0, 0]`, `reports.cl.direction = [0, 1, 0]`,
   `reports.cd.direction = [1, 0, 0]` and `[mesh.transform] rotate_z_deg = -4` (-2), `rotate_origin = [0.25, 0, 0]`; the
   same store and solver settings, so the runs are comparable iteration by iteration. The mesh is the committed
   `meshes/naca0012-ogrid` (a far-field circle of 20 chords about (0.5, 0)), rotated with the aerofoil: the rotated-mesh
   problem is the flow-rotated problem turned rigidly by -alpha about the quarter chord, so the two differ only where
   the numerics are not invariant under a rotation (per-component inner solves and residual normalisation, round-off),
   on runs that stop at their iteration cap.
10. **D10 Generation.** `scripts/make_example_projects.py` gets the two cases in its wind-tunnel spec and an option
    `--add-missing`: run, through the real run service exactly as a rebuild does, only the spec's cases that have no
    committed run, keep every committed run byte for byte, then write the ledger and the run table from all runs and the
    manifest. The six committed runs are not run again in the repository (their bytes stay; job 48P's Linux stores are
    keyed by them). The ledger's `study` is `naca0012-angle-of-attack` with `alpha` 4 (2) as for the flow-rotated
    variants; the variant name tells them apart.
11. **D11 Docs.** `docs/case-schema.md` (a "Mesh transform" section), the wind-tunnel notes (`notes/naca0012.md`: "How
    to set the angle of attack" with both methods and the sign convention; `notes/README.md`), the doc comments.
12. **D12 The existing example tests change where the content changes.** The counts and lists of the staleness tests grow
    with the two cases (10 to 12 case files, 6 to 8 wind-tunnel cases and runs, `MESH_FILE_CASES` 2 to 4); S4 reads the
    angle of a rotated case from its transform; G3's lists stay as they are (the rotated cases get their own checks,
    E2-E3). No limit of ADR 0142 changes.

## Placement

`rcfd-core` (`settings.rs`: optional fields, display unit), `rcfd-mesh` (new `transform.rs`: `rotate_z`),
`rcfd-api` (`schema.rs`, `sim.rs` `build_mesh`, new `mesh_transform.rs`: reading the table, the periodic rule, MT101,
the checks; `case.rs`; `rules.rs` `check_case`), `rcfd-studio` (`schema_model.rs` and `ui.rs`: the display unit), the
example project, the script and its self-test, docs, tests. No new crate, no new dependency, no new workspace edge;
`Cargo.lock`, `workspace_graph.rs` and `docs/architecture.md` unchanged.

## Registered checks (limits fixed here, before any of them ran)

Release builds of this branch, `--test-threads 1`, output to files, this host. "The aerofoil mesh" is the committed
`example-projects/wind-tunnel/meshes/naca0012-ogrid`; "the angles" are -4, 2, 30, 90, -90, 180, 123.456 degrees; "the
meshes" are `cavity_2d` 8 x 8, `cylinder_ogrid` 32 x 12, `annulus_3d_sector` (8 x 4 x 3, 40 degrees), `box_2d` 6 x 4 with
`periodic_x` and the aerofoil mesh.

| Id | Check | Limit | Negative control (must fail or discriminate) |
|---|---|---|---|
| U1 | Rotation matrix of each angle | `max |R^T R - I| <= 1e-15`, `|det R - 1| <= 1e-15`, z row and column exactly (0, 0, 1) | a matrix with a scaled column (1 + 1e-12) fails the orthogonality check |
| U2 | Round trip `rotate_z(rotate_z(m, a, o), -a, o)` on the meshes, each angle, origin (0.25, -0.5, 0) | every point within `1e-12 max(1, |p|)` of the original | (U5 is the exact case) |
| U3 | Derived geometry of the rotated meshes | total volume `|sum V' - sum V| <= 1e-12 sum V`; per cell `|V' - V| <= 1e-12 V`, per face `| |S'| - |S| | <= 1e-12 |S|` and `|n' - R n| <= 1e-12` on the generator meshes, `1e-9` (each) on the aerofoil mesh, whose smallest cells (edges of about 1e-4 chord at coordinates of about 1) carry a round-off of the rotated coordinates near 1e-12 of their size; cell centres `|c' - (o + R (c - o))| <= 1e-12 max(1, |c|)`; the largest values reported | the unrotated mesh compared with `R` applied fails the centre check |
| U4 | Topology | owner, neighbour, face point lists, patch names, kinds, starts, sizes, periodic partners and stored rotations, cell and face zones equal exactly; points' z bit-equal | |
| U5 | Exact angles | 0, -0, 360, -360 through `build_mesh` with the table present: content hash and every point bit-equal to the case without the table; 90 degrees about the origin maps (x, y, z) to (-y, x, z) bit for bit | a rotation by 1e-9 degrees changes the content hash |
| U6 | Invariants on the aerofoil mesh rotated by -4 about (0.25, 0, 0) | wall distance to `airfoil` per cell within `1e-12` relative; the quality report's maximum non-orthogonality, skewness and aspect ratio within `1e-9` relative; cell count, patches and face counts equal | |
| K1 | Spellings | `[mesh.transform]` table, dotted keys under `[mesh]`, an inline table, JSON, and `Case::set` (the `--set` path) give equal settings data and equal meshes (content hash) | |
| K2 | Refusals | an unknown key in the table, a misspelled table name, a string angle, an origin of 2 components, an angle of 400: each refused with a diagnostic at its path; on the base code (record, R0) `[mesh.transform]` itself is refused as "unknown setting 'transform'" | (the record is the control: the key is unknown without this job) |
| K3 | Schema | the JSON schema lists `mesh.transform` with both keys, not in `mesh.required`, `x-rcfd-optional` true, `x-rcfd-unit` "deg" and "m"; the defaults of `Case::new()` validate against it and have no `transform` | |
| H1 | Existing cases unchanged | for every case file of `cases/`, `example-projects/*/cases/`, the test fixtures and the TOML blocks of `docs/` that parse: settings content hash, `to_toml` and `to_json_string` bytes, mesh content hash (where it builds) and the base rejection text equal to the record made on 6867e40 before any code change (R0) | (R0 is recorded by the same test; a changed value is reported with its file) |
| H2 | The rest unchanged | `baseline_hashes.rs` and the test suites of `rcfd-core`, `rcfd-mesh`, `rcfd-api`, `rcfd-cli`, `rcfd-mcp` and `rcfd-studio` pass, the example tests with D12's edits only | |
| P1 | Periodic | `box_2d` periodic_x rotated 30 degrees about (0.3, 0.2, 0) and `annulus_3d_sector` rotated 17 degrees about (0, 0, 0.7): `FvGeometry::new` succeeds and the partner face map equals the unrotated mesh's; the sector rotated about (1, 0, 0): `build_mesh` refuses at `mesh.transform.rotate_origin` | the sector's points rotated about (1, 0, 0) without the refusal: `FvGeometry::new` fails ("not matched by the periodic transform") |
| W1 | MT101 | a cavity case with a probe report, a moment report, a porous source with a box zone, a `rotating_frame` source, a rotating lid (`wall_rotation_rate`), an initial velocity expression in x and a named expression in y, rotated by -4: one MT101 at each of those paths and no other; the NACA rotated cases: none | the same case without the table, and with `rotate_z_deg = 0`: no MT101 |
| S1 | Studio editor (headless) | the editor's schema tree has `mesh.transform.rotate_z_deg` (float, range [-360, 360], unit "deg", the doc names the sign convention) and `mesh.transform.rotate_origin` (number list, unit "m"); on a copy of the aerofoil case `CaseEditor::edit_text` of `-4` and `[0.25, 0, 0]` commits, the saved bytes contain the table and parse to the edited settings | a string angle typed in the field is refused by the editor |
| S2 | Studio preview | the preview of the edited aerofoil case (the editor's text and the saved file) has the mesh hash of `build_mesh` of the rotated case and bounds within 1e-12 of the rotated points' bounds; the preview of a case without the table has the mesh hash it had before (R0's mesh hash) | the unrotated preview's bounds differ from the rotated ones |
| S3 | Case starter | a case made by the starter from `cases/wedge/wedge.toml` takes `rotate_z_deg = 10` in the editor, saves, previews rotated and validates | |
| E1 | The counted runs (`--add-missing`) | both rotated runs `finished`, solver state `not_converged` at iteration 2000, exit code 5, each at most 180 s; the six committed runs unchanged in git | |
| E2 | Rotated mesh against rotated flow | `|cl_rot - cl_flow| <= tol |cl_flow|` and `|cd_rot - cd_flow| <= tol cd_flow` at iteration 2000, for 4 and 2 degrees, against the committed runs of `naca0012-aoa4.toml` and `naca0012-aoa2.toml`; **tol registered by amendment 1** (after a disclosed smoke, before the counted runs); the largest difference of the `cl` and `cd` series over all 2000 iterations reported | the same comparison of the 4-degree rotated run with the 2-degree flow run fails |
| E3 | Lift direction | the rotated runs' `cl` > 0 and `cl(4) > cl(2)`; on the rotated aerofoil mesh (-4 about the quarter chord) the leading edge (x = 0 before) has y > 0 and the trailing edge (x = 1 before) y < 0 | +4 degrees flips both signs of the geometric check |
| E4 | Reproduction | `--rerun-check --project wind-tunnel` (all 8 runs rebuilt in a temporary copy): the reproducible files (ADR 0142 D10) of every run byte-equal to the committed ones | (ADR 0142 R1's) |
| E5 | Example folder | `--check` 0 problems; the Python self-test passes; S8: wind-tunnel and all under the ADR 0142 caps | |
| G1 | Hygiene | `cargo fmt -p` of the changed crates clean; `cargo clippy` of the changed crates with `--all-targets -D warnings` clean; `workspace_graph.rs` passes | |

R0, the record made on the base code 6867e40 before any code change (by the H1 test itself, with
`RCFD_W48U_RECORD=1`): the H1 values, the base rejection text of `[mesh.transform]`, the K1-K2 and U5 tests run on the
base code (they fail there: the key is unknown), and `--rerun-check --project wind-tunnel` with the base binaries (the
committed runs reproduce on this machine before the change, so E2 compares against what the current solver gives).

## Not in CI (for the maintainer at the window)

1. `cargo build --release -p rcfd-studio -p rcfd-runner -p rcfd-cli`; copy `example-projects\wind-tunnel` to a scratch
   folder; start the Studio on it.
2. Select `cases/naca0012-aoa4-rotated.toml`: the Mesh side shows the aerofoil nose up, the far-field circle around it;
   in the Case editor, `mesh` > `transform` shows `rotate_z_deg` -4 "deg" and `rotate_origin` [0.25, 0, 0] "m"; hover
   both. Type 0 in `rotate_z_deg`: the preview turns the aerofoil level as you type; type -4 again.
3. Select the run of `cases/naca0012-aoa4-rotated.toml`: `p` on the rotated aerofoil, suction on the upper side; the Plots
   panel's `cl` and `cd` against those of `cases/naca0012-aoa4.toml`.
4. Case > Validate on the rotated case: valid. Report what looked wrong.

## Out of scope

A general axis/angle rotation, translation and scale (D1); rotating case settings with the mesh; the NACA mesh generator;
a finer aerofoil mesh; Linux-built stores of the new runs (job 48P's set; the ADR 0137 rule applies to them).

## Questions for the maintainer (open at registration; the addendum may add to them)

1. Accept the optional-table mechanism in `rcfd-core` (D2) as the way new keys keep existing case hashes, for later keys
   too (it answers ADR 0133 question 5 for new tables, not for keys added to existing tables)?
2. Translation and scale in `[mesh.transform]` (left out, D1)?
3. Periodic rotation about another axis: refused (D5). Or move the stored periodic rotation with the mesh (always
   consistent geometrically, but a rotating frame given in world coordinates no longer matches it)?
4. Show set-up warnings such as MT101 in the Studio's Case > Validate (today errors only)?

