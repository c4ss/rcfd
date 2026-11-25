# ADR 0797: F-889 registration: case-file keys for the cutoff, the load's residual vector and the load patterns of the reduction

Status: **REGISTERED 2026-11-25, written before any code of F-889 and before any run of its checks.** Before this text no
test or library code was written, no file of the repository was changed and nothing was built or run for it. What was done:
reading (below) and searches of the source tree; after the text was written, the link check `docs_site` the brief asks
for, on this text. Results go in addenda at the end; nothing above them is changed afterwards.

It answers the brief `docs/adr/briefs/f889-reduction-keys.md` ([the file](briefs/f889-reduction-keys.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f889-reduction-keys` from main 25311db5,
scratch `w49i-work/f889-reduction-keys-scratch`. Model: Claude Opus 5.5.

## What is asked

Roadmap item F-889 (deps F-616), the binding acceptance:

> Every limit not stated here is registered in the ADR before the run. The table [reduction] of the solid case file gains keys for the cutoff and the residual vector of the interior load of ADR 0474 and for load patterns of ADR 0511 named by entries of the case's loads; a part file made from a case file with each key equals the library's byte for byte, the configuration E1 of ADR 0532 (cutoff 32964 rad/s, the load's vector, one block of enrichment) is made from a case file and gives the library's frequencies in bits, and a case without the keys gives every result bit-identical.

Clauses: **c1** the key of the cutoff; **c2** the key of the residual vector of the interior load; **c3** the key of the
load patterns, named by entries of the case's loads; **c4** a part file made with each key equals the library's byte for
byte; **c5** E1 of ADR 0532 from case files gives the library's frequencies in bits; **c6** a case without the keys gives
every result bit-identical.

The lead's decisions of 2026-11-25 are in the brief: the keys belong to the existing table `[reduction]` and are optional;
a name of a load that the case does not have, a cutoff that is not positive and finite, a key the reduction cannot use (a
load's residual vector without an interior load) and two keys that the library refuses together are refused by key and
line, and where the library refuses, its refusal is what is reported. No golden holds the solid schema (ADR 0785,
finding 1).

## What was read

`AGENTS.md`; `docs/process/agent-rules.md` (version 5, whole); the brief (whole); the item F-889 of `docs/roadmap.json` (by
a script); [ADR 0785](0785-f616-registration-case-file-keys-for-superelements-and-interface-enrichment.md) (whole);
[ADR 0474](0474-f552-registration-craig-bampton-residual-vectors-cutoff.md) (whole);
[ADR 0511](0511-f559-registration-craig-bampton-residual-vectors-of-load-patterns.md) (the options, the set-ups) and
[ADR 0532](0532-f603-registration-craig-bampton-interface-enrichment.md) (the configurations, the options, the set-ups, the
rows of E1); [docs/solid-case-file.md](../solid-case-file.md) (whole); `crates/rcfd-api/src/solid_case.rs` (the schema, the
setup types, the reader, `build`, the reduction's run); `crates/rcfd-fem/src/cms.rs` (the module documentation,
`CmsOptions`, `reduce` whole, `reduce_load`); of `crates/rcfd-fem/src` also `DofMap`, `Constraint`, `Load` and
`SolidModel::loads`/`constraints` in `model.rs`, `nodal_load_vector` in `loads.rs`; `coerce` in
`crates/rcfd-core/src/settings.rs`, the unit table of `units.rs`; `crates/rcfd-fem/tests/sa_cms_enrich.rs` (lines 1 to
175); `crates/rcfd-api/tests/f616_superelement_case.rs` (the helpers and the refusals) and
`f616_superelement_case_default_bits.rs` (whole) with their patch; ADR 0793 (the form of the rows, the Linux record); ADR
0150 (D4, the container); the [tolerance table](../process/tolerances.md).

No published source is used: nothing numerical is added. The reduction and its options are those of ADRs 0469, 0474, 0511
and 0532, called as they are.

## Findings of the reading (before any run)

1. **The case file can write E1.** B's corner force is a `nodal_force` on a node set of one node, selected by a box. So
   the stop of the brief's task 1 does not apply.
2. **Part A of E1 has no load**, and E1 of ADR 0532 asks the load's vector on both parts. In the library the option is
   inert there (ADR 0474 D3: A with it equals A without it, same bytes). By the lead's decision a case that asks the vector
   without an interior load is refused. So A's case file gives the cutoff and the enrichment only, and check E1 compares
   its file with the library's A that has `residual_vectors(true)`, byte for byte.
3. **The library refuses three things about the cutoff** (not finite, not positive, with a mode count), all with
   `FemError::InvalidValue { key: "cutoff", .. }`. A value TOML writes as `inf` or `nan` does not reach it: the reader's
   settings refuse a number that is not finite. The library takes a pattern of any content: a zero interior part or one in
   the span of the basis is dropped without a message.
4. **The unit table reads `Hz` as 1/s.** A key of the dimension 1/s would take `"5246 Hz"` as 5246 rad/s without a word.
   `solid_case.rs` has a key with a plain number and its unit named (`materials.*.expansion`, `unit("1/K")`), which
   refuses a string with a unit.
5. **No transcendental function enters the reduction** (searched: `sin`, `cos`, `exp`, `ln`, `powf`, `hypot` and the like
   in the mesh builder's box, assembly, mass, loads, `cms.rs`, `damped.rs`, the dense and subspace eigen-solutions and the
   skyline factor). The records of main are of IEEE operations and square roots, so one record is asserted on every
   platform; it is made on Windows x86_64 and on Linux x86_64 before the change (row L).
6. The schema digest S1 of F-616 removes the whole table `reduction`, so it does not see the new keys; a new row (S1)
   holds the table's present entries.

## Design

Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation).

- **D1. The keys**, in the existing table `[reduction]`, named as the library's options and in the library's units:

  | Key | Type | Unit | Default | Library |
  |---|---|---|---|---|
  | `cutoff` | number | rad/s | absent: no cutoff | `CmsOptions::cutoff(omega)`; with it `modes` is 0 or absent |
  | `residual_vectors` | boolean | none | `false` | `CmsOptions::residual_vectors(on)`: the vector of the sum of the case's loads |
  | `load_patterns` | list of strings | none | empty | `CmsOptions::load_patterns(v)`: one vector per name, in the list's order |

  `cutoff` is a plain number (finding 4): a string with a unit is refused by the settings. Whether it is given is read
  from the file, not from the schema's default (the number 0, which the library refuses when it is written).
- **D2. A load pattern** named `x` is `nodal_load_vector` of the model whose only load is the case's load `x` (mesh, sets,
  materials and constraints as built). This is the pattern of ADR 0511's set-ups. **The order of the patterns is the order
  of the list in the file**, never the order of a map or of the names. A name may appear twice; the library drops the second
  (its vector is in the span of the first). The case's loads stay the model's loads: naming a load as a pattern takes
  nothing away from the sum. Nothing is computed for it when the list is empty or absent.
- **D3. Refusals** (each with key and line):
  - an unknown name in `load_patterns`, at `reduction.load_patterns` (the message names the load and lists the case's);
  - `residual_vectors = true` where the sum of the case's loads has no interior part, at `reduction.residual_vectors`;
  - a name in `load_patterns` whose load has no interior part, at `reduction.load_patterns` (taken as the same class as the
    lead's decision: the library would drop it in silence);
  - a cutoff the library refuses (not positive, or with a mode count) at `reduction.cutoff`, with the library's message:
    an `InvalidValue` of `reduce` with the key `cutoff` is reported at `reduction.cutoff`; every other refusal of `reduce`
    stays at `reduction` as now (the test of F-616 on `modes = 500` is unedited);
  - a cutoff that is not finite, a string for it, a list of the wrong type, by the settings as now.
  "No interior part" means: the load vector is zero at every equation whose node is not in the interface and whose component
  is not constrained at that node (`Constraint::Displacement`, the only kind a case file writes). It is evaluated at
  `build`, so it is reported with the other problems of the file. It is independent of the library; the tests compare it
  with `static_interior()` of the reduced part (check Z).
- **D4. Nothing else changes.** `[reduction]` without the keys calls the same `CmsOptions` as before; the report, the part
  file and `CaseRun` keep their shape. The library of `rcfd-fem` is not edited. `crates/rcfd-api/src/solid_case.rs` is the
  only source file changed.

## Set-ups

Steel `E = 2.1e11`, `nu = 0.3`, `rho = 7850`, Hex8 `hex_box`, cross-section `0.1 x 0.08`, `2 x 2` cells. **A**: `[0, 0.5]`,
8 cells, clamped at `xmin` (all components), interface `xmax`, no load. **B**: `[0.5, 1]`, 8 cells, free, interface `xmin`,
node set `corner` (the one node at `(1, 0.1, 0.08)`, box with both corners there), load `tip` = `nodal_force` `[1, 1, 1]`.
**B3**: B with the loads `f_x`, `f_y`, `f_z` (the unit forces) in place of `tip` (their sum is `[1, 1, 1]` again).
**Bm**: B with the loads `a_traction` (traction `[1e6, 2e6, -3e6]` on `xmax`), `b_gravity` (`[0, -9.81, -9.81]`),
`c_pressure` (pressure 2e5 on `ymax`). Cutoff 32964 rad/s (ADR 0474). The library side of every comparison is the same model
by library calls and `reduce` with the registered options and `threads(1)`; the case side runs `run_solid_case_any` with a
file that holds the case text, so nothing of the test's own reader is used. "Byte for byte": the part file's bytes against
`ReducedPart::to_text` of the library's part.

## Checks

Tests in `crates/rcfd-api/tests/solid_case_reduction_keys_f889.rs`, release profile, except the rows that name other files.
The records of main (D1, S1) are committed, with their numbers, before any library line; D1 of main is run on Windows
x86_64 and in the Linux container of ADR 0150.

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| K1 | c1, c4 | B and A with `cutoff = 32964.0` alone: the part file against the library's | equal bytes; modes 9 and 5 (ADR 0474 addendum: A 5, B 9) | acceptance of F-889; ADR 0474 | equal | exact | code verification |
| K2 | c2, c4 | B3 with `modes = 5`, `residual_vectors = true` alone: the part file against the library's; `false` written out: equal to the file without the key and to the library without it; the two differ; B with `tip` likewise | equal bytes; one residual vector; differ | acceptance of F-889 | equal | exact | code verification |
| K3 | c3, c4 | B3 with `modes = 5`, `load_patterns = ["f_z", "f_x", "f_y"]` alone: the part file against the library's with the patterns in this order; against the library's in the order `f_x, f_y, f_z` | equal bytes; differ from the second | acceptance of F-889 (the order of the case) | equal; differ | exact | code verification |
| K4 | c4 | the combinations the library takes, on B3: `cutoff` with `residual_vectors`, `cutoff` with `load_patterns`, `residual_vectors` with `load_patterns`, all three; each with `interface_enrichment` 0 and 1 (8 cases; without a cutoff `modes = 5`) | equal bytes, 8 of 8 | acceptance of F-889 | equal | exact | code verification |
| K5 | c3, c4 | Bm with `load_patterns = ["c_pressure", "a_traction"]` (gravity is a load of the model and not a pattern) and `modes = 5`: equal to the library with `nodal_load_vector` of the single-load models; a repeated name (`["a_traction", "c_pressure", "a_traction"]`): equal to the library given the vector twice | equal bytes; the repeated name drops one vector | acceptance of F-889 | equal | exact | code verification |
| E1 | c5 | A and B from case files: `cutoff = 32964.0`, `interface_enrichment = 1`, and on B `residual_vectors = true`; each part file against the library's E1 part (`CmsOptions::new(0).cutoff(32964).residual_vectors(true).interface_enrichment(1)` on both parts); the assembly of the two files read back against the assembly of the two library parts: every eigenvalue, and every frequency `sqrt(lambda)` of it | equal bytes; equal in bits; 27 and 28 residual vectors (ADR 0532 E-P1) | acceptance of F-889; ADR 0532 | equal | exact | code verification |
| R | c1 to c3 | the refusals of D3, one case each: `cutoff = 0.0`; `-1.0`; `inf`; `nan`; `"5246 Hz"`; `"32964 rad/s"`; `cutoff` with `modes = 5`; `load_patterns = ["nowhere"]`; `load_patterns = "f_x"`; `[1]`; `residual_vectors = true` on A (no load), on A with a traction on `xmax` (its interface), on A with a nodal force on `xmin` (clamped); a pattern `f_x` of A with that load on `xmin`, and one with a traction on `xmax`; `residual_vectors = "yes"` | 16 of 16 refused at the registered key and line with the registered text (the library's for the cutoff's value and count) | acceptance of F-386 (located errors, ADR 0438); the brief | refused | exact | code verification |
| R2 | c2, c3 | a case with two problems of D3 in one file (an unknown pattern name and, on A, `residual_vectors = true`) reports both, each with its key and line | both | the reader's rule (ADR 0438) | both | exact | code verification |
| Z | c2, c3 | of the refused loads of R: the library's part has `static_interior()` zero in every entry (the case's loads); of the accepted ones (B3, Bm): not zero | zero; not zero | the definition of D3 against the library | holds | exact | code verification |
| N | c2, c3 | negative control: `residual_vectors = true` on A with a traction on `xmax` and gravity (an interior load, an interface load with it) runs; without the interface load the same; the pattern named `b_gravity` of Bm runs | accepted | acceptance of F-889 | accepted | exact | code verification |
| D1 | c6 | digests recorded on main 25311db5 before any library line, of part cases without the new keys: A, B and Bm with `modes` 0 and 5, and with `interface_enrichment = 1` and 5 modes (9 cases): part file and report (without `outputs`); and the static and modal run of the host of arrangement I of F-616 with the part of A: report without `outputs`; equal after the change | equal | acceptance of F-889; tolerances, bit-identity | equal | exact | unchanged default output |
| S1 | c6 | the digest, recorded on main 25311db5, of the JSON schema of the solid case file with the properties `cutoff`, `residual_vectors` and `load_patterns` removed from `reduction` (and from its `required`), a removal that does nothing on main; the digest of the file is equal after the change and the diff is the three added entries | equal | rule 3, the class of additive edits | equal | exact | unchanged default output |
| L | c6 | Linux x86_64 in the container of ADR 0150: the test file on an export of the commit of the records (its D1 and S1 against main's on Windows), then at the final commit | equal; passes | acceptance of F-889 (bit-identical); tolerances, bit-identity | equal | exact | unchanged default output |
| X1 | c6 | unedited: `f616_superelement_case`, `f616_superelement_case_default_bits`, `sb_solid_case`, `sb_solid_case_iterative` | pass | rule 3 | pass | exact | unchanged default output |
| X2 | c6 | unedited: `baseline_hashes`, `case_hashes_w48u`, `w48v_case_hashes` (its one test is an ignored print: run with `--ignored` on the branch and on a copy of main, the printed lines equal) | pass; equal | rule 3 | pass | exact | unchanged default output |
| X3 | design | unedited, the library files that hold the reduction: `sa_cms_residual`, `sa_cms_pattern`, `sa_cms_enrich` of `rcfd-fem` (no file of it is changed; run on the branch as it is) | pass, as on main | rule 3 | pass | exact | unchanged default output |

No limit on time is set. No check is of kind validation: the acceptance claims no agreement with a measured case.

### Negative controls and mutations

Negative controls: K2's and K3's "differ"; the refusals of R; N. Planned mutations (rule 16), each alone on a scratch copy
of the final code commit with a target folder of its own, the test file run once, the file restored; the unchanged copy is
run too. Each must be caught by a row of the table, named here as expected:

| Id | Change in `solid_case.rs` | Expected to be caught by |
|---|---|---|
| M1 | `cutoff` read and ignored | K1, K4, E1, R |
| M2 | `residual_vectors` read and ignored | K2, K4, E1 |
| M3 | `load_patterns` read and ignored | K3, K4, K5 |
| M4 | the cutoff taken in Hz (multiplied by `2 pi`) | K1, K4, E1 |
| M5 | the patterns sorted by name | K3, K4, K5 |
| M6 | the pattern of another load (the case's first load, by name, for every name) | K3, K4, K5 |
| M7 | the load's vector asked for the case's first load only (as a pattern) instead of the sum | K2, K4 |
| M8 | the pattern as the sum of all loads instead of the named one | K3, K4, K5 |
| M9 | the refusal of an unknown pattern name removed (the name skipped) | R, R2 |
| M10 | the refusal of `residual_vectors` without an interior load removed | R, R2 |
| M11 | the refusal of a pattern without an interior part removed | R |
| M12 | the interior test ignores the constraints | R |
| M13 | a non-positive cutoff treated as absent | R |
| M14 | `modes` ignored when a cutoff is given | R |
| M15 | the refusal of the cutoff reported at `reduction` | R |
| M16 | a default of `residual_vectors` other than absent (`true`) | D1, K1, K3, R |
| M17 | a default of `cutoff` other than absent (`Some(1e5)`) | D1, K2, K3 |
| M18 | a default of `load_patterns` other than absent (every load) | D1, K1, K2 |

Committed: M4 as `crates/rcfd-api/tests/solid_case_reduction_keys_f889.mutation.patch`, its command running the test of K1.

Size (agent): done; 0; 0; 0; 220000

## Decision rule

**done** if every row of the table holds, each mutation is caught by its row, the named existing tests pass unedited, and no
amendment is made after a run. **partial** otherwise, with the key named (a missed row, a mutation not caught by a registered
row, or a row met only after an amendment made after its run). **todo** if the keys cannot be built. A row `K*`, E1 or Z is
attributed to its key in the report.

## What is not covered

- A cutoff, vector or pattern of a superelement case: a `[superelements.<name>]` host reads a finished part file, so these
  keys belong to the part case only (as in F-616).
- A pattern that is not a load of the case (a vector over mesh equations from elsewhere), a pattern for a thermal or moment
  load (the case file has the four kinds of load).
- A pattern in the span of the modes and vectors before it is dropped by the library and is not refused; `residual_vectors`
  whose vector lies in the span of the kept modes (ADR 0474 RV3) is likewise not refused.
- The text of the refusals of `reduce` other than the cutoff, reported at `reduction` as now.
- Frequencies of E1 against the full model (ADR 0532 holds them; here they are equal in bits to the library's by rows
  E1) and the harmonic and response checks of ADRs 0474, 0511 and 0532.
- A CLI subcommand or MCP tool (there is none for any solid case).
- No golden is edited and none needs to be (ADR 0785, finding 1).

## Open questions for the maintainer

None expected.
