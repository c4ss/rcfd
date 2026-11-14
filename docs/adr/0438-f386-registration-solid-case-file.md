# ADR 0438: F-386 registration: a solid case file of its own and the library entry that runs it

Status: **REGISTERED 2026-11-14, written before any code of F-386 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-386 was written, compiled or run: the code and documents under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by hand from closed forms and from measured
numbers recorded in earlier ADRs (no library code run). Results go in the addendum at the end; nothing above the addendum is
changed after a run.

Parent: [ADR 0287](0287-solid-case-file-and-library-run-entry.md) (the planned stub, SB-17 of the basic tier,
[ADR 0270](0270-structural-solver-basic-tier-plan.md)). Roadmap item F-386 (depends on F-371, F-377, F-381, F-383, F-013,
F-080). Size M. Branch `f386-solid-case-file` from main 52ce5ea6, worktree `w49i-work/f386`, scratch `w49i-work/f386-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical: the only
existing file edited is `crates/rcfd-api/src/lib.rs` (one `pub mod solid_case;` line); the new module adds items only and no
existing function of any crate is changed, so every flow case, the flow schema, the CLI, the MCP tool list and every hash
table give exactly what they gave before (checks S1, S2); no edit of `baseline_hashes.rs`, of the case and mesh hash tables or
of any golden file; no existing test edited; no new dependency (`rcfd-api` already depends on `rcfd-core`, `rcfd-fem`,
`rcfd-io`, `rcfd-linalg`, `rcfd-post`, `serde_json` and `toml`), no unsafe code; clean-room (first principles, derived below).

## What F-386 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. A solid case file (TOML of its own: mesh from a generator
> or a Gmsh file, material, constraints and loads on named sets, static or modal analysis, solver choice, output) is read and
> run by a library function of rcfd-api that writes the .vtu and a JSON report (displacement and stress extremes, reactions,
> frequencies, solver statistics): an unknown key, a missing set, a wrong unit or type each give an error with the key and the
> line; a file run gives the bits of the same model built by library calls (limit 1e-12, expected identical); the cantilever and
> the bar-frequency checks pass when run from files, and a Gmsh tetrahedral mesh of a box read through rcfd-io gives the
> uniaxial closed form within 1e-10; no flow case key, CLI help text, MCP tool list, hash table or golden changes (the existing
> schema, CLI and tool-list tests pass unedited).

The roadmap note adds the solver mappings decided earlier: the static iterative route is `StaticSolver::PreconditionedCg` with
`CgPreconditioner::BlockJacobi` or `CgPreconditioner::IncompleteCholesky(IcOptions::default())` (ADR 0282); the recommended
iterative choice is `CgPreconditioner::IncompleteCholeskyFill { options: IcOptions::default(), level: 1, ordering:
IcOrdering::Natural }` and a key should map onto it and onto the ordering (ADR 0357); a key should be able to select
`CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::default())` (ADR 0305), `ElasticAmgSettings::quadratic()` (ADR 0370)
and `ElasticAmgSettings::quadratic_smoothed()` (ADR 0375); the result is written by
`rcfd_post::solid_output::write_solid_vtu(path, model, result)` and a multi-material model is refused by the nodal average
(ADR 0278).

Clauses: (c1) a TOML file of its own with mesh (generator or Gmsh file), material, constraints and loads on named sets, static or
modal analysis, solver choice, output; (c2) read and run by a library function of `rcfd-api`; (c3) that writes the `.vtu`; (c4)
and a JSON report with displacement and stress extremes, reactions, frequencies, solver statistics; (c5) an unknown key gives an
error with the key and the line; (c6) a missing set the same; (c7) a wrong unit the same; (c8) a wrong type the same; (c9) a
file run gives the bits of the same model built by library calls, limit 1e-12, expected identical; (c10) the cantilever checks
pass from files; (c11) the bar-frequency checks pass from files; (c12) a Gmsh tetrahedral box read through `rcfd-io` gives the
uniaxial closed form within 1e-10; (c13) no flow case key, CLI help text, MCP tool list, hash table or golden changes, the
existing schema, CLI and tool-list tests pass unedited; (c14, note) the iterative keys map onto the `CgPreconditioner` choices
of ADRs 0282, 0357, 0305, 0370 and 0375.

## What was read (main 52ce5ea6, read only)

- ADR 0287 (the stub), ADR 0257 (its check K0: how a schema addition was shown to need no golden edit; the list of tests it ran),
  ADR 0433 (F-379: the cantilever of slenderness 20, its support D2, load D3, measure `delta / delta_T`, limits C1 0.01 and C2
  0.05, measured 5.29e-4 and 3.21e-2), ADR 0284 (F-383: the fixed-free bar, limits B1 5e-3 and B3 1e-9, measured 1.004e-4,
  9.038e-4, 2.512e-3), ADR 0282, 0357, 0305, 0370, 0375 (the preconditioners), ADR 0278 (the `.vtu`), ADR 0436 (the newest
  registration, for the form).
- Code: `crates/rcfd-api/src/case.rs` (`Diagnostic`, `collect_errors`, `locate_line`, `Case::parse_toml_with`: the TOML is
  parsed by `toml` into a `serde_json::Value`, checked against a `rcfd_core::settings::Schema` by `collect_errors`, each
  diagnostic then gets its line from `locate_line`), `crates/rcfd-core/src/settings.rs` (`Schema`, `Kind`, `coerce`,
  `SettingsTree`; unit strings converted to SI by `Quantity::parse` and `expect(dim)`), `crates/rcfd-core/src/units.rs` (the
  dimensions), `crates/rcfd-api/src/schema.rs` lines 60 to 110 (the flow `[mesh]` table), `crates/rcfd-api/src/cosim_case.rs`
  (an optional table checked after the schema), `crates/rcfd-api/Cargo.toml`, `src/lib.rs`; `crates/rcfd-fem/src/builders.rs`
  (`hex_box`, `tet_box`, `upgrade_to_tet10`, the sets `all`, `xmin` .. `zmax`), `convert.rs` (`from_mesh`: sets per patch and
  zone, `all`), `model.rs` (`Material::isotropic_elastic`, `Constraint::displacement`, `Load::*`, `SolidModel`, `SolidResult`,
  `SolverStats`), `solve.rs` (`StaticSolver`, `CgPreconditioner`, `IcOrdering`, `StaticOptions`, `solve_static_report`,
  `SolveReport`), `modal.rs` (`ModalOptions`, `ModalResult`, `modal_analysis`); `crates/rcfd-io/src/gmsh.rs` lines 1 to 60 and
  `read_file`, `crates/rcfd-io/tests/gmsh_samples.rs` (the files `tests/data/tet_v{2.2,4.1}_{ascii,bin}.msh`: real Gmsh 4.15.2
  output of the box `[0, 2] x [0, 1] x [0, 1]`, 1139 tetrahedra, patches `inlet` (x = 0), `outlet` (x = 2), `walls`, zone
  `fluid`); `crates/rcfd-post/src/solid_output.rs` (`solid_fields`, `solid_vtu_string`, `write_solid_vtu`, `vtk_from_fem`),
  `crates/rcfd-io/src/vtk.rs` (`fe_vtu_string`, `FeCellBlock`, `FeArray`); the tests `crates/rcfd-fem/tests/sb_closed_forms.rs`
  (cantilever) and `sb_modal.rs` (bar); the hash and scan tests `crates/rcfd-api/tests/case_hashes_w48u.rs` (hashes every
  `.toml` under `cases/`, the example projects and every crate's `tests/fixtures/`, and every fenced `toml` block of
  `docs/**/*.md`), `rules_w19c.rs` (evaluates `cases/*.toml`, two docs, and string literals of test sources that contain
  `[mesh]` or `generator`; a text that is not a valid flow case is skipped), `w48v_case_hashes.rs`.
- Scaffold rows: `grep -rn "F-386\|0287\|solid_case"` over `crates/` finds no test or code row (the only hits are coordinates in
  two `.msh` files). There is no scaffold row to retire.

Consequences for the design: the solid case files of the tests are placed under `crates/rcfd-api/tests/solid_cases/` (not under
`tests/fixtures/`, which `case_hashes_w48u` hashes), and the format document under `docs/` uses no fenced `toml` block (a fenced
`text` block), so that neither hash table sees a new file; string literals of the new test that contain `[mesh]` are not valid
flow cases and are skipped by `rules_w19c` (its counts are printed, not asserted).

## Arithmetic before this registration (disclosed)

1. Uniaxial tension of a box `[0, 2] x [0, 1] x [0, 1]`, `E = 2e11 Pa`, `nu = 0.3`, traction `(s, 0, 0)` with `s = 1e6 Pa` on
   `outlet` (x = 2), traction-free `walls`, `u_x = 0` on `inlet` (x = 0), `u_y = u_z = 0` at the node `(0, 0, 0)` and `u_z = 0`
   at the node `(0, 1, 0)` (both are Gmsh corner points, nodes of the file: lines 48 and 54 of `tet_v4.1_ascii.msh`). The
   stress `sigma = s e_x e_x` satisfies equilibrium (constant) and the traction conditions on every face; Hooke's law gives
   `eps_xx = s/E`, `eps_yy = eps_zz = -nu s/E`, shear 0; integrated with the constraints: `u = (s x/E, -nu s y/E, -nu s z/E)`
   (at `(0,0,0)` zero; at `(0,1,0)` `u_z = 0`: consistent, and the constraints remove exactly the six rigid motions: `u_x = 0` on
   `x = 0` the translation x and the rotations about y and z, the first node the translations y and z, the second node the
   rotation about x). The field is linear, so a Tet4 mesh represents it exactly, the consistent traction load of a linear
   field on a plane face is exact, and the finite-element solution equals it up to rounding. Scale `max |u| = |u_x(2)| = 1e-5 m`.
   Rounding estimate: `eps cond(K)` with `cond(K) ~ (L/h_min)^2` of order 1e3 to 1e4 for 351 nodes: 1e-13 to 1e-12 relative;
   the acceptance's 1e-10 has a margin of 100.
2. Cantilever (ADR 0433 D1 to D5, reused, not re-derived): `L = 2`, `h = 0.1`, `P = 1000 N` as a traction `(0, 0, -P/h^2) =
   (0, 0, -1e5) Pa` on `xmax`, `E = 2e11`, `nu = 0.3`; `delta_T = P L^3/(3 E I) (1 + 3 E I/(kappa G h^2 L^2))`, `I = h^4/12`,
   `kappa = 5/6`, `G = E/(2 (1 + nu))`: `delta_T = 1.603120e-3 m` (ADR 0433 addendum). Support D2: `u_x = 0` on `xmin`, `u_z =
   0` at the root nodes `(0, 0, h/2)` and `(0, h, h/2)`, `u_y = 0` at `(0, 0, h/2)`. In a file the two nodes are node sets
   selected by a box around each point (D3 below). The file model is the model of `sb_closed_forms.rs` up to the order of the
   constraints, so the expected ratios are those measured there: Tet10 `40 x 2 x 2` 1.000529, Hex8 `80 x 4 x 4` 0.967873.
3. Bar (ADR 0284 D5, reused): `L = 1`, `b = 0.1`, `E = 2e11`, `rho = 7850`, `nu = 0`; `xmin` `u_x = 0`, `ymin` `u_y = 0`,
   `zmin` `u_z = 0`; `f_n = (2n - 1) sqrt(E/rho)/(4 L)`; the 1-D consistent-mass chain `lambda_h = 6 E/(rho h^2) (1 - cos k h)/
   (2 + cos k h)`, `k = (2n - 1) pi/(2 L)`. Hex8 `32 x 1 x 1`, four modes: errors 1.004e-4, 9.038e-4, 2.512e-3 against the
   closed form (ADR 0284 addendum), the discrete form within rounding (2e-12).
4. Reactions of the cantilever's support: the support is statically determinate in z (two nodes, symmetric), so the z reactions
   of the two root nodes sum to `P` and each is `P/2` up to the solve's residual (ADR 0433 K1: 1e-11 measured at limit 1e-8).

## Design

### D1. The file and its schema (clauses c1, c13)

A TOML file read by `toml` into a `serde_json::Value` and checked against a schema of its own, `solid_case::schema()`, built
from `rcfd_core::settings::Schema` (the machinery of the flow case reader, no new crate). The flow schema `case_schema()` is not
touched, so its JSON Schema export, its hashes and every golden stay as they are. Tables (every unit string goes through
`Quantity::parse` and the declared dimension, as in the flow case; numbers are SI):

- top level: `title` (string).
- `[mesh]`: `source` = `generator` | `file` (default `generator`). Generator: `generator` = `box` (the only one), `element` =
  `hex8` | `tet4` | `tet10` (`rcfd_fem::builders::hex_box`, `tet_box`, `tet_box` then `upgrade_to_tet10`), `origin` (3
  lengths, default the origin), `lengths` (3 lengths, required), `divisions` (3 integers >= 1, required). File: `file` (a Gmsh
  `.msh` path relative to the case file), read by `rcfd_io::gmsh::read_file` and converted by `rcfd_fem::convert::from_mesh`
  (all tetrahedra or all hexahedra; its refusals name the cell), `quadratic` (bool, default false: Tet4 upgraded to Tet10).
  The sets of the mesh are those of the builder or of `from_mesh` (`all`, one node and surface set per box face or patch, one
  element set per cell zone).
- `[node_sets.<name>]`: `box_min`, `box_max` (3 lengths each): the nodes `p` with `box_min <= p <= box_max` componentwise
  (inclusive), in ascending node order; a box that selects no node, or a name that exists already, is an error with the key and
  line.
- `[materials.<name>]`: `element_set` (default `all`), `youngs_modulus` (pressure, required), `poisson_ratio` (number,
  required), `density` (density, default 0), `expansion` (number per kelvin, default 0, given as a plain number);
  `Material::isotropic_elastic`; at least one material.
- `[constraints.<name>]`: `set` (a node set, required), `components` (list of `x`, `y`, `z`, required), `value` (length,
  default 0): one `Constraint::displacement(set, c, value)` per component in the order listed.
- `[loads.<name>]`: `kind` = `traction` | `pressure` | `nodal_force` | `gravity`; `set` (a surface set for `traction` and
  `pressure`, a node set for `nodal_force`, none for `gravity`), `traction` (3 pressures), `pressure` (pressure), `force` (3
  forces, per node), `acceleration` (3 accelerations). The key of the kind is required, a key of another kind is an error naming
  it; `Load::traction`, `Load::pressure`, `Load::nodal_force`, `Load::gravity`.
- `[analysis]`: `type` = `static` | `modal` (default `static`); `modes` (integer >= 1, default 6), `tolerance` (default 1e-10),
  `residual_test` = `relative` | `backward` (default `relative`) for modal (`ModalOptions::new(modes).tolerance(..)
  .residual_test(..)`, shift rule of ADR 0284).
- `[solver]` (static only): `method` = `direct` | `cg` (default `direct`, `StaticSolver::Direct`), and for `cg`
  `preconditioner` (D5), `fill_level` (integer >= 0, default 1), `ordering` = `natural` | `node_rcm` (default `natural`),
  `tolerance` (default 1e-10), `max_iterations` (default 20000). `method = cg` with `type = modal` is an error naming
  `solver.method` (the modal solve has its own eigensolver; silently ignoring a key is not accepted).
- `[output]`: `vtu` (default `result.vtu`; empty: none), `report` (default `report.json`), relative to the case file directory
  or to an output directory given to the run function.

Tables are named collections (`Kind::Named`), never arrays of tables, so that `locate_line` (which reads `[a.b]` headers and
`key = value` lines) finds each key.

### D2. Errors with the key and the line (clauses c5 to c8)

Every problem is a `case::Diagnostic { path, line, message }`, all problems of a file returned together, as in the flow reader:

- a TOML syntax error: the line from the parser's span (as `Case::parse_toml_with`);
- schema problems by `case::collect_errors` with the solid schema: unknown key (`unknown setting 'k' (known: ...)`), wrong type
  (`expected integer, got ...`), wrong unit (the `Quantity` error, e.g. `expected dimension Pa, got m`), value out of range,
  value not in the enumeration;
- after the schema: a required key missing (path of the key, line of its table), a key of another load kind or analysis, a
  missing set (the path of the `set` or `element_set` key, the message naming the set and its kind), an empty selection or a
  duplicate set name, a list of the wrong length, a refusal of `rcfd-fem` while the model is built (the path of the table that
  added the item, the message of `FemError`), a Gmsh read error (the path `mesh.file`).

The line of a path is `case::locate_line(text, p)` where `p` is the path without trailing list indices (the line of
`traction = [..]` for `loads.tip.traction.2`); a path not in the file (a required key that is missing) gets the line of its
table header by `locate_line`'s fallback.

### D3. Node sets by a box (clause c1)

Inclusive componentwise comparison, no tolerance added (the file gives the box; a point is selected by a box a little larger
than the point, e.g. `+-1e-9 m`). Gmsh coordinates are printed with 16 to 17 digits, so a box of `+-1e-9` around a corner point
selects that node only on a mesh whose node spacing is above 2e-9.

### D4. The model and its run (clauses c2, c9)

`SolidCase::build` turns the checked tree into a `SolidModel` by the library calls a user would make, in file order (BTreeMap
order of the named tables, i.e. alphabetical by name, which the format document states): mesh, node sets, materials,
constraints, loads. `run` calls `solve_static_report(model, &[], &[], &options)` or `modal_analysis(model, &options)`. The
check of c9 builds the same model in the test by direct library calls (the same builder, the same set, material, constraint
and load calls in the same order) and compares the results bit for bit (`to_bits` of every displacement, reaction,
eigenvalue and mode value); the registered limit is the acceptance's 1e-12 relative (largest `|a - b| / max |b|`), the
expectation identical (0).

### D5. Solver keys (clause c14)

| `method` / `preconditioner` | `StaticSolver` |
| --- | --- |
| `direct` | `Direct` |
| `cg` / `block_jacobi` | `PreconditionedCg { BlockJacobi, tolerance, max_iterations }` (ADR 0282) |
| `cg` / `ic0` | `PreconditionedCg { IncompleteCholesky(IcOptions::default()), .. }` (ADR 0282) |
| `cg` / `ic_fill` (default) | `PreconditionedCg { IncompleteCholeskyFill { options: IcOptions::default(), level: fill_level, ordering }, .. }`, `natural` -> `IcOrdering::Natural`, `node_rcm` -> `IcOrdering::NodeReverseCuthillMcKee` (ADR 0357) |
| `cg` / `amg` | `PreconditionedCg { SmoothedAggregation(ElasticAmgSettings::default()), .. }` (ADR 0305) |
| `cg` / `amg_quadratic` | `PreconditionedCg { SmoothedAggregation(ElasticAmgSettings::quadratic()), .. }` (ADR 0370) |
| `cg` / `amg_quadratic_smoothed` | `PreconditionedCg { SmoothedAggregation(ElasticAmgSettings::quadratic_smoothed()), .. }` (ADR 0375) |

`fill_level` and `ordering` given with another preconditioner are an error naming them. The default preconditioner of `cg` is
`ic_fill` with level 1 and natural ordering (ADR 0357's recommendation; taken on the maintainer's standing instruction of
2026-11-11, see the open points).

### D6. Outputs (clauses c3, c4)

- Static: the `.vtu` by `rcfd_post::solid_output::write_solid_vtu(path, model, result)` (a multi-material model is refused by
  the nodal average, ADR 0278, and the refusal is a diagnostic with the path `output.vtu`).
- Modal: a `.vtu` of the mesh with one point array `mode_<k>` (3 per node) per mode, by `rcfd_io::vtk::fe_vtu_string` with the
  cell blocks of `vtk_from_fem` (the same layout as `solid_vtu_string`); exposed as `solid_case::modal_vtu_string`.
- The JSON report (`serde_json`, pretty): `format` = `rcfd-solid-report/1`, `title`, `analysis`, `mesh` (`nodes`, `elements`),
  `unknowns`; static: `displacement` (`max_magnitude`, `max_magnitude_node`, componentwise `min` and `max`), `stress`
  (`von_mises_max` and its node from the nodal average of `solid_fields`, `element_von_mises_max` and its element from the
  element means), `reactions` (per constraint name: the sum over the nodes of its set of the reaction vectors; and `total`
  over all nodes), `solver` (`name`, `iterations`, `restarts`, `relative_residual`, `shift`, `breakdowns`); modal:
  `frequencies` (Hz), `eigenvalues`, `solver` (`name` = `subspace`, `iterations`, `shift`, `residuals`, `rigid_free`);
  `outputs` (the paths written). Floats are written by `serde_json` (shortest round trip; the workspace enables
  `float_roundtrip`, so the values read back have the bits written).

### D7. The entry functions (clause c2)

`rcfd_api::solid_case`: `schema()`, `SolidCase::parse_toml_in(text, base_dir)`, `SolidCase::load(path)`, `SolidCase::build()
-> SolidSetup { model, analysis }`, `run(&SolidSetup) -> SolidRun`, `SolidRun::report_json(..)`, and
`run_solid_case_file(path, out_dir: Option<&Path>) -> Result<SolidRun, Vec<Diagnostic>>`, which loads, builds, runs and writes
the `.vtu` and the report (into `out_dir` if given, else the case file's directory). No CLI subcommand and no MCP tool (each
changes a golden: the maintainer's step, ADR 0287 decision M3).

## Registered checks

All in `crates/rcfd-api/tests/sb_solid_case.rs` (the file named by ADR 0287), with the case files in
`crates/rcfd-api/tests/solid_cases/`, release profile, run once (`cargo test -p rcfd-api --release --test sb_solid_case --
--nocapture --test-threads=1`). Outputs go to `CARGO_TARGET_TMPDIR`.

### A. The acceptance

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| A1 | c1, c2, c3, c4 | `cantilever_tet10.toml` (generator box tet10 `40 x 2 x 2`, two box node sets, three constraints, traction, static, direct) through `run_solid_case_file`: the `.vtu` exists and equals `solid_vtu_string(model, result)` of the run byte for byte; the report parses, has every field of D6, and its `displacement.max_magnitude`, `stress.von_mises_max`, `solver.name`, `solver.iterations`, `unknowns` equal the values computed from the result in the test (bits) | as stated | by construction |
| A2 | c4 | the same report: `reactions.root_n0[2] + reactions.root_n1[2]` against `P`; `reactions.total` against `-(0, 0, -P)`, i.e. `abs(R_z total / P - 1)` and `abs(R_x total), abs(R_y total) / P` | each <= 1e-8 | arithmetic 4: about 1e-11 |
| A3 | c5 | an unknown key (`youngs = ..` in a material table; and an unknown top-level table `[flow]`): a diagnostic whose path is the key and whose line is the key's (resp. the header's) line, message containing `unknown setting` | as stated | D2 |
| A4 | c6 | a missing set: a constraint `set = "nowhere"`, a load `set = "nowhere"` (surface), a material `element_set = "nowhere"`: one diagnostic each, path the key, line the key's line, message naming `nowhere` | as stated | D2 |
| A5 | c7 | a wrong unit: `youngs_modulus = "200 m"`; `traction = [0, 0, "5 N"]`: path `materials.steel.youngs_modulus` with its line; path `loads.tip.traction.2` with the line of `traction` | as stated | D2 |
| A6 | c8 | a wrong type: `divisions = [40, 2.5, 2]`, `poisson_ratio = "abc"`, `components = "x"`: path and line of each, message naming the expected type | as stated | D2 |
| A7 | c9 | file runs against the same models by library calls: `cantilever_tet10.toml` (displacement and reactions), `bar_hex8.toml` (eigenvalues and modes), `gmsh_uniaxial.toml` (displacement), `cantilever_tet10_amg.toml` (CG with `amg_quadratic`; displacement): largest `|a - b| / max |b|` | each <= 1e-12; bit-identical expected and recorded | 0 |
| A8 | c10 | `cantilever_tet10.toml`: `abs(delta / delta_T - 1)`, `delta = sum_i f_i u_i / P` with `f = nodal_load_vector(model, &[])` (ADR 0433 C1) | <= 0.01 | 5.29e-4 (ADR 0433) |
| A9 | c10 | `cantilever_hex8.toml` (hex8 `80 x 4 x 4`, the same support and load): the same measure (ADR 0433 C2) | <= 0.05 | 3.21e-2 |
| A10 | c11 | `bar_hex8.toml` (hex8 `32 x 1 x 1`, modal, 4 modes): the first three frequencies are axial (kinetic fraction of x components `>= 0.99`, ADR 0284 B5) and `abs(f_n / f_n^exact - 1)`, n = 1, 2, 3 (B1); against the discrete closed form (B3); M-orthonormality of the modes | `<= 5e-3`; `<= 1e-9`; `<= 1e-9` | 1.004e-4, 9.038e-4, 2.512e-3; 2e-12; 1e-15 |
| A11 | c12 | `gmsh_uniaxial.toml` (file `tet_v4.1_ascii.msh` of `rcfd-io`, Tet4, arithmetic 1): largest `|u_h - u| / max |u|` over all nodes; largest `|sigma_e - s e_x e_x| / s` over the elements' means (all six components) | each <= 1e-10 | 1e-13 to 1e-12 |
| A12 | c12 | the same with `tet_v2.2_bin.msh` (another Gmsh format and encoding of the same box, other node numbering) | each <= 1e-10 | as A11 |
| A13 | c14 | the solver keys: for each row of the D5 table a file that differs only in `[solver]` parses to the `StaticOptions` listed (`PartialEq`), including `fill_level = 2, ordering = "node_rcm"`; `fill_level` with `amg` and `method = "cg"` with `type = "modal"` give a diagnostic naming the key | as stated | D5 |
| A14 | c14 | `cantilever_tet10.toml` with each of `block_jacobi`, `ic0`, `ic_fill`, `amg`, `amg_quadratic`, `amg_quadratic_smoothed` (tolerance 1e-10): report `relative_residual <= 1e-10`; displacement against the direct run, largest `|u - u_d| / max |u_d|` (F-381's limit) | `<= 1e-10`; `<= 1e-7` | residual just below 1e-10; 1e-12 to 1e-9 (F-381: 3.3e-13) |

### K. The set-ups themselves

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| K1 | the box node sets of the cantilever files hold exactly one node each, at `(0, 0, h/2)` and `(0, h, h/2)`; the Gmsh corner sets one node each at `(0, 0, 0)` and `(0, 1, 0)` | as stated | D3 |
| K2 | the Gmsh model: 351 nodes, 1139 Tet4, surface sets `inlet`, `outlet`, `walls` with areas 1, 1, 8 within 1e-12 | as stated | `gmsh_samples` |
| K3 | the modal `.vtu` equals `modal_vtu_string(model, modal)` byte for byte and contains 4 arrays `mode_1` .. `mode_4`; the report's `frequencies` equal the result's (bits) | as stated | by construction |

### N. Negative controls

| Id | Control | Expected |
| --- | --- | --- |
| N1 | `cantilever_tet10.toml` with `poisson_ratio = 0.3000000001` against the library model with 0.3: A7's measure | above 0 (the comparison can fail), below 1e-6 |
| N2 | a solid case text through the flow reader `Case::parse_toml`; the flow case `cases/cavity/adapt_re100.toml` through `SolidCase::parse_toml_in` | both `Err` with unknown-key diagnostics (the schemas are separate) |
| N3 | a TOML syntax error (an unclosed string on line 3) | one diagnostic with line 3 |
| N4 | a node-set box that selects no node | a diagnostic naming the set's `box_min` path with its line |

### S. Defaults (clause c13)

| Id | Measure | Limit |
| --- | --- | --- |
| S1 | `git diff 52ce5ea6 --stat -- crates` lists only `crates/rcfd-api/src/lib.rs` (one added line), the new `crates/rcfd-api/src/solid_case.rs`, the new test and the new `tests/solid_cases/` files | as stated |
| S2 | unedited existing tests pass: `rcfd-api` `baseline_hashes`, `case_hashes_w48u`, `w48v_case_hashes`, `physics_schema`, `rules_w19c`; `rcfd-cli` `cli`; `rcfd-mcp` `default_f1`, `tool_list_f275`, `project_list_f270` (the K0 list of ADR 0257 plus the tool list and the scan tests) | all pass, no file edited |

### Mutations (scratch copy `w49i-work/f386-scratch/tree` from `git archive`, not committed)

| Id | Mutation (in `solid_case.rs`) | Expected to be caught by |
| --- | --- | --- |
| MU1 | `ordering` ignored (always `IcOrdering::Natural`) | A13 |
| MU2 | the line of every diagnostic left `None` | A3 to A6 |
| MU3 | the dimension of the traction items `FORCE` instead of `PRESSURE` | A5 (the `"5 N"` is accepted) |
| MU4 | the node-set box exclusive (`<` instead of `<=`) on the upper bound | K1 (and A8: a set with no node is an error) |
| MU5 | component `y` mapped to index 2 | A7 is not affected (the library model is built independently in the test), A8 (the support changes) |
| MU6 | the report's reaction per constraint summed over all nodes | A2 |
| MU7 | the report's `frequencies` written as the eigenvalues | K3 |
| MU8 | the Gmsh path resolved against the working directory instead of the case file's directory | A11 (file not found) |
| MU9 | the traction vector's components 0 and 2 swapped | A7 (cantilever) and A8, A11 |

## Decision rule

- **done**: every A and K check met on the single run, every negative control as expected, S1 and S2 as stated, every mutation
  caught by at least one check, no amendment made after a run.
- **partial**: any A or K check missed (kept as a `*_known_miss` record with the limit intact and the measured number, the cause
  diagnosed), a mutation not caught, or a check met only after an amendment made after its run.
- **todo**: the checks could not be run.

## Limits (what is not covered)

- One generator (the box) and one file format (Gmsh, through `rcfd-io`; the other mesh readers of `rcfd-io` produce the same
  `rcfd_mesh::Mesh` and could be added by a key, not done here). No hexahedral-to-tetrahedral split, no curved mid-edge
  projection, no Hex20, beams, shells, MPCs, rigid sets, springs, temperature, centrifugal loads, harmonic or transient analyses
  in the file: the basic-tier set of ADR 0287 only.
- Node sets by a box only (no surface sets by a box, no set algebra).
- No CLI subcommand and no MCP tool (the maintainer's step: each changes a golden). No per-material output (ADR 0278's open
  question).
- The modal `.vtu` carries the mode shapes only (no modal stresses).

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The default preconditioner of `method = "cg"` is ADR 0357's recommended `ic_fill` level 1, natural ordering (ADR 0305's
   aggregation needs fewer iterations on large models but is a choice by key; the default direct solve is unchanged).
2. Named tables (`[constraints.<name>]`) rather than arrays of tables, so that every key has a line `locate_line` can find and
   every reaction in the report has a name.
3. Node sets by an inclusive box without an implicit tolerance (the file states the box), rather than by nearest node.
4. Tables are processed in name order (the order of the JSON object of the parsed TOML), stated in the format document; a user
   who needs a given order of loads names them accordingly. The order does not change the result of a linear solve except in
   rounding of the load sum.

## Amendment 1 (2026-11-14, after the registered run, before the corrected run): A14's loop stopped at its first failure

The registered run (log `f386-scratch/run1.log`, run once) ran every test. All checks of A1 to A13, K1 to K3 and N1 to N4 were
met. `a14_a7_cg_preconditioners_from_files` panicked at its first row: the file run with `preconditioner = "block_jacobi"`
returned the diagnostic `solver cg-block-jacobi: not converged in 5063 iterations and 3 restart(s) (true residual 1.528e-7,
target 2.635e-8 = 1e-10 relative)`, i.e. a relative residual of 5.8e-10 against the limit 1e-10. That is a miss of A14 for block
Jacobi (the solve stopped by the restart cap of ADR 0282 D5 at the attainable true residual of a 5063-iteration CG; diagnosis
in the addendum). The set-up was defective: a failed solve aborted the loop, so the five other preconditioners of A14 and the
iterative A7 row after the loop were not run.

Second finding, from the registered A7 rows of the static cantilever (met, 4.9e-15 and 2.2e-15 against 1e-12, but not
bit-identical as expected): the test's library model applies the traction `-C_P / (C_H * C_H)`, which is
`-99999.99999999999` (`-0x1.869ffffffffffp+16`, because `0.1 * 0.1 = 0.010000000000000002`), while the file gives `-1.0e5`
(`-0x1.86a0000000000p+16`): the two models differ by one ulp in the load, so the reference of the bit expectation was not "the
same model". Checked by the disclosed python line `-1000.0/(0.1*0.1)`; no library code run for it. The registered A7 result
stands as measured (met).

Corrections, written and committed before their run:

- The registered test stays as an ignored record (`#[ignore]`, citing this amendment). Its block-Jacobi row becomes
  `a14_block_jacobi_known_miss` (ignored), with the limit 1e-10 intact and the measured 5.8e-10 (not converged) recorded.
- `a14c_a7c_cg_preconditioners_from_files`: the rows `ic0`, `ic_fill`, `amg`, `amg_quadratic`, `amg_quadratic_smoothed` of
  A14 with the registered limits (relative residual <= 1e-10, against the direct solve <= 1e-7), a failed solve recorded as a
  miss of its row instead of a panic; then A7 for `cantilever_tet10_amg.toml` (limit 1e-12) against a library model whose
  traction is the file's `-1.0e5`; bits recorded, expected identical.
- `a7b_cantilever_tet10_bits_with_the_same_traction` (new check A7b): `cantilever_tet10.toml` against the library model with
  the traction `-1.0e5`: displacement and reactions bit-identical (`to_bits` of every value), and the A7 measure <= 1e-12.

By the registered rule the item is partial whatever the corrected run gives (A14 missed for block Jacobi; A14 for the other
preconditioners is met, if it is, only after this amendment).

## Amendment 2 (2026-11-14, after the run of amendment 1, before the corrected run): the 1e-10 tolerance is below this model's residual floor

The run of amendment 1 (log `f386-scratch/run2.log`, run once): A7b met (displacement and reactions bit-identical, 0 against
1e-12, all 6075 values). A14c: every row stopped with "not converged ... and 3 restart(s)" at a true residual of 1.49e-7 to
1.65e-7 against the target 2.635e-8 (`1e-10 |b|`, `|b| = 263.5`): `ic0` after 1849 iterations, `ic_fill` 803, `amg` 71,
`amg_quadratic` 54, `amg_quadratic_smoothed` 37 (block Jacobi, run 1: 5063 iterations, 1.53e-7). A7c was not reached with a
result: `cantilever_tet10_amg.toml` (tolerance 1e-10) failed the same way.

Diagnosis (by these numbers, no further run): five preconditioners whose iteration counts differ by a factor of 140 all stop at
the same true residual, 5.7e-10 to 6.2e-10 of `|b|`; that is a floor of the residual `b - K u` evaluated in floating point, not
a convergence failure. Its size: the internal nodal forces of this model are much larger than the load (`|b|` is the tip load
`P = 1000 N` spread over the end face, while the bending stresses near the root, `sigma ~ 1.2e7 Pa` on faces of `~2.5e-3 m^2`,
give nodal forces of order 3e4 N; with 6075 equations, `eps x 3e4 x sqrt(6075) ~ 5e-10 x |b|`). F-381 met 1e-10 on Hex8
cantilevers whose load is a larger share of the internal forces; this registration's estimate ("just below 1e-10") did not
check that ratio. The registered limit 1e-10 is kept: every A14 row is a miss.

Corrections, written and committed before their run:

- `a14c_a7c_cg_preconditioners_from_files` stays as an ignored record; its A14 rows become `a14_cg_rows_known_miss` (ignored,
  limits intact, measured numbers above).
- A7d (the iterative row of c9, whose purpose is the bit comparison, not the tolerance): `cantilever_tet10_amg_1e8.toml`, the
  same file with `tolerance = 1e-8`, against the library model of A7b with the same `StaticSolver`: largest relative difference
  <= 1e-12, bits recorded (expected identical).
- Unregistered diagnostic printed in the same test (not a check): the relative residual and the difference from the direct
  solve of that run.

By the registered rule the item is partial (A14 missed; c9's iterative row met, if it is, only after this amendment).
