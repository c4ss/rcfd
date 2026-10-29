# ADR 0155: Job 48U, rotate the mesh instead of the flow: `[mesh.transform]` (rotation about z), the NACA 0012 example at an angle of attack with the flow along +x

Status: **registered 2026-10-28**, part 1 (design, set-up and every limit except the tolerance of E2), before any
registered run and before any code change; the tolerance of E2 is registered by amendment 1 after a disclosed smoke run
and before the counted runs; amendment 2 (MT101's key list was defective) before the corrected check; results in the
addendum: every registered limit met (H2 after re-running two pre-existing tests under their own assumptions, a fresh
`TEMP` and one thread: deviation 9), every control failed as required, no known miss; not yet accepted by the
maintainer. Job 48U (the letter after 48T; ADRs 0150-0154 are taken by jobs 48P-48T on other branches),
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

## Amendment 1 (2026-10-28, after the R0 record and a disclosed smoke run, before the counted runs)

What had run when this was written, and what it gave:

1. **R0 on the base code 6867e40**, before any code change: the H1 record (41 sources: 30 valid case files and doc blocks
   with their settings hash, written texts and mesh hash, 11 refused with their diagnostic paths; commit c2a8d72); the
   base answer to `[mesh.transform]`: "mesh.transform: unknown setting 'transform' (known: fan, source, generator, ...)";
   the case-level test file `crates/rcfd-api/tests/mesh_transform_w48u.rs` (K1-K3, U3-U6, P1, W1, E3) on the base
   code: **9 of 9 failed**, each at "unknown setting 'transform'" (K3 at the missing schema entry), the control that the
   key did not exist; `--rerun-check --project wind-tunnel` with the base binaries: the reproducible files of all 6
   committed runs byte-equal (30 of 30), so the committed aerofoil runs are what the current solver gives.
2. **The code**, then the unit and case-level tests (not counted runs; they are re-run with the counted set): H1 0 of 41
   sources differ; the 9 case-level tests pass (U3's largest per-cell volume difference 2.1e-13 relative, face area
   4.3e-13, normal 2.2e-13, all meshes; U6 wall distance 9.6e-14, non-orthogonality 1.5e-14 relative).
3. **Smoke (disclosed, not counted)**: `rcfd run` of `naca0012-aoa4-rotated.toml` and `naca0012-aoa2-rotated.toml` with
   this branch's binary, one thread each, outside the project (scratch output). Against the committed flow-rotated runs
   at iteration 2000:

   | alpha | `cl` rotated mesh | `cl` rotated flow | relative difference | `cd` relative difference | largest `cl` difference over 2000 iterations | largest relative difference over iterations 1001-2000 (`cl`, `cd`) |
   |---|---|---|---|---|---|---|
   | 4 | 0.3481949506 | 0.3481949567 | 1.8e-8 | 1.6e-8 | 3.0e-4 at iteration 24 | 5.0e-8, 3.9e-8 |
   | 2 | 0.1988332970 | 0.1988332781 | 9.5e-8 | 5.6e-8 | 6.5e-4 at iteration 19 | 3.3e-7, 1.7e-7 |

   The two problems are one problem turned rigidly (D9): the far-field circle turns with the aerofoil, so there is no
   discretisation difference of the far-field boundary at all. What differs is numerics that are not invariant under a
   rotation: the rounded coordinates, and (probably, not investigated) the per-component inner solves of the momentum
   equations, whose effect is largest in the first iterations (up to 6.5e-4 in `cl` near iteration 20) and decays as
   the outer iteration settles.

**E2's tolerance (registered here):** at iteration 2000, `|cl_rot - cl_flow| <= 1e-6 |cl_flow|` and
`|cd_rot - cd_flow| <= 1e-6 cd_flow` for both angles (the smoke's largest is 9.5e-8, a factor of 10 below; the runs'
own drift of `cl` over their last 1000 iterations is 3e-3, ADR 0142); and over iterations 1001-2000 of the monitor
series the same comparison at every iteration within `1e-5` (the smoke's largest 3.3e-7). The largest difference over
all 2000 iterations is reported, not limited. The control is unchanged (the 4-degree rotated run against the 2-degree
flow run fails). The counted runs are the script's `--add-missing` runs (E1); with the same binary and one thread they
are expected to reproduce the smoke's values exactly, which E4 does not check (the smoke is not committed).

**Deviations of the set-up found while writing the tests (before any counted run):**

1. U2 on `annulus_3d_sector` uses an origin on its periodic axis, (0, 0, 0.7), not (0.25, -0.5, 0): D5 refuses the
   registered origin for that mesh (P1 is the check of that refusal). The other meshes use (0.25, -0.5, 0).
2. U5's "(x, y, z) to (-y, x, z) bit for bit" is checked as exact values: `0 - 0` is `+0` in floating point, so a
   coordinate 0 may come back as `+0` where `-y` is `-0`; every other bit is exact, z bit for bit.


## Amendment 2 (2026-10-28, after the counted runs E1, before the corrected W1 run)

What I knew when writing this: the counted runs (E1) and the checks U1-U6, K1-K3, H1, P1, W1, S1-S3, E2, E3, E5 had run
and passed (results in the addendum); E4 was running. Reading the schema for the docs, I found D6's key list
**defective**: it misses the post-processing sample lines (`postprocessing.samples.<n>.p0`, `p1`) and plane directions
(`e1`, `e2`), which the brief names ("probe/sample points"), and other keys that place something relative to the
geometry: `normal` (a VOF half-space), `pivot`, `rest_position`, `orientation_axis` (dynamic mesh bodies), `disk_centre`,
`disk_axis` (actuator disks), `cone_axis` (particle injection cones). Corrected D6: those keys join the list of position
and axis keys (a non-empty value warns), and every `postprocessing.samples.<n>` entry warns at its `p0` (line) or `origin`
(plane) even at the default, as probes do. Nothing else of D6 changes. No solver run depends on MT101 (a warning never
changes a run), so E1-E4 stand.

**W1b (registered here, before it ran):** a cavity case rotated by -4 with a line sample (`p0`, `p1`), a plane sample at its
default origin, an actuator disk source (`disk_centre`, `disk_axis`) and a VOF-free `normal`-less set-up gets exactly one
MT101 at each of `postprocessing.samples.line.p0`, `postprocessing.samples.line.p1`, `postprocessing.samples.plane.origin`,
`sources.disk.disk_centre`, `sources.disk.disk_axis` (each key is its own setting; a box is the one pair warned once);
control: the same case without the table gets none.

## Addendum: results (2026-10-28, branch `worktree-agent-a19b8077ee9b9bd5a`)

Commits: 6f19884 (registration), c2a8d72 (R0), 9d50287 (code, case-level tests, the two cases, the script's
`--add-missing`), c2d30c2 (amendment 1), c436d3d (the counted runs, the example tests), ffb859a (the Studio checks, U2 on
the aerofoil mesh), 6cd65d2 (amendment 2), ec4330e (the amendment-2 code, W1b), faff240 (H1 and S2 compare Windows-made
mesh hashes on Windows only), c0e3b31 (the unregistered checks X1, X2, S1b), and this addendum (7f9cd91, 9cb3c90 and the
commit that adds the suite totals). The branch started at 1248a76 and was fast-forwarded to local main 6867e40 before
any work. Release builds on this
host (shared with other agents' builds), `--test-threads 1` for the registered tests, output to files. The counted runs
used the binaries built at c2d30c2, copied out of the target folder before the runs.

| Id | Limit | Measured | Result |
|---|---|---|---|
| R0 | the record before any code change | 41 sources (30 valid with their mesh, 11 refused); the base answer "mesh.transform: unknown setting 'transform'"; the 9 case-level tests failed on the base code, each at the unknown key; the base rerun check: 30 of 30 files of the 6 runs byte-equal | done |
| U1 | `<= 1e-15`, exact z row and column | orthogonality 2.2e-16, determinant 2.2e-16; z exact; the control (a column scaled by 1 + 1e-12) fails | pass |
| U2 | `<= 1e-12 max(1, |p|)` | generator meshes at most 5.1e-16, the aerofoil mesh 5.6e-16 | pass |
| U3 | total volume `1e-12`; per cell and face `1e-12` (generators), `1e-9` (aerofoil); centres `1e-12` | largest over the 5 meshes and 7 angles: total volume 3.5e-16, cell volume 2.1e-13, face area 4.3e-13, normal 2.2e-13 (all within the generator limit, the aerofoil included), centres 9.4e-16; control: the unrotated centres miss by 0.49 to 0.80 | pass |
| U4 | topology equal exactly | equal on 5 meshes x 7 angles; z bit-equal | pass |
| U5 | 0, -0, 360, -360 bit-identical; 90 exact; 1e-9 changes the hash | as required on all 5 meshes (90 degrees value-exact, amendment 1 deviation 2) | pass |
| U6 | wall distance `1e-12`; quality `1e-9` relative | wall distance 9.6e-14; maximum non-orthogonality 80.2687 deg both (1.5e-14), skewness 2.3e-13, aspect ratio 2.4e-13; 6144 cells, patches and sizes equal | pass |
| K1 | equal settings and meshes | table, dotted keys, inline table, JSON, `Case::set`, the TOML and JSON writers: settings `621d826a83f00930`, mesh `c5d96657ac8f4b3d` for all | pass |
| K2 | refused at their paths | 5 of 5 (unknown key in the table, misspelled table, string angle "give the angle as a plain number of degrees ...", 2-component origin, 400 outside [-360, 360]); `Case::set` of a string angle refused; `"25 cm"` in the origin stored as 0.25 | pass |
| K3 | the schema | `x-rcfd-optional` true, not in `mesh.required`, units "deg" and "m", range [-360, 360]; `Case::new()` has no table and validates | pass |
| H1 | 0 of the record differ | 0 of 41 (the new TOML block of `docs/case-schema.md` is not in the record) | pass |
| H2 | the suites pass | `rcfd-core`, `rcfd-mesh`, `rcfd-api` (166 test binaries): 987 passed, **1 failed**, 295 ignored, `baseline_hashes.rs` 3 passed and 1 ignored as before; `rcfd-mcp` and `rcfd-cli`: 138 passed, **1 failed**, 9 ignored (`workspace_graph.rs` 3 passed); `rcfd-studio` (`fake-claude` built first, one thread): 242 passed, 0 failed, 4 ignored; `rcfd-io` and `rcfd-post` (the other users of `rcfd_core::settings`): 238 passed, 21 ignored; `rcfd-gui` (frozen, reads the schema): 48 passed, 4 ignored. The two failures are pre-existing tests that fail on this host's set-up, not on this job's code (deviation 9); run as they expect (a fresh `TEMP`; one thread) they pass | met after re-running those two |
| P1 | matched as before; refused off-axis; control fails | `box_2d` periodic_x rotated 30 about (0.3, 0.2, 0): 8 periodic faces, the same partners; the sector rotated 17 about (0, 0, 0.7): 24 faces, the same partners; about (1, 0, 0): refused at `mesh.transform.rotate_origin` (the pair `theta_min`/`theta_max` about the axis through (0, 0, 0) named, with the fix); control: the points rotated alone, `FvGeometry::new` fails "periodic patches theta_min/theta_max are not matched by the periodic transform" | pass |
| W1 | one MT101 at each of 7 paths, no other; none without a rotation | the 7 paths, nothing else, warning/mesh; no table and `rotate_z_deg = 0`: none; the two rotated example cases: none | pass |
| W1b | (amendment 2) 5 paths; control none | the 5 paths (`p0` and `p1` of the line, the plane's `origin`, `disk_axis`, `disk_centre`), nothing else; without the table none | pass |
| S1 | the editor's tree, units, hover, edit | `rotate_z_deg` float [-360, 360] "deg", `rotate_origin` number list "m", hover texts with the sign convention and the quarter chord; no other leaf has a unit; edited, saved text parses back equal, valid; control: `-4 deg` refused ("is not a number"), no table created | pass |
| S2 | the preview's mesh = `build_mesh`; R0's hash without the table | editor text, saved file, `build_mesh` and the committed rotated run: `c5d96657ac8f4b3d`; a field typed as -2: `8a038de15728d249` (= `build_mesh`, = the 2-degree rotated run's mesh); without the table `8516e4b6ee41c1d9` (R0); bounds within 1e-12; control: the unrotated bounds differ by 0.0217 | pass |
| S3 | a starter case takes a rotation | the wedge example rotated 10: valid, preview `2256e49837221e34` = `build_mesh`, unrotated `3533926a50503374`; 1200 cells, still 2D | pass |
| E1 | finished, `not_converged` at 2000, exit 5, each at most 180 s | 53.1 s and 50.2 s; the six committed runs untouched in git (`--add-missing`) | pass |
| E2 | final `1e-6`, iterations 1001-2000 `1e-5` | 4 deg: `cl` 0.3481949506 vs 0.3481949567, 1.8e-8; `cd` 1.6e-8; late 5.0e-8 and 3.9e-8. 2 deg: `cl` 0.1988332970 vs 0.1988332781, 9.5e-8; `cd` 5.6e-8; late 3.3e-7 and 1.7e-7. Largest over all iterations: `cl` 3.0e-4 at iteration 24 (4 deg), 6.5e-4 at 19 (2 deg); `cd` 6.0e-5 at 16, 2.8e-5 at 20. The counted runs gave the smoke's values to every printed digit. Control: 0.75 and 0.34, fails | pass |
| E3 | `cl` > 0, `cl(4) > cl(2)`; geometric signs | 0.3482 > 0.1988 > 0; -4 about the quarter chord: leading edge y = +0.01744, trailing edge y = -0.05232; +4 flips both (control) | pass |
| E4 | byte-equal | all 8 runs rebuilt in a temporary copy with the branch's binaries: 40 of 40 reproducible files and the 5 polyMesh files byte-equal, so the six old runs are what the changed solver gives too | pass |
| E5 | `--check` 0, self-test, caps | `--check` 0 problems; self-test 8 passed; wind-tunnel 34 826 293 bytes (cap 35 000 000), all 39 080 436 (cap 40 000 000) | pass |
| G1 | fmt, clippy, graph | `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` of `rcfd-core`, `rcfd-mesh`, `rcfd-api`, `rcfd-mcp`, `rcfd-studio`: clean; `workspace_graph.rs` 3 passed | pass |

**What the comparison means.** The rotated-mesh and the flow-rotated problems are one problem turned as a whole (D9), so the
"discretisation difference of the far-field boundary orientation" the brief expected is zero by construction: the far-field
circle turns with the aerofoil. The measured differences (1.8e-8 and 9.5e-8 at the end) are numerics that depend on the
axes: the rounded rotated coordinates and, probably (not investigated), the per-component inner solves of the momentum
equations, which differ most in the first iterations (`cl` apart by up to 6.5e-4 near iteration 20) and fade as the
outer iteration settles. Both forms are therefore interchangeable for these runs; the rotated form needs one setting
instead of four.

**Deviations and notes.**
1. Amendment 1: U2 on the sector with its on-axis origin; U5's 90-degree map value-exact (signed zeros).
2. Amendment 2: D6's key list was defective (sample lines and planes and other geometric keys missing); corrected and
   checked by W1b. No run depended on it.
3. D12: the example tests changed where the content changed (12 case files, `MESH_FILE_CASES` 4, 8 wind-tunnel cases in
   the Studio's H1, S4's rotated branch); no ADR 0142 limit changed. G3's lists are unchanged.
4. H1 records 8 of the 10 TOML blocks of `docs/` (2 are fragments that do not parse as a case on the base code either).
5. The smoke (amendment 1) ran with the binary of the code at 9d50287 before `cargo fmt`; the counted runs with c2d30c2's.
   Their `cl` and `cd` are equal to every printed digit; the smoke's files are not kept.
6. `render.png` of the service draws a session's simulation, whose mesh is `build_mesh`'s (`Simulation::new`); it has no
   test of its own here (code path only). The MCP tool `edit_case` sets the table with its smallest-change TOML edit
   (it adds `[mesh.transform]`); not exercised by a test.
7. `CHANGELOG.md` "Unreleased" is not edited (no 48x job edited it; question 6).
8. A probe `timeout 20 cargo build -p rcfd-mcp --tests` (to see whether the build folder was locked while a test run went
   on) ended my own cargo build after 20 s; no other process was touched. Reported against the standing rule not to kill
   cargo; it is not repeated.
9. Two failures of the suites that this job does not cause, each passing when run as the test expects: `rcfd-mcp`'s
   `design_tests::t2_paths_are_sandboxed_and_names_are_restricted` asserts that `%TEMP%\x` does not exist, and on this host
   that folder exists since 2026-10-15 (made by something else); with `TEMP` and `TMP` set to a fresh folder it passes.
   `rcfd-api`'s `restart_w19c` `s5_coarse_to_fine_iteration_counts_are_reported` and `s5b_...` share one temporary folder
   (`tmp("s5")`, the same process id), so the default parallel runner lets one remove the other's checkpoint ("cannot find
   the file specified"); with `--test-threads 1` the file passes (7 passed, 1 ignored known miss).

**Not run.** The "Not in CI" checklist (a person at the window); Linux: the rotated mesh's coordinates come from the
platform's `sin_cos`, so on Linux the two new runs' store mesh hashes (S5), their reproducible bytes (E4) and the solver
fields may differ in the last bits: their Linux-built stores need regenerating with job 48P's set (ADR 0137 rule; only
the Windows set is added here); the workspace gate (only the changed crates' suites and clippy); `cargo deny` (no new
dependency).

**New dependencies:** none (no crate, no `Cargo.lock` change, no new workspace edge).

**Unregistered checks (after the counted runs, reported, no limit of this ADR).**

| Id | What | Result |
|---|---|---|
| X1 | `docs/case-schema.md` says a study can sweep the angle: `[study.parameters.a] path = "mesh.transform.rotate_z_deg"`, values -2 and -4, on a cavity | `study::plan` gives 2 points; each point's case builds the mesh the table written in the case builds (`c29bcc7fee53e81e`, `54bbc10d24df50e6`) |
| X2 | the docs say a rotationally periodic generator only turns about its axis: `cases/axial_fan_sector/axial_fan_sector.toml` | rotated 10 about (0, 0, 0.3): builds, `FvGeometry::new` matches 1288 periodic faces; about (1, 0, 0): refused at `mesh.transform.rotate_origin` naming `seam_lo`/`seam_hi` |
| S1b | the drawn editor (S1 checks the model): one headless frame of the app, the cavity open, the filter `transform` | both fields drawn (`field_rects`), with the texts `mesh`, `transform`, `rotate_z_deg`, `deg`, `[-360, 360]`, `rotate_origin`, `m` |

**Merging with the other 48x branches (read from their commits, not merged here).**
* **48R (ADR 0152)**: its DV6 (`crates/rcfd-studio/tests/saved_store_r.rs`, `dv6_unedited_plans_are_bit_identical`, commit
  6fbdbb7) enumerates `example-projects/*/cases/*.toml` at run time and asserts 22 cases equal to its record made on
  6867e40. With this branch merged there are 24 (the two rotated cases), so DV6 fails on the count and the label list.
  The fix at the merge: DV6 takes the cases its record names (the two new cases have no plan recorded on 6867e40), with a
  note in ADR 0152; the record itself stays as it is.
* **ADR 0133**: 48R adds a dated note at its end too; both branches append there, so the merge has a textual conflict to
  resolve by keeping both notes.
* **48P (the Linux-built stores)**: the files both branches touch are `example-projects/manifest.json`, the wind-tunnel
  ledger and the run table of its `notes/README.md`, `crates/rcfd-mcp/tests/example_projects_w48h.rs`,
  `crates/rcfd-studio/tests/example_projects_h.rs` and the size caps (wind-tunnel 173 707 bytes under its cap with the
  two new runs). The two new runs have no Linux set (ADR 0137 rule); they need one made like 48P's.

**Recommendations for the questions** (registration 1-4, addendum 5-9), each with its options; none is decided here.
1. Optional settings tables in `rcfd-core` (D2) as the way a new table keeps every existing case hash. Options: (a) accept
   it as the pattern for new tables; (b) make `content_hash` and the writers skip every key at its default (changes every
   existing hash once, ADR 0133 question 5); (c) accept that a new key changes all hashes. **Recommendation (a)**: no
   existing hash, text or test moves; (b) is the general answer for keys added to existing tables, a separate change.
2. Translation and scale in `[mesh.transform]`. Options: (a) leave out until a use needs them; (b) add `translate` now
   (rigid, safe for the mesh, same world-coordinate warnings); (c) add a uniform scale (overlaps `mesh.scale`).
   **Recommendation (a)**; (b) is cheap when a use appears.
3. A rotationally periodic mesh turned about another axis. Options: (a) refuse, as now; (b) move the stored periodic
   rotation with the mesh (the pair stays matched, but a rotating frame given in world coordinates no longer matches it).
   **Recommendation (a)**: a fan or sector turned off its axis is almost always a mistake, and the error says the fix.
4. MT101 in the Studio. Options: (a) Case > Validate also lists the set-up rule warnings (`rules::check_case`); (b) keep
   errors only. **Recommendation (a)**: a small Studio change, and the warnings are exactly what a rotated case needs.
5. Size caps with 48P's Linux stores. Options: (a) raise the caps per project; (b) keep one platform's stores in git and
   build the other's on demand; (c) move the stores to a release asset or Git LFS (ADR 0142 question 1).
   **Recommendation (b)** for now (the CI can rebuild the Linux set), (c) before the examples grow further.
6. `CHANGELOG.md`. Options: (a) one Unreleased entry for the 48x waves at the next release; (b) an entry per job now (every
   48x branch edits the same line, so merges conflict). **Recommendation (a)**.
7. The early-iteration difference of the two forms (up to 6.5e-4 in `cl` near iteration 20, 1e-7 at the end). Options:
   (a) accept; (b) find its cause (per-component inner solve tolerances are the likely one). **Recommendation (a)**: it is
   gone long before a result is read.
8. The display unit "deg" for the existing degree keys (`ramp_angle`, `sector_angle`, the fan's angles). Options: (a) add
   it (the JSON schema changes, no case hash or text does); (b) leave them. **Recommendation (a)**, in a later job.
9. A rotated channel with translational periodic patches keeps `solver.bulk_direction` and `physics.body_force` in world
   coordinates, so the driver no longer runs along the channel, and MT101 does not warn (D6 counts them as world
   quantities by intent). Options: (a) warn (MT102) when the mesh has periodic pairs and a driver is set; (b) leave it to
   the docs. **Recommendation (a)**: cheap, and the mistake would otherwise be silent.

## Note (2026-10-29, [ADR 0157](0157-job48w-merge-48u-rotated-cases-with-the-linux-tree.md))

Merged into main with job 48P's Linux tree and job 48R's DV5/DV6 by job 48W: the two rotated cases and their Linux-built
runs are in `example-projects-linux/` too (`--docker --add-missing`), DV6 takes the cases its record names. Results there.

2026-10-29 (ADR 0169, job 49F): the regenerated aerofoil runs converge at 409 / 230 iterations, so E2 (iteration 2000, 1001-2000) is the ignored known miss `e2_rotated_mesh_matches_rotated_flow_known_miss`, limit intact; at the final iteration the two forms agree within 5.3e-9 (ADR 0169).
