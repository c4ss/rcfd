# ADR 0244: F-237 registration: heat-rate load export on shell decks and its tube round trip

Status: **REGISTERED 2026-11-12**, written before any code of F-237 and before any run of its checks. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parent: [ADR 0051](0051-wave27c-viscoelastic-structural-export.md) (wave 27C: `rcfd-io::registry`, `rcfd-io::nastran::Deck`,
`rcfd-post::structural`, decisions D7 to D9, checks R1 to R7, addendum A). Roadmap item F-237 (deps F-119, F-103, F-072; status
partial).

Number: the lead gave 0244 to F-237. Branch `f237-load-export` from main 9ea5ea2a, worktree `w49i-work/f237`.

House rules (as ADR 0236 and 0242):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written (and committed) before the corrected run, and the old one
  stays as an ignored record;
- no case-schema key, no new field of an existing struct (`Deck` is built with struct literals in `rcfd-io/src/registry.rs` and in
  the existing test `rcfd-io/tests/w27c_registry.rs`, so a new field would force an edit of an existing test);
  `crates/rcfd-api/src/sim.rs` and `schema.rs` are not touched; `crates/rcfd-api/tests/baseline_hashes.rs`, hash tables and
  goldens are not edited or re-recorded; no existing test is edited;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, `Cargo.lock` unchanged, no `unsafe`, the crate graph unchanged (`rcfd-post` already depends on
  `rcfd-io` and `rcfd-fv`), unsigned commits, no trailers, no push.

## What F-237 asks

Acceptance (quoted, binding): "Exported pressure and temperature loads on a tube reproduce the integrated force and heat rate to
0.1 percent after round-trip import; a public CGNS example round trip preserves node count and field values exactly; each listed
format has a pass/fail conformance case".

## What was read and computed before this text (disclosed)

- **Read:** the roadmap entry F-237 (title, deps, status, acceptance, note); ADR 0051 in full; `crates/rcfd-io/src/nastran.rs` and
  `crates/rcfd-post/src/structural.rs` in full; `crates/rcfd-post/tests/w27c_structural.rs` lines 1 to 135 (the R3 and R4 tube and
  plane set-ups); the struct literals of `Deck` in `rcfd-io/src/registry.rs` and `rcfd-io/tests/w27c_registry.rs`;
  `rcfd_post::integrals::conductive_heat_flow` and its unit test; the section "Format registry and structural load deck" of
  `docs/interchange-formats.md`; ADR 0242 for the style.
- **Searched:** the whole repository (`*.md`, `*.rs`, `*.json`) for `QBDY`, `heat rate`, `heat_flux`, `DFLUX`, `CalculiX`,
  `Abaqus`. No repository document describes any finite-element heat-flux card layout (the only mentions are "Heat-rate (QBDY)
  loads are not delivered" in ADR 0051 and `interchange-formats.md`). The heat flux exists on the solver side only as a boundary
  condition (`heat_flux_value`, `heat_flux_expression`, `heat_flux_udf`) and as the integral `conductive_heat_flow`.
- **Nothing was run.** No code of this ADR existed when this text was written.
- **Arithmetic (python, one line, before any code):** the facet area of a cylinder face spanning `dtheta` is
  `2 r sin(dtheta/2) dz = r dtheta dz k_n` with `k_n = sin(pi/n)/(pi/n)` for `n` equal chords. Hence
  `1 - k_64/k_96 = 2.2311e-4` and `k_96/k_64 - 1 = 2.2316e-4` (see the estimates below).

## What exists and what is missing

Exists (ADR 0051, unchanged here):

- `rcfd-io::registry`: 14 supported formats each with a pass/fail conformance case; CGNS and HDF5 registered `NotSupported`.
  The third clause of the acceptance is met.
- `rcfd-io::nastran::Deck`: free-field `GRID`, `CTRIA3`, `CQUAD4`, `CHEXA`, `PLOAD4`, `TEMP`, `FORCE`, `MOMENT`, bit-exact round
  trip, `line N` errors; the reader rejects any other card.
- `rcfd-post::structural`: parameter-space overlap weights (plane, tube), pressure mapping, `PLOAD4` shell deck, force and moment
  of fluid and deck; nodal temperature by remap plus linear recovery, `TEMP` deck. Measured (ADR 0051 addendum A): tube force
  `|dF|/S = 1.55e-4` against `1e-3`: the force part of the first clause is met.

Missing:

1. **The heat-rate part of the first clause** ("reproduce the ... heat rate to 0.1 percent after round-trip import"). Nothing
   carries a heat rate: `TEMP` carries nodal temperatures, which do not determine a heat rate without the structure's
   conductivity and the wall-normal gradient. This item's job: a heat-flux load on shell elements, its mapping from the fluid wall,
   its deck text and reader, and the tube round trip check.
2. The public CGNS round trip (second clause): not this job (no CGNS reader or writer exists, and a CGNS file is an HDF5 or ADF
   container; it needs a format implementation of its own; listed as open, unchanged).
3. Case-tree wiring and third-party solver acceptance: not this job (wiring needs `sim.rs`/`schema.rs`; third-party acceptance is
   not testable here).

## Design (registered)

### D1 Interpretation of the acceptance

"Pressure and temperature loads ... reproduce the integrated force and heat rate": the force comes from the pressure loads (exists),
the heat rate from a thermal surface load. A heat rate cannot be recovered from a temperature load alone, so the thermal load that
reproduces the heat rate is the wall heat flux, exported as a per-element surface heat flux on the shells. Taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation). The nodal temperature export (`TEMP`) stays as it is.

### D2 The card: an rcfd card, specified in this repository, not a vendor card

No document of the repository specifies a finite-element heat-flux card, and the house rule forbids a layout remembered from a
vendor manual. The heat flux is therefore written as a card defined by this repository, with a name that does not claim to be any
vendor's card:

```
RCFDQ,SID,EID,Q
```

- `SID` load set id (positive integer), `EID` id of a `CQUAD4` or `CTRIA3` element defined earlier in the deck, `Q` the heat flux
  per unit area [W/m^2], uniform over the element, written with `{:?}` (shortest round-trip form of an `f64`, as every real of the
  deck), so the round trip is bit-exact.
- Sign: positive `Q` is heat flowing from the fluid into the structure (the heat leaving the fluid domain through the wall, the
  sign of `conductive_heat_flow`). Element heat rate `Q A_e`, `A_e = |area vector|` of the element facet (the same `quad_geom` as
  the force).
- Placement: after the `TEMP`, `FORCE` and `MOMENT` cards and before `ENDDATA`; one card per element.
- Format specification: this ADR and the section "Format registry and structural load deck" of `docs/interchange-formats.md`
  (extended by this item). It is the repository's own format; no third-party solver reads it, and that is stated there.

Which vendor card should carry the heat flux (and its sign and element type, which in common solvers need surface heat-transfer
elements of their own) is an **open question**: it needs a specification the repository does not have (see the remaining
questions).

### D3 Library in `rcfd-io::nastran` (new items only)

- `pub struct HeatFlux { sid: i64, eid: i64, q: f64 }`.
- `pub fn write_with_heat_flux(deck: &Deck, heat: &[HeatFlux]) -> Result<String>`: `deck.to_string_checked()`, then the `RCFDQ`
  cards inserted before the final `ENDDATA`. With `heat` empty the text is byte-identical to `deck.to_string_checked()`.
  Validation: positive `SID`, `EID` an existing `CQUAD4` or `CTRIA3`, finite `Q`, at most one card per `(SID, EID)`.
- `pub fn parse_with_heat_flux(text: &str) -> Result<(Deck, Vec<HeatFlux>)>`: the `RCFDQ` lines are replaced by comment lines (so
  every line keeps its number), the rest goes to `Deck::parse` unchanged, then the `RCFDQ` lines are parsed with `line N` errors
  (missing field, non-integer, non-number, non-finite, undefined element, element not a shell, duplicate `(SID, EID)`, card after
  `ENDDATA`).
- `Deck::parse` is unchanged: it still rejects `RCFDQ` as an unknown card (the new card is opt-in and the old reader is strict).

### D4 Library in `rcfd-post::structural` (new items only)

- `wall_heat_flux(geom: &FvGeometry, patch, t: &ScalarField, k) -> Result<Vec<f64>, String>`: per face of the patch
  `q_f = -k dT/dn` with `dT/dn` the outward normal gradient from the temperature boundary coefficients, the same expression as
  `rcfd_post::integrals::conductive_heat_flow` before its multiplication by `|S_f|`. Then `sum q_f |S_f|` is the solver's
  integrated heat flow (up to summation order).
- `fluid_heat_rate(mesh, patch, q) -> Result<f64, String>`: `sum q_f |S_f|`.
- Two mappings of the face heat flux onto the shell elements, both from the ADR 0051 overlap weights `w_ts = |s cap t|/|t|`
  (parameter space):
  - consistent: `q_t = sum_s w_ts q_s` (the existing `map_face_field`, reused as is): a uniform flux is exact, the heat rate is
    conserved only up to the ratio of the area densities `A/|.|_param` of the two tessellations;
  - conservative (new, `map_heat_flux_conservative(weights, src_rects, src_areas, tgt_rects, tgt_areas, q)`): the heat rate
    `Q_s = q_s A_s` of each source face is split over the targets in the proportion `|s cap t|/|s| = w_ts |t|/|s|`, so
    `Q_t = sum_s w_ts (|t|/|s|) q_s A_s` and `q_t = Q_t / A_t`. When the target surface covers every source face,
    `sum_t |s cap t| = |s|` and `sum_t Q_t = sum_s Q_s` exactly in exact arithmetic (first principles; the same argument as the
    conservative remap of ADR 0051). The price: a uniform flux becomes `q0 (A_s/|s|)/(A_t/|t|)` on the target.
- `shell_areas(surface) -> Vec<f64>`: `|area vector|` of each quad (as `quad_geom`).
- `shell_heat_flux(surface, q, sid) -> Result<Vec<HeatFlux>, String>` (element ids from 1, as `shell_deck`).
- `deck_heat_rate(deck, heat, sid) -> Result<f64, String>`: `sum Q A_e` over the `RCFDQ` cards of set `sid`, `A_e` from the deck's
  own grids (so it measures what an importer would see).

### Derivation of the estimates

Tube: fluid `annulus_3d(96, 1, 24, 0.02, 0.05, 1, 0, Uniform)`, patch `outer` (r = 0.05, 96 chords); shell `shell_tube(0.05, 0, 1,
64, 20)` (64 chords). Both cover `theta in [0, 2 pi) x z in [0, 1]`. Area densities: `A_s = r k_96 |s|`, `A_t = r k_64 |t|`.

- consistent: `sum_t q_t A_t = r k_64 sum_s q_s sum_t |s cap t| = r k_64 sum_s q_s |s|`, the fluid heat rate is
  `r k_96 sum_s q_s |s|`: relative error `1 - k_64/k_96 = 2.231e-4` for ANY flux distribution (signed difference: the deck
  heat rate is the smaller). Limit `1e-3` (the acceptance) holds with a factor 4.5.
- conservative: heat rate equal up to rounding: about 1536 source faces each split over at most 4 targets, sums of positive terms,
  relative rounding about `1e-15`; limit `1e-12`.
- uniform flux, conservative: every element `q_t/q0 = k_96/k_64`, deviation `2.232e-4`; limit `1e-3`.
- plane (`box_mesh([8, 8, 1], [1; 3])`, patch `zmax` onto `shell_plane` 5 x 7 and 13 x 11): equal area density (`A = |.|_param`),
  so both mappings conserve to rounding; limit `1e-12`.

## Registered checks (limits fixed now)

`rel(a, b) = |a - b| / |b|`. Test file `crates/rcfd-post/tests/f237_heat_rate.rs` (H1 to H5, H7, NC1, NC2) and
`crates/rcfd-io/tests/f237_heat_deck.rs` (H6, H8, NC3, NC4). Release profile.

| Check | What | Limit | Estimate behind the limit |
| --- | --- | --- | --- |
| H1 (acceptance) | tube 96 x 24 -> 64 x 20, `q = 2e4 (1 + 0.5 cos theta + 0.3 z)` W/m^2, consistent mapping, written, parsed by `parse_with_heat_flux`, `deck_heat_rate` vs `fluid_heat_rate` | rel <= 1e-3 | 2.231e-4 (chord ratio) |
| H2 | the same with the conservative mapping | rel <= 1e-12 | rounding, ~1e-15 |
| H3 | uniform `q0 = 1.7e4`: consistent, every element `|q_t/q0 - 1|` | <= 4 eps | the shifted average of ADR 0051 (exact, 0 measured for pressure) |
| H3b | uniform `q0`, conservative, every element `|q_t/q0 - 1|` | <= 1e-3 | 2.232e-4 |
| H4 (solver's integrated value) | annulus as H1, `T` cell values `350 + 10 cos theta_c`, outer patch fixed per face `300 + 20 (1 + 0.5 cos theta + 0.3 z)`, `k = 0.6`: (a) `fluid_heat_rate(wall_heat_flux)` vs `conductive_heat_flow`; (b) conservative export round trip vs `conductive_heat_flow`; (c) consistent export round trip vs `conductive_heat_flow` | (a) rel <= 1e-12, (b) rel <= 1e-12, (c) rel <= 1e-3 | (a) same formula, summation order only; (b) as H2; (c) 2.231e-4 |
| H5 | plane 8 x 8 `zmax`, `q = 1 + 2x + 3y`, onto 5 x 7 and 13 x 11, both mappings, round trip | rel <= 1e-12 | equal area density, rounding |
| H6 | round trip of a deck with `GRID`, `CQUAD4`, `CTRIA3`, `PLOAD4`, `TEMP` and 5 `RCFDQ` cards (values incl. `1/3`, `1e-300`, `-2.5e17`, two sets): `Deck` equal and every `HeatFlux` bit-identical (`to_bits`); with no heat cards `write_with_heat_flux(d, &[])` byte-identical to `d.to_string_checked()` | exact | `{:?}` is a round-trip format |
| H7 | the pressure part on the same tube in the same run as H1 (force through the same text): `|dF|/S` | <= 1e-3 | 1.55e-4 measured in ADR 0051 (same set-up) |
| H8 | located errors: `RCFDQ` with a missing `Q`, non-numeric `Q`, `inf`, an undefined element, a `CHEXA` element, a duplicate `(SID, EID)`, after `ENDDATA`; a malformed `GRID` after an `RCFDQ` line keeps its line number; writer: non-finite `Q`, missing element, non-positive `SID` | each `Err` names the line (`line N`, N checked) or the item | - |

Negative controls (each must show the check can fail):

- NC1 the consistent mapping onto a shell tube of the wrong radius 0.045 (fluid at 0.05): H1's metric must exceed `1e-3`
  (estimate: area ratio 0.9, error about 0.1).
- NC2 the conservative mapping of the full H1 source onto a half-length shell `shell_tube(0.05, 0, 0.5, 64, 10)`: every target
  is covered (so `overlap_weights` accepts it) but the sources with `z > 0.5` are not; H2's metric must exceed `1e-3` (estimate:
  the heat rate of `z < 0.5` is `0.5375/1.15 = 0.467` of the total, miss about 0.53). It shows the conservation of H2 rests on full
  coverage of the source, which `overlap_weights` does not check.
- NC3 the old reader `Deck::parse` on an `RCFDQ` deck returns `Err` containing `unknown card 'RCFDQ'` (opt-in, old reader unchanged).
- NC4 one digit of one `Q` changed in the text: the parsed heat rate differs from the written one (rel > 0) and the bit check of H6
  fails for that card.

Planned mutations (applied to a scratch copy one at a time, the relevant tests run, then reverted; recorded in the addendum):

- MU1 conservative mapping without the factor `|t|/|s|` (it becomes the consistent one): H2 and H4(b) must fail.
- MU2 `deck_heat_rate` skips the last card: H1, H2, H5 must fail.
- MU3 the writer formats `Q` with 6 significant digits (`{:.6e}`): H6 must fail.
- MU4 `wall_heat_flux` with the sign flipped: H4(a) must fail.
- MU5 the reader swaps `SID` and `EID`: H6 (and H8 undefined-element message) must fail.

## Decision rule

- **done** (for this item's missing heat-rate part): H1 to H8 met, NC1 to NC4 behave as stated, every mutation caught by at least
  one check. The roadmap item as a whole stays **partial** even then, because the CGNS clause of the acceptance is not met (no CGNS)
  and the case-tree wiring is not delivered; the proposal will say "heat-rate clause met".
- **partial**: H1 met but any other check missed (known miss, diagnosed).
- **todo / no progress**: H1 missed.

## Limits of the claim

- The heat flux is a uniform value per shell element; the corner-varying form is not written.
- `RCFDQ` is the repository's card; no third-party solver reads it, and the mapping to a vendor heat-flux card is open.
- H4 uses the solver's boundary-flux expression and its integral `conductive_heat_flow` on a prescribed temperature field, not an
  energy-equation solve; the conservation it shows is that of the export, the solver's own energy balance is out of scope.
- Only tensor-product parametrised surfaces (plane, cylinder), as ADR 0051.
- No case key, no CLI or MCP surface.

## Addendum 1: results

Written after the single registered run (`cargo test --release -p rcfd-io --test f237_heat_deck` and
`cargo test --release -p rcfd-post --test f237_heat_rate`, both `--nocapture --test-threads=1`, at ce3705bb). No limit above was
changed; no test was re-run to pick a result.

| Check | Limit | Measured | Met |
| --- | --- | --- | --- |
| H1 (acceptance, heat rate) tube 96 x 24 -> 64 x 20, consistent, after write and parse | rel <= 1e-3 | 2.2311e-4 (fluid 7.2243734870e3 W, deck 7.2227616610e3 W) | yes |
| H2 tube, conservative, after write and parse | rel <= 1e-12 | 2.518e-15 | yes |
| H3 uniform, consistent, worst element | <= 4 eps (8.88e-16) | 0 | yes |
| H3b uniform, conservative, worst element | <= 1e-3 | 2.2316e-4 | yes |
| H4(a) per-face `wall_heat_flux` sum vs `conductive_heat_flow` (395.98203956 W) | rel <= 1e-12 | 0 | yes |
| H4(b) conservative round trip vs `conductive_heat_flow` | rel <= 1e-12 | 5.742e-14 | yes |
| H4(c) consistent round trip vs `conductive_heat_flow` | rel <= 1e-3 | 2.2311e-4 | yes |
| H5 plane 5 x 7 / 13 x 11, both mappings | rel <= 1e-12 | 2.538e-16 / 1.269e-16 (both mappings) | yes |
| H6 bit-exact round trip of 5 `RCFDQ` cards; empty heat list byte-identical | exact | exact, byte-identical | yes |
| H7 (acceptance, force) same tube deck, `|dF|/S` | <= 1e-3 | 1.552e-4 | yes |
| H8 located errors (9 reader cases incl. CHEXA, 3 writer cases) | `line N` / item named | all as registered | yes |
| NC1 wrong-radius shell (0.045) | must exceed 1e-3 | 1.0020e-1 | held |
| NC2 half-length shell, conservative | must exceed 1e-3 | 5.3261e-1 | held |
| NC3 `Deck::parse` on an `RCFDQ` deck | `unknown card 'RCFDQ'` | `line 12: unknown card 'RCFDQ'` | held |
| NC4 one digit of one `Q` changed | 1 card differs, rel > 0 | 1 card, 5.797e-3 | held |

The estimates were met as derived: H1, H4(c) and H3b sit on the chord-ratio numbers `1 - k_64/k_96 = 2.2311e-4` and
`k_96/k_64 - 1 = 2.2316e-4` to the printed digits; NC1 (`1 - 0.9 k_64/k_96 = 0.1002`) and NC2 (0.533) on theirs.

Deviation: H4(b) is 5.7e-14, about 20 times the estimated 1e-15 rounding (and the 2.5e-15 of H2); within its limit by a factor
17. Not diagnosed further; the likely source is summation order (`conductive_heat_flow` and the deck sum add the terms in
different orders and groupings), not the mapping (MU1 moves this number to 0.44).

Mutations (applied one at a time to a `git archive` copy of ce3705bb in the scratch folder with its own target directory; the
unmutated copy passed first; each mutation reverted after its run):

| Mutation | Caught by |
| --- | --- |
| MU1 conservative mapping without `|t|/|s|` | H2 (0.444), H3b, H4(b), H5 conservative |
| MU2 `deck_heat_rate` skips one card | H1 (1.25e-3), H2, H4(b), H4(c), H5 |
| MU3 `Q` written with `{:.6e}` | H6, NC4, H2 (1.8e-9), H4(b) (3.0e-8) |
| MU4 `wall_heat_flux` sign flipped | H4(a) (2.0) |
| MU5 reader swaps `SID` and `EID` | H6, H1, H2, H4, H5 |

Also run: `cargo test --release -p rcfd-io -p rcfd-post` (every test of both touched crates): 273 passed, 0 failed, 25 ignored
(existing ignored records). `cargo fmt -p rcfd-io --check` and `cargo fmt -p rcfd-post --check`: clean.
`cargo clippy --release -p rcfd-io -p rcfd-post --all-targets -- -D warnings`: clean (one `neg_cmp_op_on_partial_ord` lint was
fixed before the code commit, before any run).

Not run: the workspace test suite (by instruction); `crates/rcfd-api/tests/baseline_hashes.rs` (no solver path, no default and no
existing writer changed: the edits to existing files are new items appended to `nastran.rs` and `structural.rs`, one `use` line,
and the format document); any third-party solver.

Defaults: `Deck::to_string_checked` and `Deck::parse` are untouched; H6 shows `write_with_heat_flux(d, &[])` is byte-identical to
`to_string_checked`, NC3 that the old reader still rejects the new card. The new output is opt-in.

**Decision by the registered rule: done for the heat-rate part** (H1 to H8 met, NC1 to NC4 held, MU1 to MU5 caught). The first
clause of the acceptance is now met in both parts (force 1.55e-4 S, heat rate 2.23e-4 relative, against 0.1 percent, after a
round trip through the repository's reader). The roadmap item stays **partial**: the public CGNS round trip (second clause) is not
met and case-tree wiring is not delivered.

### Remaining questions

1. Vendor heat-flux card. Options: (a) keep `RCFDQ` as the repository's card; (b) place a specification of a vendor heat-flux
   card (fields, sign, the surface heat-transfer element it needs) from an open, citable source in the repository, then write that
   card beside `RCFDQ`; (c) export the heat flux to an open finite-element input format with a published specification.
   Recommendation: (b) once a citable open specification is in the repository, (a) until then. Not taken here: it needs a
   specification the repository does not have.
2. Default mapping for heat flux once it is wired into a case: consistent (uniform fields exact, heat rate off by the tessellation
   area ratio) or conservative (heat rate exact, uniform fields off by the same ratio). Recommendation: conservative, since the
   heat rate is what the acceptance and an energy balance need. Not taken: it is a case default.
3. Case-tree wiring of the load export (pressure, temperature, heat flux) needs `sim.rs`/`schema.rs`, a new case key and the
   CLI/MCP goldens: the remaining step, not done on this branch.
4. The `nastran_deck` conformance case of `rcfd-io::registry` does not exercise `RCFDQ`. Options: extend that case (changes an
   existing conformance case) or add a separate entry. Recommendation: extend it in the wiring step; not done here because it
   edits existing behaviour.

## For the lead: roadmap entries

Proposed status of F-237: **partial** (unchanged).

New note text for F-237:

> Priority P3. Size M. Area: io. WAVE 27C (ADR 0051) and ADR 0244, PARTIAL. Not delivered: public CGNS round trip (acceptance NOT met), case-tree wiring, third-party solver acceptance (not testable), deck card layouts partly unverified (PLOAD4/TEMP read, rest recalled, sign convention unverified), vendor heat-flux card (no specification in the repository). Format names stay in the format registry documentation, not in this item. rcfd-io registry (12 supported formats each with a conformance case, CGNS and HDF5 registered NotSupported). Nastran-style free-field deck (GRID, CTRIA3, CQUAD4, CHEXA, PLOAD4, TEMP; bit-exact round trip, line-located errors) plus the repository's own shell heat-flux card RCFDQ,SID,EID,Q (ADR 0244; opt-in writer and reader, old reader unchanged). rcfd-post structural: conservative parameter-space pressure mapping (plane and tube), nodal temperature via remap plus linear recovery, wall heat flux from the temperature boundary condition mapped consistently or conservatively in heat rate. Measured: Uniform pressure exact (0 vs 4 eps). Tube force error 1.6e-4 S (limit 1e-3 S; acceptance 0.1 percent met for force). Tube heat rate after round trip 2.23e-4 consistent, 2.5e-15 conservative (limit 1e-3; acceptance 0.1 percent met for heat rate); conservative export against conductive_heat_flow 5.7e-14. Plane force 2.5e-16, plane heat rate 2.5e-16. Moments within limits. Linear temperature exact to 2.4e-14 on 3 structural meshes.

Follow-up item (proposed; id at the lead's choice). Title: "Vendor heat-flux card for the structural load deck from a cited open
specification". Deps: F-237. Acceptance: "A specification of the card (fields, sign, required surface elements) from an open
source is placed in the repository; the deck writes it beside RCFDQ; the tube heat rate after a round trip through the repository
reader matches the fluid heat rate to 0.1 percent". Note:

> Priority P3. Size S. Area: io. Opened by ADR 0244: RCFDQ is the repository's own card; no vendor heat-flux card layout is specified in the repository, so none was written.
