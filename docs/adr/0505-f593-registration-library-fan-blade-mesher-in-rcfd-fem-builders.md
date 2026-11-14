# ADR 0505: F-593 registration: the library fan-blade mesher in rcfd_fem::builders (library half; the case-file keys stay open)

Status: **REGISTERED 2026-11-14, written before any code of F-593 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addenda is changed after a run.

Roadmap item F-593 (deps F-387, F-386), the follow-up of [ADR 0500](0500-f387-registration-fan-blade-static-and-modal-demonstration.md)
(open questions 1 and 2). Number 0505 given by the lead. Branch `f593-work` from 3aec9cdb, worktree `w49i-work/f593`, scratch
`w49i-work/f593-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code; no existing test
edited; clean-room. The files of F-591, F-529 and F-562 are not touched.

## Scope of this ADR

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. rcfd_fem::builders gains the structured Tet10 blade mesh of
> ADR 0500 D1 for an AxialFan with given divisions, with the sets hub, tip, pressure and suction; the solid case file gains a
> centrifugal load (axis, origin, speed) and a mesh source fan_blade (a ComputerFan or its defaults, the blade index and the
> divisions); the F-387 blade run from a file gives the bits of the library run, the existing solid-case and schema tests pass
> unedited, and docs/solid-case-file.md and docs/structural-solver.md describe the keys.

Clauses: (a0) every limit registered here; (b) the library builder with its four sets; (c) the case-file centrifugal load and mesh
source; (d) the run from a file equals the library run in bits; (e) existing solid-case and schema tests pass unedited; (f) the
two pages describe the keys.

**This ADR covers (a0), (b), (e) and the builder paragraph of `docs/structural-solver.md`.** Clauses (c), (d) and the case-file
part of (f) touch the case schema and need the maintainer's approval; no case-file key and no schema change is made. The proposed
keys are written in "Open questions". Status by the rule below: **partial**.

## What was read (no run)

`AGENT_RULES.md`; ADR 0500 (D1, S0 to S2, amendment 1, open questions 1 and 2); `crates/rcfd-fem/tests/sb_fan_blade.rs`
(`blade_tet4`, `tet10`, `clamped`, `run_blade`, `first_moment`, `area_vector`, `reaction_sum`, `tip_displacement`);
`crates/rcfd-fem/src/builders.rs` (`tet_box`, `upgrade_to_tet10`: surface sets keep their members, node sets gain the mid-edge
nodes of edges both of whose ends they contain); `rcfd_mesh::fan` (`AxialFan::validate`, `station_at`, `surface_zs`) and
`fan_design.rs` (`ComputerFan::axial_fan(k, inlet, outlet)`, `n_blades`, `Deviation::blades`); `FemError::InvalidValue`.
No arithmetic and no run beforehand.

## Design

### B1. The functions (in `crates/rcfd-fem/src/builders.rs`)

- `fan_blade_tet4(fan: &AxialFan, divisions: [usize; 3]) -> Result<SolidMesh>`: ADR 0500 D1 verbatim: chord parameter
  `x_i = (1 - cos(pi i / n_c)) / 2`, thickness fraction `k / n_t`, radius `r_hub + L j / n_s`, edge nodes merged, the six Kuhn
  tetrahedra of each cell in the order of the test's `PERMS`, repeated-node tetrahedra dropped, each oriented positive by the
  signed volume (swap of vertices 1 and 2 when negative). Identical arithmetic in identical order to the test, so the node
  coordinates and the connectivity are bit-equal to the test's mesh.
- `fan_blade_tet10(fan, divisions) -> Result<SolidMesh>`: `fan_blade_tet4` then `upgrade_to_tet10`.
- `computer_fan_blade_tet10(cf: &ComputerFan, blade: usize, divisions) -> Result<SolidMesh>`: `cf.axial_fan(blade, 0.0, 0.0)`
  then `fan_blade_tet10`; `blade >= cf.n_blades` refused. (`ComputerFan::default()` is the "or its defaults" of the acceptance.)
- Sets: element set `all`; node sets `all`, `hub` (`j = 0`), `tip` (`j = n_s`); surface sets `hub`, `tip`, `pressure` as in ADR 0500
  D1 (classified in that order from the `face_incidence` faces with one member), and the new surface set **`suction`**: every
  boundary face (one member) not in `hub`, `tip` or `pressure`. The sets of the test keep their definition and order, so they are
  bit-equal to the test's.
- Refusals (`FemError::InvalidValue`, keys `divisions`, `fan`, `blade`): any division zero; `n_c < 2` (both closed edges would fall
  into one cell and the element-count formula below no longer holds; refused rather than half-defined); `fan.validate()`
  non-empty; `blade >= n_blades`. No other validation (an extreme blade that makes a bad mesh is the caller's).
- Element count (ADR 0500 S2): `6 (n_c - 2) n_t n_s + 6 n_t n_s`. Boundary faces: hub and tip `2 n_t (n_c - 1)` each, pressure
  and suction `2 n_c n_s` each (the suction side mirrors the pressure side), total `4 n_c n_s + 4 n_t (n_c - 1)`. These are
  formulas registered now and checked on the divisions below.

### B2. The tests: `crates/rcfd-fem/tests/sb_fan_blade_builder.rs` (new file)

The test file contains a verbatim copy of `blade_tet4` and its helpers from `sb_fan_blade.rs` as the reference (the original is not
edited and not imported), the F-387 run helpers (`clamped`, `with_load`, `first_moment`, `area_vector`, `reaction_sum`,
`tip_displacement`, `modal_options`, `static_options`) copied, and the `Checks` helper (a non-finite value is a miss). Divisions
M1 `[12, 2, 8]`, M2 `[24, 2, 16]` of ADR 0500; the fan is `ComputerFan::default().axial_fan(0, 0.0, 0.0)`; material, speed (2000
rpm) and pressure (50 Pa) of ADR 0500.

## Checks (limits fixed now; no run yet)

| id | clause | quantity | limit | estimate / reason |
|---|---|---|---|---|
| BM1 | b | M1, M2 Tet4: builder mesh vs the test's copy: `f64::to_bits` of every node coordinate, every connectivity entry, element and node counts | zero differences (bit-equal), counts `1056` and `4416` | same arithmetic in the same order |
| BM2 | b | M1, M2 Tet10 (`fan_blade_tet10` vs the copy lifted by `upgrade_to_tet10`): same | zero differences | deterministic lift |
| BM3 | b | the node sets `all`, `hub`, `tip`, the element set `all` and the surface sets `hub`, `tip`, `pressure` (as ordered `(element, face)` lists) vs the copy, on M1 and M2 Tet10 | equal | same classification |
| BS1 | b | surface sets `hub`, `tip`, `pressure`, `suction` exist and are non-empty; sizes `2 n_t (n_c - 1)`, `2 n_t (n_c - 1)`, `2 n_c n_s`, `2 n_c n_s` | equal to the formulas, on M1 and M2 | counted in ADR 0500 S2 for the first three; suction by mirror |
| BS2 | b | the four sets are pairwise disjoint as sets of `(element, face)` | zero common members | disjoint by construction |
| BS3 | b | the four sets together equal the set of faces with one incidence (`face_incidence`): union size equals that count, every such face in the union | equal (zero missing, zero extra); total `4 n_c n_s + 4 n_t (n_c - 1)` | construction |
| BS4 | b | closed boundary: the sum over the four sets of the outward area vectors (from corner coordinates, as `area_vector` of the copy, per set) | `norm(sum) / sum of face areas <= 1e-12` | a closed polyhedral surface has zero vector area; rounding only |
| BS5 | b | `suction` faces away from `pressure`: `dot(A_suction, A_pressure) < 0` | holds on M1, M2 | the two sides face opposite ways |
| BR1 | b | F-387 identity through the builder, M1 and M2: `e_R` (centrifugal reactions vs `rho omega^2 S_perp`) and `e_P` (pressure reactions vs `p sum A`), reaction sums over the `hub` node set | `<= 1e-8` each (the ADR 0500 limits) | 1.3e-11 and 1.2e-10 measured in ADR 0500 |
| BR2 | b | F-387 results through the builder vs through the copy, M1 and M2: both reaction sums (3 components each, centrifugal and pressure), the mean tip displacement under each load, the first natural frequency `f_1` (`ModalOptions::new(4)`, tolerance 1e-12, backward residual) | `f64::to_bits` equal | identical meshes, identical deterministic solves. If a parallel reduction makes bits differ, that is recorded as a known miss with the measured relative difference, not tuned |
| BR3 | b | every aggregated error and result of BR1, BR2, BC1 is finite | all finite | |
| BC1 | b | `computer_fan_blade_tet10(&ComputerFan::default(), 0, M1)` is bit-equal to `fan_blade_tet10(&cf.axial_fan(0, 0.0, 0.0), M1)` (coordinates, connectivity, all sets) | zero differences | direct composition |
| BC2 | b | blade index selects the blade: a `ComputerFan` with `deviation.blades` of length `n_blades`, `chord_rel = 0.05` for blade 1 and zero elsewhere: `computer_fan_blade_tet10(.., 1, M1)` is bit-equal to the builder on `axial_fan(1, 0.0, 0.0)` and differs from blade 0's mesh in at least one coordinate | both hold | `axial_fan(k)` reads the deviation of blade `k` |
| BF1 | b | refusals: divisions `[0, 2, 8]`, `[12, 0, 8]`, `[12, 2, 0]`, `[1, 2, 8]` each `Err`; `blade = n_blades` and `blade = usize::MAX` each `Err`; a fan with `r_tip = r_hub` `Err` | all 7 refused | |
| BF2 | b | the smallest legal mesh `[2, 1, 1]` (positive control): 6 Tet4, builds, Tet10 builds, `hub`, `tip`, `pressure`, `suction` non-empty | holds | count `6 n_t n_s` for `n_c = 2` |
| DOC1 | b | `docs/structural-solver.md` names the builder; `cargo test -p rcfd-cli --release --test docs_site` | passes | |
| EX1 | e | existing tests unedited: `cargo test -p rcfd-fem --release` (all targets, including `sb_fan_blade.rs`), `cargo test -p rcfd-api --release --test baseline_hashes` | all pass; baseline hashes unchanged | public functions and a test only |
| FM1 | | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |

Not run and not claimed (the case-file half): (c), (d) and the case-file part of (f).

## Negative controls and planned mutations

Negative controls (each a test assertion, run once):

- NC1: the builder mesh compared to the copy built with divisions `[12, 2, 9]` reports nonzero differences in the BM1 comparison
  (a comparison that cannot fail would show zero).
- NC2: `pressure` and `suction` are not equal and their intersection is empty, both non-empty (BS2 is not vacuous).
- NC3: BS3 applied with the `suction` set left out reports a missing count of exactly `2 n_c n_s` (the check detects an uncovered
  boundary).

Mutations of the library, each run once on a scratch copy of the worktree with its own `CARGO_TARGET_DIR`; the named check must
fail:

| id | mutation | caught by |
|---|---|---|
| MU1 | chord parameter `x_i` uses `i / n_c` (uniform) instead of the cosine spacing | BM1 |
| MU2 | `suction` also contains the `pressure` faces (classification overlap) | BS2 |
| MU3 | `suction` drops its last face | BS3, BS1, BS4 |
| MU4 | orientation fix removed (no swap of vertices 1 and 2 for a negative volume) | BM1, BR1 |
| MU5 | `hub` classification uses `node_j == 1` | BM3, BS1 |
| MU6 | the blade index check removed | BF1 |
| MU7 | `fan_blade_tet10` omits `upgrade_to_tet10` (returns Tet4) | BM2 |

## Decision rule

- **done**: all checks of the table met and the case-file clauses (c), (d), (f) done. Not reachable in this ADR.
- **partial** (the status registered for this ADR): every check of the table met, the case-file clauses open pending the
  maintainer. A missed check with a diagnosed cause also gives partial, recorded as an ignored `*_known_miss` test.
- **todo**: the builder not delivered.

## Limits (what is not covered)

The limits of ADR 0500 (mid-edge nodes at straight-edge midpoints, not on the curved blade; no hub flexibility, no fillet; nothing
validated against a measured blade). The builder does not check element quality of extreme blades (the orientation swap would hide
an inverted element; BM1 and BR1 cover only the default blade). No case-file key.

## Open questions (to the maintainer)

1. **The case-file keys (not implemented; they need the maintainer's approval because they touch the case schema, its golden and
   the solid-case tests).** Proposed, in the form of `docs/solid-case-file.md`:
   - Mesh source `[mesh] generator = "fan_blade"` with keys `fan` (table: a `ComputerFan` as in the fan design file; absent means
     `ComputerFan::default()`), `blade` (integer, default 0, refused if `>= n_blades`), `divisions` (array of three positive
     integers, `n_c >= 2`, default `[12, 2, 8]`, refused otherwise). The mesh is `computer_fan_blade_tet10`; the surface and node
     sets `hub`, `tip`, `pressure`, `suction` and the element set `all` are usable by name in `[materials]`, `[constraints]` and
     `[loads]`. Refused: `fan`, `blade` or `divisions` given with another generator.
   - Load `[loads.<name>] kind = "centrifugal"` with `axis` (three numbers, not zero), `origin` (three numbers, default `[0, 0,
     0]`), `speed` (number, rad/s, finite; one unit only, no `rpm` alias). Refusals: missing `axis` or `speed`, zero axis,
     non-finite value, unknown key (the file's usual `deny_unknown_fields`).
   - Recommendation: take the keys as written, add them in a separate item with the schema golden updated by the maintainer, and
     make check (d) an `rcfd-cli` test `run from file == library run` by `f64::to_bits` on M1 (the BR2 list).
2. **Mid-edge nodes on the curved surface.** The builder keeps straight-edge midpoints (ADR 0500). Options: (a) keep; (b) a
   further function projecting boundary mid-edge nodes onto the parametric surface (defaults unchanged). Recommendation: (a) for
   now.

Both are open; none is taken.

## For the lead: roadmap entries

(To be written in addendum 1.)
