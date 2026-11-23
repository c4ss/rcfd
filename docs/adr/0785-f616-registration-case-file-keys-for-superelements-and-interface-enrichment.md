# ADR 0785: F-616 registration: case-file keys for superelements and interface enrichment

Status: **REGISTERED 2026-11-23, written before any code of F-616 and before any run of its checks.** Before this text no
library or test code of F-616 was written, no file of the repository was changed and no program was run for it: what was
done is reading (below). Results go in addenda at the end; nothing above them is changed afterwards.

It answers the brief `docs/adr/briefs/f616-superelement-case-keys.md` ([the file](briefs/f616-superelement-case-keys.md))
and follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f616-superelement-case-keys`
from main fcc84a84, scratch `w49i-work/f616-superelement-case-keys-scratch`.

## What is asked

Roadmap item F-616 (deps F-553, F-603), the binding acceptance:

> Every limit not stated here is registered in the ADR before the run. A case file names reduced part files of ADR 0469 and the host node sets they attach to (key superelements) and the interface enrichment of ADR 0532 (key interface_enrichment); the bar of ADR 0536 run from a case file gives the library's results in bits; the schema golden re-recorded with the maintainer's approval; a case without the keys gives every result bit-identical.

Clauses: **c1** a case file names reduced part files of ADR 0469 and the host node sets they attach to, by the key
`superelements`; **c2** a case file names the interface enrichment of ADR 0532, by the key `interface_enrichment`;
**c3** the bar of ADR 0536 run from a case file gives the library's results in bits; **c4** the schema golden
re-recorded with the maintainer's approval (by the class rule of rule 3: the agent reports an additive diff and edits no
golden); **c5** a case without the keys gives every result bit-identical.

## What was read

`docs/process/agent-rules.md` (version 5, whole); the brief (whole); the item F-616 of `docs/roadmap.json` (by a
script); [ADR 0536](0536-f553-registration-reduced-parts-as-superelements.md) (whole, with its addendum and the
maintainer's answer of 2026-11-18); [ADR 0532](0532-f603-registration-craig-bampton-interface-enrichment.md) (the
acceptance, the design, the options and API, the checks' table, remaining question 1);
[ADR 0469](0469-f430-registration-substructuring-craig-bampton.md) (the lines on the part file and its checks F1, F2);
[ADR 0780](0780-f842-registration-inertia-and-retry-in-the-solid-case-output.md) (whole: the newest registration on the
solid case file, its finding 5 on goldens); `crates/rcfd-api/src/solid_case.rs` (whole);
[docs/solid-case-file.md](../solid-case-file.md) (whole); `crates/rcfd-fem/src/cms/superelement.rs` (whole);
`crates/rcfd-fem/src/cms.rs` (the module documentation, `CmsOptions`, `BoundaryUnknown`, `ReducedPart` and the list of
its public functions, the documentation of `reduce`, `write`, `read`); `crates/rcfd-fem/tests/sa_superelement.rs` (the
set-ups: lines 1 to 200) and `sa_superelement_default_bits.rs` (the digest pattern); `crates/rcfd-api/tests/sb_solid_case.rs`
(the helpers and how the tests match `SolidOutcome`); three case files of `crates/rcfd-api/tests/solid_cases/`; the heads of
`crates/rcfd-api/tests/baseline_hashes.rs`, `case_hashes_w48u.rs`, `w48v_case_hashes.rs` and `case_schema_page.rs`;
`docs/process/lead-process.md` (the section on golden files); `scripts/push_main.py` (the mutation patch) and a committed
patch; the rows of [the tolerance table](../process/tolerances.md) on bit-identity.

No published source is used: nothing numerical is added. The reduction, the enrichment and the superelement are those of
ADRs 0469, 0532 and 0536, called as they are; this work is the reading of keys and the calls.

## Findings of the reading

1. **The solid case's schema is in no golden.** `rcfd_api::solid_case::schema()` is named by `solid_case.rs` and by
   nothing else in `crates/rcfd-api/src`, `crates/rcfd-cli`, `crates/rcfd-mcp` or `scripts` (searched); the goldens of
   the repository hold the flow case's schema, the CLI help and the MCP tool lists. ADR 0780 found the same (its
   finding 5). So c4 has no file to re-record. What is reported instead is the additive diff of the schema itself
   (check S1), for the lead to put in the ledger.
2. **The existing tests match `SolidOutcome` exhaustively** (`sb_solid_case.rs`, `static_of` and `modal_of`), and an
   existing test is not edited (rule 2). A run with superelements can therefore not be a new variant of `SolidOutcome`:
   it gets its own types and its own entry (design D5).
3. **The interface enrichment is an option of the reduction** (`CmsOptions::interface_enrichment`), not of a reduced
   part: a part file does not say which of its vectors are of the enrichment (ADR 0532, "no new file field"), and the
   vectors need `K_ii` and `M_ii`, which the file does not hold. A case file can name the enrichment only where it asks
   for a reduction. So the case file must be able to reduce its model to a part file (design D2).
4. **The solid case file has a static and a modal analysis and no harmonic one.** The direct harmonic solve of ADR 0536
   cannot be asked by a case file, with or without superelements (open question 1).
5. `SuperelementModel::add_part` matches the part to host nodes by position over every host node and takes no node
   set. The check that the part attaches to the named set is made by the reader (design D3).

## Design

Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation), except open question 1.

- **D1. The key `superelements`**: named tables `[superelements.<name>]`, processed in ascending order of their names
  (as every named table of the file), each with
  - `file` (string, required): a part file written by `ReducedPart::write` (ADR 0469; version 1 or 2), relative to the
    case file;
  - `set` (string, required): the node set of the host (the case's mesh) the part attaches to.

  The case's model is the host. Part `p` of the combined model is the `p`-th name in ascending order.
- **D2. The key `interface_enrichment`**, in a new table `[reduction]`. A case file with a `[reduction]` table is a part
  case: its model is built as for any case, reduced by `rcfd_fem::cms::reduce` and written as a part file; nothing is
  solved. Keys:
  - `interface` (string, required): the interface node set of the model;
  - `modes` (integer, default 0, 0 to 10000): fixed-interface modes kept (`CmsOptions::new(modes)`; 0 is static
    condensation);
  - `interface_enrichment` (integer, default 0, 0 to 10000): Krylov blocks of the interface enrichment
    (`CmsOptions::interface_enrichment`, ADR 0532; 0 adds none and the reduction is that of ADR 0469 bit for bit, ADR
    0532 D7);
  - `file` (string, default `part.cms`): the part file written, into the output directory of the run (as the outputs).

  The form (a) of ADR 0532's remaining question 1 put the key "in the part entry of the substructure key"; by finding 3
  it belongs where the reduction is asked, which is this table. A table of its own, and not a third value of
  `analysis.type`, so that no existing schema entry changes (an enumeration with one more value, and the description
  of `analysis.type`, would be changed entries, not added ones).
- **D3. The node set check** (at build, each a located error). With `tol = MATCH_TOLERANCE` (1e-9) times the largest
  extent of the host's bounding box (1 if that is 0), the tolerance of `SuperelementModel::new`: every boundary unknown
  of the part has a node of `set` within `tol` of its position in each coordinate, and every node of `set` has a
  boundary unknown of the part within `tol`. A part that would attach outside the named set, and a set with nodes the
  part does not hold, are both refused. Then `SuperelementModel::new(host)` and `add_part` in name order; their
  refusals (a fixed or dependent host unknown, a component the host lacks, a non-zero constraint value) are located
  errors.
- **D4. The runs.** With superelements: `[analysis] type = "static"` gives `SuperelementModel::solve_static(None)`
  (the envelope factorisation; `solver.method` must be `direct`), `type = "modal"` gives
  `SuperelementModel::modal_analysis` with the `ModalOptions` the case gives today (`modes`, `tolerance`,
  `residual_test`). The threads are left at `None`, as the case's runs without the keys do; the result does not depend
  on them (ADR 0536 T1). The part's interior is not recovered: that needs the part's model (`recover_part_static`),
  which the host case does not have.
- **D5. The entry and the types** (finding 2). `SolidSetup` gains two public fields: `superelements` (name, file, set
  and the part read, per entry) and `reduction` (the interface, the `CmsOptions`, the file), empty and `None` for a
  case without the keys. New: `SuperOutcome` (`Static(SuperStatic)`, `Modal(SuperModes)`), `SuperRun` (the setup, the
  `SuperelementModel`, the outcome, the report, the files written), `ReductionRun` (the setup, the part, the report,
  the files written), the enumeration `CaseRun` (`Solid(SolidRun)`, `Superelements(SuperRun)`,
  `Reduction(ReductionRun)`, non-exhaustive) and the entry `run_solid_case_any(path, out_dir)`, which runs any solid
  case file. `run` and `run_solid_case_file` keep their signatures and, for a case without the keys, every byte of
  their results; given a case with one of the keys they refuse with a located error that names
  `run_solid_case_any` (their return type cannot hold the new outcomes).
- **D6. Outputs.** The report keeps the format name `rcfd-solid-report/1` and gains keys only in the new runs.
  Superelements: `format`, `title`, `mesh`, `analysis` (`static` or `modal`), `superelements` (a list in part order:
  `name`, `file`, `set`, `boundary_unknowns`, `generalised_coordinates`, `offset`), `unknowns` (of the combined
  model); static: `displacement` of the host (largest magnitude and its node, componentwise minimum and maximum) and
  `solver` (`name` = `skyline-ldlt`); modal: `frequencies` (Hz, `sqrt(max(lambda, 0)) / (2 pi)`), `eigenvalues`,
  `solver` (`name` = `subspace`); `outputs`. No stress and no reaction (they need a `SolidResult`, which the combined
  solve does not give). The `.vtu` is the host mesh with the point array `displacement` (static) or `mode_<k>`
  (modal; the host part of each mode). Reduction: `format`, `title`, `mesh`, `analysis` = `reduction`, `reduction`
  (`interface`, `modes`, `interface_enrichment`, `boundary_unknowns`, `interior_unknowns`, `residual_vectors`,
  `reduced_unknowns`), `outputs` (`part`, `report`); no `.vtu`.
- **D7. Errors**, each with its dotted key and line, with the others of the file:
  `superelements.<name>.file` and `.set` required; the file not read or not a part file (the library's text) at
  `.file`; the set unknown (the node sets listed) at `.set`; the two refusals of D3 at `.set`; the refusals of
  `add_part` at `superelements.<name>` and of `SuperelementModel::new` at `superelements`; `solver.method` other than
  `direct` with superelements, at `solver.method`; `reduction.interface` required, and unknown (the node sets listed);
  `analysis`, `solver`, `superelements` or `output.vtu` given together with `reduction` (each at its own key, "does
  not apply to a reduction"); the refusals of `reduce` at `reduction`; `run` or `run_solid_case_file` on a case with
  one of the keys, at `superelements` or `reduction`.
- **D8. Defaults.** A case without the keys takes the code path it takes today: `build` adds nothing to the model,
  `run` calls what it calls now. The settings tree of such a case holds the two new tables at their defaults
  (`superelements` empty, `reduction` with its defaults); no output of a run is made from them. No numerics change and
  no file of `rcfd-fem` is edited (nothing of `cms` needs to become public: what is used is public already).

## Set-ups

The solid bar of ADR 0536 (its "Set-ups"): steel `E = 2.1e11`, `nu = 0.3`, `rho = 7850`; half A `hex_box` on
`[0, 0.5] x 0.1 x 0.08`, `8 x 2 x 2` Hex8, clamped on `xmin`, interface `xmax`; half B from `x = 0.5`, `8 x 2 x 2`,
interface `xmin`; the full bar F `16 x 2 x 2`, clamped on `xmin`. Static loads: the traction `(1e6, 2e6, -3e6)` Pa on
`xmax` of the half that reaches `x = 1`, then gravity `(0, -9.81, -9.81)` (in that order: the load tables are named
`a_traction` and `b_gravity`). Arrangement **I**: A reduced with five modes, B the host. Arrangement **II**: B reduced
with five modes (the loads of `x = 1` in the part), A the host.

- **From case files**: the part case (a `[reduction]` table: `interface`, `modes = 5`) run by `run_solid_case_any`
  writes the part file; the host case names that file and the set (`xmin` of B, or `xmax` of A) in
  `[superelements.half]` and is run by `run_solid_case_any`. The case files are texts of the test, written to the
  test's temporary directory (no file is added to `tests/solid_cases/`, which other tests list).
- **By the library** (the reference): the same models by `hex_box`, `Material::isotropic_elastic`,
  `Constraint::displacement`, `Load::traction`, `Load::gravity`; `reduce(part, interface,
  CmsOptions::new(5).threads(1))`, the part kept in memory (not through a file); `SuperelementModel::new(host)`,
  `add_part`, `solve_static(Some(1))`, and `modal_analysis` with `ModalOptions::new(8)`, the tolerance 1e-12, the
  backward residual test and one thread (the options of ADR 0536); the case file asks `modes = 8`,
  `tolerance = 1e-12`, `residual_test = "backward"`.
- **Two parts** (B4): the host is the middle third M of a bar `[0, 1.5]`: `hex_box` from `x = 0.5`, `8 x 2 x 2`, no
  constraint, gravity; part `a_root` is A (interface `xmax`, set `xmin` of M), part `b_tip` is the third C from `x = 1`
  to `1.5`, `8 x 2 x 2`, traction on its `xmax` and gravity (interface `xmin`, set `xmax` of M), five modes each.
- **Enrichment** (R1 to R3): the part case of A with `modes = 5` and `interface_enrichment = 1`; the reference
  `CmsOptions::new(5).interface_enrichment(1).threads(1)`.

Arithmetic before this registration: none; no program was run. The estimates below are from ADR 0536 (its addendum)
and from the construction (the case file makes the library's calls with the same arguments in the same order).

## Checks

New test files `crates/rcfd-api/tests/f616_superelement_case_default_bits.rs` (D1, S1; written with the API of main
fcc84a84 and committed with its digests before any library line) and `crates/rcfd-api/tests/f616_superelement_case.rs`
(the rest), release profile. "Equal in bits" is `f64::to_bits` equal for every entry and equal lengths.

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| K1 | c1 | the host case of arrangement I built: the setup holds one entry with the name, the file, the set and a part whose text (`to_text`) equals the library part's | equal bytes | acceptance of F-616 | equal | exact | code verification |
| K2 | c1 | each error of D7 on the superelement side, one case text each: `file` missing, `set` missing, the file absent, a file that is not a part file, the set unknown, the set `xmax` (the part is at `xmin`), the set `all` (nodes the part does not hold), the host clamped on the interface, a host constraint value 1e-3, `solver.method = "cg"` | each refused with the key registered in D7 and the line of that key or table | acceptance of F-386 (located errors, ADR 0438) | refused | exact | code verification |
| K3 | c1 | `run_solid_case_file` on the host case of arrangement I, and on the part case | refused, the message names `run_solid_case_any` | D5 of this registration (acceptance of F-616: the keys are opt-in) | refused | exact | code verification |
| B1 | c3 | arrangement I, static, from case files: `coordinates` and `host_displacement` against the library's | equal in bits | acceptance of F-616 | equal | exact | code verification |
| B2 | c3 | arrangement II, static (the loads in the part), likewise | equal in bits | acceptance of F-616 | equal | exact | code verification |
| B3 | c3 | arrangements I and II, modal, eight modes: `eigenvalues` and every entry of `vectors` against the library's | equal in bits | acceptance of F-616 | equal | exact | code verification |
| B4 | c1, c3 | the two-part set-up, static: `coordinates` and `host_displacement` against the library with `add_part` in the order `a_root`, `b_tip` | equal in bits | acceptance of F-616 | equal | exact | code verification |
| B5 | c3 | arrangement I from case files, static: the host displacements against the full bar F solved by `solve_static` (Direct), at the nodes of equal position, largest difference over the largest entry of F | `<= 1e-10` | acceptance of F-553 (ADR 0536, ST1) | 6.35e-13 (ADR 0536 addendum, ST1) | 157 | solution verification |
| B6 | c3 | the outputs of arrangement I: the report's `unknowns` 248 and its `superelements` entry (27 boundary unknowns, 5 generalised coordinates, offset 243); the `.vtu` exists and holds the array `displacement`; the report of the modal run holds eight `eigenvalues` equal to the library's as JSON numbers | equal | D6 of this registration; ADR 0536 addendum (ST1: 248 unknowns) | equal | exact | code verification |
| R1 | c2 | the part case of A with `modes = 5`, `interface_enrichment = 1`: the part file's bytes against `to_text` of the library part with `interface_enrichment(1)`; the part has at least one residual vector and as many as the library's | equal bytes | acceptance of F-616 | equal | exact | code verification |
| R2 | c2 | the same case without the key, and with `interface_enrichment = 0`: both files equal the library part's text without the option, and differ from R1's file | equal bytes; R1's differs | acceptance of F-616; ADR 0532 D7 | equal | exact | code verification |
| R3 | c2, c3 | arrangement I with the enriched part of R1 from its file: static `coordinates` and the eight modal `eigenvalues` and `vectors` against the library with the enriched part | equal in bits | acceptance of F-616 | equal | exact | code verification |
| R4 | c2 | each error of D7 on the reduction side, one case text each: `interface` missing, `interface` unknown, `[analysis]` given, `[solver]` given, `[superelements.x]` given, `output.vtu` given, `modes = 100000` (above the range), `modes = 500` (above the interior size of A, 189: the library's refusal) | each refused with the key registered in D7 and its line | acceptance of F-386 (located errors, ADR 0438) | refused | exact | code verification |
| D1 | c5 | digests recorded on main fcc84a84 before any library line, of six case files of `tests/solid_cases/` run by `run_solid_case_file` (`bar_hex8`, `cantilever_hex8`, `cantilever_tet10`, `cantilever_tet10_amg`, `gmsh_uniaxial`, `box_exact_bounds`): the bytes of the `.vtu`, the report without its `outputs`, and the bits of the displacements or eigenvalues and modes; equal after the change | equal | rule 3; [tolerance table](../process/tolerances.md), bit-identity of default output | equal | exact | unchanged default output |
| S1 | c4, c5 | the digest, recorded on main fcc84a84, of the JSON schema of `solid_case::schema()` with the top-level properties `superelements` and `reduction` removed (a removal that does nothing on main); equal after the change: every entry of main is unchanged and the diff is the two added tables | equal | rule 3, the class of additive edits | equal | exact | unchanged default output |
| D2 | c5 | `sb_solid_case` and `sb_solid_case_iterative`, unedited | pass | rule 3 | pass | exact | unchanged default output |
| D3 | c5 | `baseline_hashes`, `case_hashes_w48u` and `w48v_case_hashes`, unedited | pass | rule 3 | pass | exact | unchanged default output |

Records (reported, not in the decision rule): the sizes of the combined models; the number of residual vectors of R1;
the difference of B5; the schema diff of S1 as text.

Negative controls: K2, K3 and R4 (refusals); R2's second half (the file with the enrichment differs from the one
without, so R1 does not pass on a key that does nothing).

Planned mutations (rule 16), on a scratch copy with its own target folder; each must be caught by a registered check:

| Id | Mutation | Expected to be caught by |
|---|---|---|
| M1 | `reduction.interface_enrichment` not passed to `CmsOptions` | R1, R3 |
| M2 | `reduction.modes` not passed (0 used) | B1, B2, B3, R1 |
| M3 | the parts not attached (the host solved alone through the combined model) | B1 to B6 |
| M4 | the parts attached in descending order of their names | B4 |
| M5 | the check that a boundary unknown lies on the named set removed | K2 (the set `xmax`) |
| M6 | the check that every node of the set is held by the part removed | K2 (the set `all`) |
| M7 | `run_solid_case_file` runs a case with superelements as if it had none | K3 |
| M8 | the modal run of a case with superelements asks one mode more than `analysis.modes` | B3, R3 |

The committed one: M1, as `crates/rcfd-api/tests/f616_superelement_case.mutation.patch`.

Size (agent): done; 0; 0; 1; 160000

## Decision rule

**done** if the case-file bar equals the library's in bits (B1 to B4 and R3 met), a case without the keys is unchanged
(D1, D2, D3 met) and the schema diff is additive (S1 met), with K1 to K3, B5, B6, R1, R2 and R4 met, every mutation
caught and no amendment after a run. **partial** otherwise (a check missed, a mutation not caught by a registered check,
or a check met only after an amendment made after its run). **todo** if the keys cannot be built. "The bar of ADR 0536"
is here its static solve and its modal analysis, the two analyses a solid case file has; the harmonic solve is open
question 1, and its answer may read the same result as partial.

## What is not covered

- The direct harmonic solve of ADR 0536 from a case file (the solid case file has no harmonic analysis; open question 1).
- The beam and shell set-ups of ADR 0536: the case file's generator makes Hex8, Tet4 and Tet10 boxes only.
- The other options of the reduction: the cutoff, the residual vector of the interior load and the load patterns (ADRs
  0474 and 0511) have no key. ADR 0532 reached its limits with the cutoff and the load's vector beside the enrichment;
  a case file can ask the enrichment with a mode count only.
- The recovery of a part's interior, stresses and reactions of a combined run; a CG solver for the combined model.
- A CLI subcommand or MCP tool (there is none for any solid case, F-405).
- No golden is edited and none needs to be (finding 1).

## Open questions for the maintainer

1. **The bar of ADR 0536 has a direct harmonic solve; the solid case file has no harmonic analysis. Does "the bar of
   ADR 0536 run from a case file" include it?** Not taken (rule 6: it changes the scope of the item). Options:
   (a) the bar from a case file is its static solve and modal analysis, the analyses a solid case file has; the
   harmonic solve of a combined model from a case file becomes part of the item that gives the solid case file a
   harmonic analysis; (b) F-616 is partial until a harmonic analysis with superelements can be asked by a case file,
   built in a follow-up of F-616 (a table for the frequencies, the loss factors and the harmonic load, for combined
   models only). Recommendation: **(a)**. A harmonic analysis of the case file is a line of work of its own (loads with
   phase, damping, the response outputs) and should not exist for combined models alone.

## Amendment 1 (2026-11-23, before any code and before any run)

Written after reading `crates/rcfd-core/src/settings.rs` (`Schema::optional`, `to_json_schema`) and before the test file
of D1 and S1 exists. Nothing was run. Two corrections of the set-up; no limit changes.

1. **S1.** The JSON schema of an object lists every field that is not optional in its `required` array, so a new
   top-level table also adds its name there. S1 removes the names `superelements` and `reduction` from the top-level
   `required` array as well as from the top-level `properties` (both removals do nothing on main). The diff reported is
   then: the added properties, and the added names of `required`.
2. **D2 and D8.** The table `[reduction]` is an optional object of the schema (`Schema::optional`, ADR 0155): it stays
   out of the settings tree of a case that does not give it, and out of `required`. The collection `superelements`
   cannot be optional (the option is for objects) and is in the tree as an empty table. So the tree of a case without
   the keys gains `superelements` (empty) only, not `reduction` as D8 said.

## Amendment 2 (2026-11-23, before any library line and before the run of D1)

The test file of D1 and S1 was written and run once on the code of main fcc84a84 to print the digests it records (log
`d1-record-1.log` in the scratch folder). One of the six case files does not give a result on main:
`cantilever_tet10_amg.toml` is refused by `run_solid_case_file` with one problem at `solver` (the CG solver with the
`amg_quadratic` preconditioner does not converge to 1e-10; the existing known miss of the rows of ADR 0438's A14). The
set-up of D1 is corrected, with no change of a limit: for a case file that `run_solid_case_file` refuses, D1 records one
digest, of its problems as text (key, line and message of each, in order), in the place of the three digests; that is
what a case without the keys gives for this file, and it must be the same after the change. The other five files and S1
are as registered. The print of that first run gave digests for `bar_hex8`, `cantilever_hex8` and `cantilever_tet10`
and for S1, and stopped at the refused file; the recording run is made again with the corrected file and its digests
are committed with it, before any library line.

## Addendum 1: results (2026-11-23)

Commits on `f616-superelement-case-keys`: 07331090 (this registration, alone), 7a61ccee (amendment 1), 7f483a94
(amendment 2), 02d907d7 (the test file of D1 and S1 with the digests of main fcc84a84, before any library line),
8ae821ef (`solid_case.rs`, the checks `f616_superelement_case.rs`, the page), then the mutation patch, this addendum and
the proposal file. Each registered check was run once after 8ae821ef, release profile, in the scratch folder's own
target (script `run1.sh`, logs `run1-*.log`, summary `run1-summary.log`). No amendment was made after a run.

| Id | Limit | Measured | Met |
|---|---|---|---|
| K1 | equal bytes | the part of the part case and the part of the host's setup equal the library part's text; one entry `half`, `a.cms`, `xmin` | yes |
| K2 | each refused with its key and line | ten of ten refused at the registered key and line (the control case runs) | yes |
| K3 | refused, names `run_solid_case_any` | both refused (`superelements` line 21, `reduction` line 20); no part file written | yes |
| B1 | equal in bits | 248 coordinates and 243 host displacements, 0 differ | yes |
| B2 | equal in bits | 221 coordinates and 243 host displacements, 0 differ | yes |
| B3 | equal in bits | I: 8 eigenvalues and 1984 vector entries, 0 differ; II: 8 and 1768, 0 differ | yes |
| B4 | equal in bits | 253 coordinates and 243 host displacements, 0 differ; parts in the order `a_root`, `b_tip` (part 1 from unknown 248) | yes |
| B5 | `<= 1e-10` | 6.353e-13 over 81 nodes | yes |
| B6 | equal | `unknowns` 248; entry 27, 5, 243; `.vtu` with `displacement` (static) and `mode_8` (modal); eight eigenvalues of the report equal in bits | yes |
| R1 | equal bytes | the file equals the library part's text (314745 bytes); 27 residual vectors in the run and in the library | yes |
| R2 | equal bytes; R1's differs | without the key and with 0: both equal the library's text without the option (143980 bytes); R1's file differs | yes |
| R3 | equal in bits | 275 coordinates, 8 eigenvalues, 2200 vector entries, 0 differ | yes |
| R4 | each refused with its key and line | eight of eight refused at the registered key and line (the control case writes `part.cms`) | yes |
| D1 | equal | six of six files: 16 digests equal (five files with three each; `cantilever_tet10_amg` refused with the same problem, amendment 2) | yes |
| S1 | equal | digest 0xc611735ab88857f7 (7712 bytes) before and after | yes |
| D2 | pass | `sb_solid_case` 8 passed, 4 ignored; `sb_solid_case_iterative` 3 passed, 1 ignored; neither edited | yes |
| D3 | pass | `baseline_hashes` 3 passed, 1 ignored; `case_hashes_w48u` 1 passed; `w48v_case_hashes` has one test, ignored (a print): run with `--ignored` here and on a copy of main fcc84a84, 43 lines, identical | yes |

Records: combined models of 248 (I), 221 (II), 253 (two parts) and 275 (I with the enriched part: 27 residual vectors)
unknowns. B5 equals ST1 of ADR 0536 (6.35e-13).

**Golden files.** No golden file holds the solid case's schema, so there is no "additive golden edit" for the lead to
re-record, and no golden or hash table was edited. The diff of the schema itself (`solid_case::schema()` as JSON,
pretty-printed, main fcc84a84 against 8ae821ef; files `schema-main.json`, `schema-branch.json`, `schema.diff` in the
scratch folder) is additive: 66 lines added, none removed, and one line changed only by its closing comma (`"output"`
becomes `"output",` in the top-level `required`, before the new `"superelements"`). Added: the property `reduction`
(an optional object with `file`, `interface`, `interface_enrichment`, `modes`), the property `superelements` (named
objects with `file`, `set`) and the name `superelements` in `required`.

Mutations (scratch copy `mut` of 8ae821ef with its own target, script `mutate.py`, logs `mutations.log`,
`mut-M*.log`), all eight caught:

| Id | Caught by |
|---|---|
| M1 | R1 (the file, the residual vectors, the report), R2 (R1's file no longer differs), R3 |
| M2 | B1, B2, B3, B4, B6, K1, R1, R2, R3, R4 (`modes = 500` no longer refused) |
| M3 | the tests of B1 to B6 and R3 fail without a printed limit: the combined model is the unsupported host alone and the run refuses or stops; K2's control fails |
| M4 | B4 |
| M5 | K2 (the set `xmax`) |
| M6 | K2 (the set `all`) |
| M7 | K3 (both) |
| M8 | B3, B6, R3 |

Committed: M1 as `crates/rcfd-api/tests/f616_superelement_case.mutation.patch` (its command runs the test
`r1_r2_r3_interface_enrichment`; `git apply --check` passes on 8ae821ef).

Also run: `cargo test -p rcfd-api --release --lib` (159 passed, 4 ignored); `cargo test -p rcfd-cli --release --test
docs_site` (2 passed); `cargo fmt -p rcfd-api --check` clean; `cargo clippy -p rcfd-api --all-targets --release -- -D
warnings` clean. No file of `rcfd-fem` was changed.

### Deviations

1. Two amendments, both before any library line and before the run of the check they correct (S1's `required` list and
   the optional `[reduction]` table; D1's refused case file).
2. The test file of D1 and S1 was run on the code of main three times before its commit, to print and confirm the
   digests (the first stopped at the refused file, the next two gave the same digests); its registered run is the one
   after the change.
3. The variants of `CaseRun` hold their runs in a `Box` (clippy's `large_enum_variant`); a `SuperelementEntry` has
   `file` (as given) and `path` (the file read). D5 named neither detail.
4. M3 is caught through a refusal and a stopped test, with no printed limit reached.
5. K2's row "a host constraint value" and R4's row "modes above the range" check the key and the line, not a text of
   the message.
6. D3: the third file runs nothing by default; it was run with `--ignored` on both codes and compared by hand.

Not run: the workspace suite and the integration tests of `rcfd-fem` (no file of it changed); nothing of the harmonic
solve (open question 1).

Size (agent), actual: done; 0; 0; 1; 200000

### Decision

**done** by the registered rule: B1 to B4 and R3 equal the library's in bits, D1 to D3 and S1 are met, every other
check is met, all eight mutations are caught, and no amendment was made after a run. The rule reads "the bar of ADR
0536" as its static solve and modal analysis; open question 1 stands and its answer (b) would read this result as
partial.

### Open questions for the maintainer

1. Unchanged from the registration: whether "the bar of ADR 0536 run from a case file" includes its direct harmonic
   solve, which no solid case file can ask. Options (a) the bar from a case file is the static solve and the modal
   analysis, and the harmonic solve of a combined model comes with the item that gives the solid case file a harmonic
   analysis; (b) F-616 is partial until then. Recommendation: (a).

## For the lead: roadmap entries

Status proposed: done.

> ADR 0785, 2026-11-23, done: a solid case file attaches reduced part files to node sets of its model by [superelements.<name>] (file, set; parts in name order, each checked to lie exactly on its set) and reduces its model to a part file by [reduction] (interface, modes, interface_enrichment, file), run by rcfd_api::solid_case::run_solid_case_any; the solid bar of ADR 0536 from case files equals the library in bits in the static solve and the modal analysis for both arrangements (248 and 221 unknowns), for two parts on one host (253) and with one block of enrichment (27 vectors, 275), and the part files equal the library's byte for byte; 18 refusals located by key and line; a case without the tables is unchanged (16 digests of six case files, baseline_hashes, the case hash tables); no golden holds the solid schema, whose own diff is 66 added lines and none removed; 8 of 8 mutations caught. Not covered: the harmonic solve (the case file has none; open question 1 of the ADR), beams and shells, the cutoff and residual vectors of the reduction, stresses of a combined run.

Follow-up proposed (number left to the lead), title "Case-file keys for the cutoff, the load's residual vector and the load patterns of the reduction", deps F-616:

> Every limit not stated here is registered in the ADR before the run. The table [reduction] of the solid case file gains keys for the cutoff and the residual vector of the interior load of ADR 0474 and for load patterns of ADR 0511 named by entries of the case's loads; a part file made from a case file with each key equals the library's byte for byte, the configuration E1 of ADR 0532 (cutoff 32964 rad/s, the load's vector, one block of enrichment) is made from a case file and gives the library's frequencies in bits, and a case without the keys gives every result bit-identical.

## The lead's reading (2026-11-23), at the merge

Merged at e3d7fd0b and pushed at c33115a6 (the report read at 04:35 UTC, 6 minutes of reading, proposed done, written
done). Checked by the lead before the merge: the registration is the first commit and alone, the two amendments and the
digest tests of main come before any library line, the branch changes only `solid_case.rs`, new test files, the page,
the ADR and the proposal, and no golden or hash table is edited (no golden holds the solid schema). 144 binaries were
left out for a left-out run.

Status: **done**, as proposed, reading the bar of ADR 0536 as its static solve and modal analysis. Open question 1
(whether the harmonic solve is part of it) changes the item's scope and is put to the maintainer with the agent's
recommendation (a), which the lead shares; answer (b) would make this result partial. The proposed follow-up, keys for
the cutoff, the load's residual vector and the load patterns of the reduction, is also the maintainer's to enter; not
entered.

## The maintainer's answer (2026-11-23)

To the lead's question with the two open points of this ADR, the maintainer answered "Yes to both F-616 questions":

1. Open question 1: **(a)**. The bar of ADR 0536 from a case file is its static solve and modal analysis; the harmonic
   solve of a combined model from a case file belongs to the item that gives the solid case file a harmonic analysis.
   F-616 stays **done**.
2. The proposed follow-up, keys of `[reduction]` for the cutoff, the load's residual vector and the load patterns, is
   entered as F-889 with the title, dependencies and acceptance of the proposal file as written.
