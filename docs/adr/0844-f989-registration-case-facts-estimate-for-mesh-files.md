# ADR 0844: F-989 registration, `case.facts` refuses by an estimate for mesh files and the generators that have none

REGISTERED 2026-11-28, written before any code of F-989 and before any run of its checks.

Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead. It answers the brief
`docs/adr/briefs/f989-facts-estimate-mesh-files.md` ([the brief](briefs/f989-facts-estimate-mesh-files.md)) and follows
`docs/process/agent-rules.md`, version 5 ([the rules](../process/agent-rules.md)). Branch `f989-facts-estimate-mesh-files`
at the brief's commit 8e5e7bb0 (equal to `origin/main`). Scratch folder `E:/projects/cfd/w49i-work/f989-facts-estimate-mesh-files-scratch`.

## The item

F-989's acceptance (read from `docs/roadmap.json` by a script): "Every limit not stated here is registered in the ADR before
the run. mesh_preview::estimate gives the cell count of a mesh file (each format the engine imports) and of every generator
that has no estimate, read without building the mesh; each count equals the built count over a registered set of cases,
checked by two routes; case.facts with a limit refuses by the estimate for each, within the time and memory limits of ADR
0812, with the reply of FactsError::TooLarge."

## What was read

AGENTS.md, the rules and the brief, in full (twice: again after a summary of the conversation).
[ADR 0812](0812-f823-registration-case-facts-refuses-by-the-counts-estimate.md) in full (E2's and E3's limits, amendments 1
and 2, question 1). `crates/rcfd-api/src/mesh_preview.rs` in full (`estimate` lines 101 to 127, its use in `preview` lines
216 to 229). Of `crates/rcfd-api/src/facts.rs`: `case_facts` lines 367 to 429 (the 0812 check at lines 388 to 392, the
build at 393). Of `crates/rcfd-api/src/sim.rs`: `build_untransformed_mesh` lines 694 to 941 (the file branch lines 696 to
716, the generator dispatch 724 to 940), `cylindrical_interface_blocks` 523 to 593, `sector_interface_pair` 611 to 630,
`sector_interface_mesh` 655 to 666. The readers: `crates/rcfd-io/src/lib.rs` `read_msh_with_notes` lines 36 to 54;
`crates/rcfd-io/src/gmsh.rs` (Reader lines 20 to 113, `nodes_per_type` 116 to 138, `classify` 148 to 158, `read_bytes`
183 to 257, `read_elements_v2` 338 to 390, `read_elements_v4` 392 to 422, `assemble` 443 to 470);
`crates/rcfd-io/src/fluent.rs` (header lines 1 to 24, `split_sections` 197 to 262, cell sections 353 to 366 and 507 to
512, `n_cells` from the faces 678 to 684, the 2D extrusion 760 to 800); `crates/rcfd-io/src/openfoam.rs` (binary refused
lines 113 to 145, `read_dir` 245 to 296, `write_dir` 332 to 372). The generators: `rcfd_mesh::gen::cylinder_ogrid`
(`crates/rcfd-mesh/src/gen.rs` 546 to 580), `sweep::pipe_mesh` and `disc_base` (`crates/rcfd-mesh/src/sweep.rs` 215 to
275, 520 to 549), `layering::Section::rect` and `rect_triangles` (`crates/rcfd-mesh/src/layering.rs` 169 to 197),
`dynmesh::extruded_stack` (`crates/rcfd-api/src/dynmesh.rs` 51 to 84), `fan_case::spec_of`, `cgrid_spec_of`, `build` and the
schema counts (`crates/rcfd-api/src/fan_case.rs` 54 to 77, 228 to 317), `build_sector_mesh`
(`crates/rcfd-mesh/src/fan_mesh.rs` 389 to 452, 600 to 640), the C-grid `Layout` and its cells
(`crates/rcfd-mesh/src/fan_cgrid.rs` 66 to 100, 2005 to 2275). The tests `case_facts_estimate_adr0812.rs` (helpers, `e6b_`
lines 311 to 330, which asserts `estimate(&file) == None`) and `mesh_preview_adr0732.rs` lines 140 to 165. The record of the
Fluent layout in [reference-data.md](../reference-data.md), "Fluent mesh format details (F-118)". The
[tolerance table](../process/tolerances.md). About 170,000 bytes.

The Gmsh reference manual (gmsh.info, section 10.1 "MSH file format" and 10.3.1 "MSH file format version 2 (Legacy)" by
its table of contents) was fetched, but the fetch returned the page cut off before those sections, so it was not read. The
layout each estimate reads is therefore the one the engine's own reader reads (the files and lines above): the estimate
must equal what that reader builds, so the reader is the definition the count follows. No vendor document was used.

## The formats and the generators without an estimate

A case with `mesh.source = "file"` reaches three readers (`sim.rs` lines 696 to 716): a directory is an OpenFOAM `polyMesh`
(`rcfd_io::openfoam::read_dir`, ASCII only); a file goes to `read_msh_with_notes`, which sends a file whose first
non-blank byte is `(` (or gzip or HDF5 magic, both refused) to the Fluent reader (ASCII and binary sections) and anything
else to the Gmsh reader (versions 2.x and 4.1, ASCII and binary). The generators `estimate` does not name, from the
dispatch: `annulus_3d_sector_cylindrical_interface` (line 863), `pipe_sweep` (891), `axial_fan_sector` (919), `box_3d`
(922, with its extruded `section` variants) and `cylinder_ogrid` (931). No other generator exists.

## Design

A new public function `mesh_preview::cell_estimate(case) -> Option<u64>`: for a mesh file, the count below; for a
generator, `estimate(case)` where that gives `Some`, else the counts of the five generators below. `None` wherever the
estimate cannot be sure of the reader's count (an unreadable file, a format the reader refuses, an unknown generator):
then `case_facts` builds as before, so its reply is the build's. `case_facts` calls `cell_estimate` in place of
`estimate` in the existing check (facts.rs lines 388 to 392); nothing else changes there.

`estimate` itself and `mesh.preview` are unchanged. The acceptance names `mesh_preview::estimate`; giving that function a
file's count would make ADR 0812's existing test `e6b_` fail (it asserts `estimate(&file) == None`), and existing tests are
not edited (rule 2); it would also change `mesh.preview`'s reply for a large file (its refusal text names the `mesh.n*`
counts). The count is therefore in `mesh_preview` under a new name. Taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation); question 1 below.

### Mesh files: what is read, without building

The path is resolved as the build resolves it (absolute, or joined to `case.base_dir`). Every file is read as a stream
through a buffer of 1 MiB (never whole into memory), so the memory is the buffer's, not the file's.

- **Gmsh** (`gmsh.rs`): the tokens of `$MeshFormat` (version, file type, data size; in binary the 4-byte endianness
  integer); a version outside 2.x and 4.05 to 4.5 gives `None`, as the reader refuses it. Every other section up to
  `$Elements` is skipped to its `$End<name>` marker, as the reader skips it. In `$Elements` the count is the number of
  elements whose type is a cell type of `classify` (4, 11 tetrahedra; 5, 12, 17 hexahedra; 6, 13, 18 prisms; 7, 14, 19
  pyramids), each of which `assemble` makes one cell (lines 453 to 465). Version 2 ASCII: per element the tokens id, type,
  number of tags, then the tags and `nodes_per_type(type)` nodes are skipped. Version 2 binary: per block the three 4-byte
  integers type, count, number of tags, then `count × (1 + tags + nodes) × 4` bytes skipped. Version 4.1: the four sizes of
  the header, then per block dim, entity, type (4-byte integers in binary) and the count (8 bytes in binary), then
  `count × (1 + nodes)` values skipped (8 bytes each in binary). An unknown element type gives `None` (the reader errors).
- **Fluent** (`fluent.rs`): the sections are walked as `split_sections` walks them (a balanced, string-aware parenthesis
  group for a text section; for an index of 2000 or more, the header group and then either `)` or a payload ended by
  `End of Binary Section`). The count is the largest `last` of the cell sections 12, 2012 and 3012 whose zone is not 0 and
  whose type is not 0, which are the reader's `cell_zones` (lines 353 to 366, 507 to 512). The reader numbers its cells by
  the largest cell index of the faces (lines 678 to 684); in a consistent file these are the same cells, and the set below
  holds only consistent files. No cell section gives `None`. A 2D file has the same count (one layer of prisms per cell).
- **OpenFOAM** (`openfoam.rs`): the label lists `owner` and `neighbour`, each after its `FoamFile { }` header; the count is
  the largest label plus 1, which is the number of cells the owner/neighbour addressing defines (the reader passes these
  lists to `Mesh::from_face_lists`). A header with `format binary;` gives `None` (the reader refuses it).

### The five generators: from their parameters (first principles, from the code read)

With `nx, ny` the case's counts, `nz` its `mesh.nz` and the right counts as `estimate` takes them (0 meaning the left):

- `cylinder_ogrid`: one cell per `(it, ir)` for `ir < ny`, `it < nx` (`gen.rs` 570 to 571): `nx · ny`.
- `pipe_sweep`: `disc_base(n = nx, n_ring = ny)` has `n²` core quads and `4 · n · n_ring` ring quads (`sweep.rs` 263 to
  275), swept through `nz` layers (`mesh.nz` as given, not raised to 1, since the build refuses 0): `(nx² + 4 · nx · ny) · nz`.
- `box_3d`: without a section, `box_mesh([nx, ny, max(nz, 1)])`: `nx · ny · max(nz, 1)`. With `section = "rectangles"`,
  `nx` layers of `Section::rect([max(ny, 1), max(nz, 1)])`, one quad per `(j, k)`: `nx · max(ny,1) · max(nz,1)`; with
  `"triangles"` two triangles per `(j, k)` (`layering.rs` 188 to 196): `2 · nx · max(ny,1) · max(nz,1)`.
- `annulus_3d_sector_cylindrical_interface`: two sectors `nx × ny × nz` and `nx_right × ny_right × nz_right`
  (`sim.rs` 586 to 588), joined periodically with every cell kept: `nx·ny·nz + nxr·nyr·nzr` (`nz` at least 1).
- `axial_fan_sector`, H-grid (`fan_mesh.rs` 444 to 448, 602 to 610): `n_i = n_up + n_chord + n_down`,
  `n_j = 2 · n_side + n_mid`, `n_k = n_span + n_gap`; the cells are `n_i · n_j · n_k` less, with `include_blade`, the
  blade's `n_chord · n_mid · n_span`.
- `axial_fan_sector`, `topology = "c_grid"` (`fan_cgrid.rs` 76 to 88, 2200 to 2270): in each of the `n_k = n_span + n_gap`
  layers `2 · (n_down + n_chord) · n_side` C-block cells and `2 · n_nose · n_up` F-block cells; and `n_core · n_chord`
  core cells in the layers `k ≥ k0`, `k0 = n_span` with `include_blade`, else 0.

The `mesh.fan.*` counts are read as `fan_case::spec_of` reads them. The C-grid's construction options (`smoothed`,
`camber_seam`, gradings) move nodes only in the code read. A transform or `mesh.scale` moves points only.

## Arithmetic done beforehand

A second reader written in Python (scratch `count2.py`, not committed; it reads each file whole and parses it, a different
route from the streaming estimate) gave for the repository's mesh files: `crates/rcfd-io/tests/data/hex_v2.2_ascii.msh`,
`hex_v2.2_bin.msh`, `hex_v4.1_ascii.msh`, `hex_v4.1_bin.msh`: 64 each; the four `prism_*` files: 336 each; the four `tet_*`
files: 1139 each; `example-projects/wind-tunnel/meshes/naca0012-ogrid` and its copy under `example-projects-linux/`: 6144
each (largest label plus 1, equal to the `nCells:6144` of the `owner` header's note). No Fluent file is in the
repository.

## The registered set (each with its count, fixed now)

Files (route B is the count named):

| Id | Case | Count | Route B |
|---|---|---|---|
| R1 to R12 | the 12 Gmsh files of `crates/rcfd-io/tests/data/` | hex 64, prism 336, tet 1139 | the Python reader above |
| R13, R14 | the two `naca0012-ogrid` polyMesh folders | 6144 | the Python reader; the header's `nCells` |
| W1 | Fluent ASCII 3D, written by the test from `gen::box_mesh([3, 4, 5])` (nodes 10, cells 12, faces 13 by zone: interior, then the six patches as walls) | 60 | the count of the mesh it was written from |
| W2 | W1's mesh as a Fluent file of binary sections (3010, 3012, 3013) | 60 | as W1 |
| W3 | Fluent ASCII 2D, written by the test: a 3 × 2 grid of quads, two-node faces | 6 | the number of quads written |

Generators (route B is the formula above, worked here by hand). Small cases: 0812's `gen_case` (`nx 8`, `ny 4`) with:

| Id | Generator, settings | Count |
|---|---|---|
| G1 | `cylinder_ogrid`, r_in 1, r_out 4 | 8·4 = 32 |
| G2 | `pipe_sweep`, r_out 1, nz 2 | (64 + 128)·2 = 384 |
| G3 | `box_3d`, nz 2 | 64 |
| G4 | `box_3d`, nz 2, `mesh.box_3d.section = "rectangles"` | 8·4·2 = 64 |
| G5 | `box_3d`, nz 2, `section = "triangles"` | 128 |
| G6 | `annulus_3d_sector_cylindrical_interface`, r_in 1, r_out 3, r_interface 2, nz 2, sector_angle 30 | 64 + 64 = 128 |
| G7 | G6 with ny_right 3 | 64 + 48 = 112 |
| G8 | `cases/axial_fan_sector/axial_fan_sector.toml` (H-grid: 8, 12, 8; side 10, mid 1; span 16, gap 7) | 28·21·23 − 12·1·16 = 13,332 |
| G9 | G8 with `include_blade = false` | 13,524 |
| G10 | `cases/axial_fan_cgrid/axial_fan_cgrid.toml` (8, 16, 8; side 10, nose 4, core 1; span 14, gap 7) | 21·(480 + 64) + 7·16 = 11,536 |
| G11 | G10 with `include_blade = false` | 11,424 + 21·16 = 11,760 |

Over the limit (the default service limit 2,000,000; heavy). Files are written first, by an ignored test, into
`$RCFD_F989_DIR` (here the scratch folder):

| Id | Case | Count |
|---|---|---|
| H1 | Gmsh 2.2 ASCII, a 128³ hexahedral grid written by the test (type 5, with the six sides as quads) | 2,097,152 |
| H2 | the same grid as Gmsh 4.1 binary | 2,097,152 |
| H3 | Fluent ASCII from `gen::box_mesh([128, 128, 128])` | 2,097,152 |
| H4 | OpenFOAM ASCII, `rcfd_io::openfoam::write_dir` of the same mesh | 2,097,152 |
| H5 | `cylinder_ogrid` nx 2000, ny 1010, r_in 1, r_out 4 | 2,020,000 |
| H6 | `pipe_sweep` nx 40, ny 20, nz 420, r_out 1 | (1600 + 3200)·420 = 2,016,000 |
| H7 | `box_3d` 130 × 130 × 120 | 2,028,000 |
| H8 | `box_3d` rectangles, nx 200, ny 100, nz 101 | 2,020,000 |
| H9 | `annulus_3d_sector_cylindrical_interface` nx 100, ny 50, nz 202, G6's radii and angle | 2·1,010,000 = 2,020,000 |
| H10 | G8 with n_up 40, n_chord 60, n_down 40, n_side 50, n_mid 5, n_span 120, n_gap 20 | 140·105·140 − 60·5·120 = 2,022,000 |
| H11 | G10 with n_up 40, n_chord 60, n_down 40, n_side 70, n_nose 10, n_core 4, n_span 120, n_gap 20 | 140·14,800 + 20·240 = 2,076,800 |

## Tests (fixed now)

New file `crates/rcfd-api/tests/case_facts_estimate_files_adr0844.rs` (header names ADR 0844 and F-989, rule 12).

- `a1_files_by_two_routes`: for R1 to R14 and W1 to W3 (W written into a temporary folder of the test): `cell_estimate`
  equals the registered count (route B) and `sim::build_mesh(case).n_cells()` (route A).
- `a2_generators_by_two_routes`: the same for G1 to G11.
- `a3_at_and_below_the_built_count`: for every case of a1 and a2 with `n` its built count:
  `case_facts(case, NotDefault, Some(n))` is not refused (`Ok`, `cells == n`, or for a transient case whatever error the
  build gave before, never `TooLarge`) and `Some(n − 1)` gives `TooLarge { cells: n, limit: n − 1 }`.
- `a4_the_service_reply`: a service with `max_cells = 20` and a session of each of G1 to G11: `case.facts` answers code
  1004 with the message `mesh has N cells, limit is 20`, N the built count.
- `n1_refused_before_a_build` (negative control): a Gmsh 2.2 ASCII file with W1's 60 hexahedra and no `$Nodes`, and a
  polyMesh folder with only R13's `owner` and `neighbour`: `case_facts(.., Some(10))` gives `TooLarge { cells: 60 or
  6144, limit: 10 }`, and `case_facts(.., None)` gives `Invalid` (the build fails): the refusal came before any build.
- `n2_none_where_the_reader_refuses` (negative control): `cell_estimate` is `None` for a polyMesh whose `owner` says
  `format binary;`, a gzip-magic file, a Gmsh file of version 4.0, and a missing path; and for each,
  `case_facts(.., Some(10))` gives the same `Invalid` diagnostics as `case_facts(.., None)`.
- Heavy, ignored (each run once by the script below, rule 7): `p0_write_heavy_files` (H1 to H4); for each `Hk`,
  `rk_` (the refusal alone: `case_facts(.., Some(2_000_000))` timed, T_r, must be `TooLarge { cells: count, limit:
  2_000_000 }`) and `hk_` (the same refusal timed, then `sim::build_mesh` timed, T_b, whose `n_cells` must equal the
  count; asserts T_r ≤ 1 s and T_r / T_b ≤ 0.5; prints both).

## Table of checks

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| K1 | "gives the cell count of a mesh file (each format) ... equals the built count ... by two routes" | `a1_` | equal integers, both routes, all 17 cases | the acceptance; tolerance table row "Same discrete problem by two routes" (exact: integers) | equal | exact | code verification |
| K2 | "... and of every generator that has no estimate" | `a2_` | equal integers, both routes, all 11 | as K1 | equal | exact | code verification |
| K3 | "case.facts with a limit refuses by the estimate ... with the reply of FactsError::TooLarge" | `a3_`, `a4_`, `n1_` | as stated in the tests | the acceptance | passes | exact | code verification |
| K4 | "within the time limits of ADR 0812" | `hk_` for H1 to H11 | T_r ≤ 1 s and T_r / T_b ≤ 0.5 for each; the count and the built count equal | ADR 0812 E2; tolerance table row "Run time or memory, as a budget" | T_r 0.4 s for H3 (the largest file, about 360 MB, scanned at about 1 GB/s), under 0.3 s for the other files, under 1 ms for H5 to H11; T_b 20 to 60 s | 2.5 (H3); ratio 0.02 or less | unchanged default output (the row's kind) |
| K5 | "... and memory limits of ADR 0812" | peak working set of `rk_` alone (P_r) and of `hk_` alone (P_b) for each Hk, by the sampler below | P_r ≤ 200 MB and P_r / P_b ≤ 0.5 for each | ADR 0812 E3; the same row | P_r 10 MB; P_b 2 to 6 GB | 20; 0.005 | unchanged default output |
| K6 | the reader's refusals unchanged | `n2_` | as stated | rule 3 | passes | exact | unchanged default output |
| K7 | existing replies unchanged | the named existing files `case_facts_adr0737`, `mesh_preview_adr0732`, `case_facts_estimate_adr0812` | each passes as on main | rule 3; tolerance table row "Bit-identity of default output" | passes | exact | unchanged default output |
| BH | rule 3 | `cargo test -p rcfd-api --release --test baseline_hashes` | passes as on main | rule 3 | passes | exact | unchanged default output |
| R7 | rule 7 | `cargo fmt -p rcfd-api --check` (or `rustfmt --check` of the touched files on os error 206); `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-api --release --lib`; the new file | each exit 0 | rule 7 | passes | exact | code verification |
| DS | rule 7 | `cargo test -p rcfd-cli --release --test docs_site` | passes | rule 7 | passes | exact | code verification |
| P1 | rules 7, 9 | `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0844-F-989.json`; `python scripts/process_report.py --check` | valid; clean but for a generated file only not current | rules 7, 9 | passes | exact | code verification |
| A1 | files owned | `git diff --stat 8e5e7bb0..HEAD` | `mesh_preview.rs`, `facts.rs`, the new test file and its patch, this ADR, the proposal file | the brief | as the limit | exact | code verification |

### Set-up of the runs (fixed here)

Every cargo command with `CARGO_TARGET_DIR=E:\projects\cfd\w49i-work\f989-facts-estimate-mesh-files-scratch\target` and
`CARGO_BUILD_JOBS=8`, release profile. The host CPU (mean `LoadPercentage` of `Win32_Processor`) is recorded before each
timed run. The heavy runs, after R7: `p0_` once, then for k = 1 to 11 in order, `rk_` and then `hk_`, each as its own
process of the test executable (`--exact <name> --ignored --test-threads=1`) under the sampler of ADR 0812 amendment 2
(the process handle held, `PeakWorkingSetSize` read once from `GetProcessMemoryInfo` after exit; scratch
`sample-peak.ps1`), with the CPU before each. A reference build that fails (an `hk_` whose `build_mesh` errors) is
recorded as not deciding K4 and K5 for that case, not as met.

## Negative controls and planned mutations

The negative controls are `n1_` and `n2_`. Mutations, each once on a scratch copy of the code commit, by `cargo test
--release -p rcfd-api --test case_facts_estimate_files_adr0844` (the fast tests):

| Id | Mutation | Caught when |
|---|---|---|
| MU1 | Gmsh: the surface types (2, 3, 9, 10, 16) counted as cells | `a1_` fails |
| MU2 | `pipe_sweep` without its `4 · nx · ny` ring term | `a2_` fails |
| MU3 | OpenFOAM: the largest label without the `+ 1` | `a1_` fails |
| MU4 | committed: `case_facts` calls `estimate` in place of `cell_estimate` (the code before this item) | `n1_` fails (`Invalid` in place of `TooLarge`) |
| MU5 | C-grid: the core cells in every layer, `k0` ignored | `a2_` fails (G10) |
| MU6 | Gmsh 4.1 binary: the element count of a block read as 4 bytes | `a1_` fails |
| MU7 | Fluent: the binary cell sections 2012/3012 not counted | `a1_` fails (W2) |

MU4 is committed as `crates/rcfd-api/tests/case_facts_estimate_files_adr0844.mutation.patch`, its first line
`# expect-fail: cargo test --release -p rcfd-api --target-dir target/mutation --test case_facts_estimate_files_adr0844 n1_`.

## Size

Size (agent): done; 0; 1; 1; 170000

(The heavy run: the 23 processes of H1 to H11.)

## Decision rule

**Done** for F-989 if K1 and K2 hold by both routes for every case, K3 to K7, BH, R7, DS, P1 and A1 hold, K4 and K5 hold
for each of H1 to H11, and MU1 to MU7 are each caught; **partial** otherwise, with every miss or case not decided recorded.

## What is not covered

`mesh.preview` keeps its own estimate (question 1). A Fluent file whose cell sections disagree with its faces (the
estimate follows the cell sections, the reader the faces). Fluent `.cas` files with solver sections other than those the
walk skips as text, polyhedral Gmsh elements beyond the types the reader knows, and OpenFOAM's compact list form `N{v}` give
what the code gives; no case of the set has them. `[mesh.adapt]` and `[mesh.sliding]`: `case_facts` counts the mesh of
`build_mesh`, which is what `cell_estimate` counts. The time on other hosts.

## Open questions for the maintainer

- **Q1 (scope, the function's name).** The acceptance names `mesh_preview::estimate`; the count is in
  `mesh_preview::cell_estimate` because ADR 0812's test `e6b_` asserts `estimate(&file) == None` and `mesh.preview`'s
  refusal is worded for the `mesh.n*` counts. Options: (a) keep two functions (as registered); (b) a later item merges them,
  with the maintainer's word to amend `e6b_`'s assertion and `mesh.preview`'s reply for large files; (c) `mesh.preview`
  refuses by `cell_estimate` too, under a new reply text. Recommendation: (a) now, (b) if the Studio's preview meets large
  files.

## Amendment 1 (2026-11-28): `a3_`'s set-up is defective; a corrected test `a3b_`; the test file's two clippy lints; fmt by rustfmt

Written and committed after the registered run of R7 (code commit 6fd86d7b, patch e978d684; CPU 60 % before the new file's
run) and before the corrected runs. Nothing above is changed.

1. `a3_at_and_below_the_built_count` failed in its set-up, before any check of its limit: it and `a1_` run in parallel in
   one process and write their files into the same temporary folder (`rcfd-adr0844-<pid>-files`), which `a1_` deletes at
   its end; `a3_`'s build of W1 then found no file (`io error: The system cannot find the path specified. (os error 3)`,
   at the test file's line 63). K3 is not decided by that run. Correction: a new test `a3b_at_and_below_the_built_count`,
   the same as `a3_` with a folder of its own (`files_in("a3b")`). `a3_` stays in the file as an ignored record. Its
   limit is K3's, unchanged. The corrected run is `a3b_` alone, once.
2. `cargo clippy -p rcfd-api --all-targets -- -D warnings` failed on two lints in the new test file, not in the library:
   `write!` with a format string ending in a newline (the 2D Fluent writer, line 171) and a field assigned after
   `ServiceConfig::default()` (`a4_`, lines 461 to 462). Correction: `writeln!` and a struct literal with
   `..ServiceConfig::default()`; they change neither a file written nor a check. Clippy is run again once after this
   correction.
3. `cargo fmt -p rcfd-api --check` failed with os error 206 (the file name or extension is too long), before checking any
   file. As the lead's instruction for that case says, `rustfmt --check` (edition 2021, the repository's `rustfmt.toml`)
   of the three touched files is run in its place, once.

## Amendment 2 (2026-11-28): H11's reference build fails; a corrected case H11b

Written and committed after the heavy runs of H1 to H11 (at 56468503) and before the corrected run. Nothing above is
changed.

H11's refusal held (`r11_`: T_r 0.000568 s, P_r 7,307,264 bytes, CPU 70 % before), but in `h11_` the reference build
failed (CPU 66 % before): `mesh.fan.front_gap`: "the line family is folded at layer 0 (r = 0.10000), C block around line
110, row 34: the leading-edge fan cannot be built for this blade with the front_gap 0.3 and n_nose 10 given". As
registered, K4 and K5 are not decided for H11 by that run. The cause is the set-up: `n_nose = 10` was raised from the
shipped case's 4 without checking that the C-grid's leading-edge fan builds, which is a matter of geometry, not of the
count. Correction: H11b, H11 with the shipped case's `n_nose = 4` and everything else as H11: per layer
`2·(40 + 60)·70 + 2·4·40 = 14,320`, times 140 layers, plus `4·60` core cells in the 20 gap layers:
`2,004,800 + 4,800 = 2,009,600` cells (over 2,000,000). New ignored tests named `r11b_` and
`h11b_`, the same as `r11_` and `h11_` for H11b; `r11_` and `h11_` stay as they are, as the record of this run. Their
limits are K4's and K5's, unchanged. The corrected runs are `r11b_` and then `h11b_`, each once, under the sampler. If
H11b's build also fails, K4 and K5 stay not decided for the C-grid, and the decision is partial.
