# ADR 0713: F-791 registration: one growth ratio of the free rows of every C line across the span layers (construction S'''')

Status: **REGISTERED 2026-11-17, written before any code of F-791 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-791 was written; the only builds were the four disclosed probes on a
`git archive` copy of the base (below). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-791 (deps F-784), proposed by [ADR 0702](0702-f784-registration-ramp-length-of-the-wake-stack-remap-from-the-wake-line-spacing.md)
"For the lead" (follow-up) and its remaining question 1. Parents: ADR 0702 (construction S''',
`CGridOptions::camber_seam_s_wake_ramp_construction()`, `wake_ramp_from_spacing`, `CGridMesh::wake_ramp_length`, the test file
`f784_cgrid_wake_ramp.rs`, the miss R2-REF at 84.688 deg), [ADR 0699](0699-f782-registration-wake-stacks-of-construction-s-prime-at-the-thin-end-wall-layers.md)
(S''), [ADR 0696](0696-f777-registration-construction-s-without-the-first-cell-stretch-on-strongly-staggered-blades.md) (S', the
first free cell at the next stack height, the second blade B2, the scan), ADR 0692 (S, straight lines), ADR 0118 (the s4 specs),
ADR 0072 (W1, the remapped radius and the blend of the far grid), ADR 0068 (the capped line distribution). Number 0713 given by the
lead. Branch `f791-work` from ad931721 (F-726 / ADR 0692, F-777 / ADR 0696, F-782 / ADR 0699 and F-784 / ADR 0702 merged),
worktree `w49i-work/f791`, scratch `w49i-work/f791-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests and ignored records not
edited; defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

Clean-room: the centroid of a prism whose cross-section varies linearly along its height (elementary integration), the
repository's own constructions (`capped_cells` of ADR 0068, the S family of ADRs 0692 to 0702) and quality measures
(`quality.rs`: non-orthogonality is the angle between the face area vector and the vector between the two cell centroids). No
source outside the repository was read.

## What F-791 asks

> Every limit not stated here is registered in the ADR before the run. A new opt-in field of S''' builds the first free rows of the blade lines between the last thin and the first thick span layers so that, at the ADR 0118 s4 specs, ComputerFan::rom_reference() blade 0 and the ADR 0696 second blade build with no folded cell within 84.5 deg non-orthogonality (the 84.688 deg of ADR 0702 at C (45, 2) / (45, 3), layer 3 of the reference disclosed), skewness 4.0 and aspect 1000, the wake-stack spanwise skewness stays within 1.0, a registered front_gap and n_nose scan inside the camber_seam range builds a registered fraction, and camber_seam_h, camber_seam_s, camber_seam_s_unstretched, camber_seam_s_wake_remap and camber_seam_s_wake_ramp are unchanged in bits.

Clauses: **c1** every limit registered here; **c2** a new opt-in field of S''' that builds the first free rows of the blade lines
between the last thin and the first thick span layers; **c3** REF (reference blade 0) and B2 (the ADR 0696 second blade) at s4
build with no folded cell; **c4** non-orthogonality <= 84.5 deg on both (all faces); **c5** skewness <= 4.0 and aspect <= 1000 on
both; **c6** wake-stack spanwise skewness <= 1.0 on both; **c7** a registered front_gap / n_nose scan inside the camber_seam range
builds a registered fraction; **c8** camber_seam_h, camber_seam_s, camber_seam_s_unstretched, camber_seam_s_wake_remap and
camber_seam_s_wake_ramp unchanged in bits. From the brief: the S''' hashes computed on a `git archive` copy of the base before
any change; the worst faces diagnosed on the base first; the named existing tests rerun; negative controls and mutations
registered; the 84.5 deg limit is the maintainer's and is not relaxed.

## What was read and computed before this text (disclosed)

Read: ADR 0702 (all), `fan_cgrid.rs` (`capped_cells`, `graded_two_sided`, `remapped_radii`, `build_layer` (the stacks, the
straight lines and their distribution `rest = capped_cells(len, n_side - nb, first, false)`), `write_layer` (the blend of the far
grid from `a` to `b` over `fade(arc / len)`), the layer loop of `build_cgrid_mesh_with`), `quality.rs` (`compute_metrics`), the
test file `f784_cgrid_wake_ramp.rs` (its helpers are reused).

**Probes on the base (disclosed; `f791-scratch/base` = `git archive ad931721`, one added test file `zz_probe791.rs` and, for
probe 2 only, one `eprintln!` of the line length and distribution in `build_layer` behind an environment variable; logs
`f791-scratch/probe.log`, `probe2.log`, `probe3.log`, `probe4.log`; each run once).** They build only existing constructions; no
code of F-791 existed. They print:

- **The S''' hashes (c8):** REF `0xba8d86a18ca34784`, B2 `0x64432ac2c53ab55a`, 12 841 cells, construction step 5 (the algebraic
  grid: the hash equals that of the same options with `smoothing: false`). S'' REF / B2 `0x63011aa8288ac9c0` / `0x769e2e330b0cf295`
  and S' `0x0d257b2fcee093ca` / `0xefc53b8c413a845b` as in ADR 0702.
- **The span layers** (s4, both blades): cell heights 5.0e-5, 6.0e-5, 7.2e-5 m (cells 0 to 2, thin), 8.998e-3, 9.070e-3, 9.070e-3,
  8.998e-3 m (cells 3 to 6, thick), 7.2e-5, 6.0e-5, 5.0e-5 m (cells 7 to 9), then the gap. So the transition from 7.2e-5 to 9.0e-3 m
  (ratio 125) lies between cell layers 2 and 3 (and 6 and 7).
- **The faces above 84.5 deg** (REF S''', all on the face between the first and second free cells `C (i, 2) / (i, 3)`, `n_bl` =
  2): layer 3, lines 43 to 50 (84.512 to **84.688**, the maximum at line 45), and layer 5, lines 44 to 50 (84.508 to 84.612). Per
  line 45 and cell layer 0 to 9 this face is 80.03, 80.46, 80.97, **84.69**, 83.89, **84.60**, 84.28, 83.61, 83.65, 83.68 deg; the next
  face out (`C (i, 3) / (i, 4)`) is 79.8 to 83.6 deg, the stack-edge face 42 to 73 deg. B2: the same face family, maximum 84.218 at
  line 45, layer 5 (84.13 at layer 3). Outside that face family the largest are REF 84.397 (the first ramp face C (56, 2) / (56, 3),
  layer 3, the same family on the trailing-edge line), 84.329 (the same, layer 5), then 83.76 (the i-face C (44, 2) / (45, 2) of
  the gap layers 14 to 16); B2 83.79, 83.56, then 83.32 (gap layers). Skewness maxima REF 2.7609 at C (37, 7) / (38, 7) layer 6, B2
  2.8312; aspect 525.6 / 551.7 at C (59, 9) layer 16.
- **The decomposition of the worst face** (REF, C (45, 2) / (45, 3), layer 3; components (z, theta, r) at the face centre): unit
  area vector `n` = (0.952, 0.284, -0.114), unit centroid vector `d` = (-0.027, 0.704, **0.710**). The in-plane parts alone make
  75.7 deg; the angle reaches 84.69 deg because `d` has a radial component as large as its in-plane one (the cell centroids are
  1.6e-4 m apart radially and in plane), and `n` leans radially by 6.6 deg the other way (the row-3 nodes of node layer 4 lie 9.8e-4 m
  along the row normal from those of node layer 3, over 9.0e-3 m). In the thin layers `d` has no radial part (-0.004) and the face
  is 80 to 81 deg. Layer 5: `d` radial 0.377, 84.60 deg.
- **The radial centroid offset is the varying cross-section of the thick cells.** A prism of height `H` whose cross-section goes
  linearly from `A0` to `A1` has its centroid `H (A1 - A0) / (6 (A0 + A1))` above mid-height. With the in-plane quad areas of the
  two node layers this predicts the measured offsets of the cells C (45, 1..4) at layers 3 to 6 within 4 to 16 % (probe 3; e.g.
  layer 3: cell 2 measured 1.648e-4, predicted 1.461e-4 m; cell 3 3.457e-4 / 3.438e-4 m). The neighbouring cells' area ratios
  `A1 / A0` differ: layer 3, cell 2 1.216, cell 3 1.595; layer 5, 1.088 and 1.239. Their difference sets the radial part of `d`.
- **Its two parts** (probe 4, line 45, node layers 3 and 4): the area of each cell is (row-edge length) x (step along the line) x
  (sine of the slant between line and row). The row-edge ratio is common (1.397 for cells 1 to 4). (i) **The step of the second
  free cell changes with the layer**: 2.264e-4 m at node layer 3, 2.658e-4 m at node layer 4 (ratio 1.174), while the first free
  cell is 7.22e-5 m on both (ADR 0696: the next stack height). The steps come from `capped_cells(len, 8, first)` with `len` the
  line length from the stack edge to the seam, which grows with the pitch: REF line 45, 1.361e-2 m at the hub, 1.373e-2 (node
  layer 3), 1.991e-2 (4), 2.650e-2 (5), 3.195e-2 (6), 3.744e-2 (7), 3.755e-2 m at the tip. The solved growth ratio rises with it:
  3.19 at the hub, 3.20, 3.70, 4.21, 4.69, 5.13 (the cells grow over the first four free cells and are capped at 1.5 times the
  mean beyond). So every free cell but the first changes its size from layer to layer by `(rho_{k+1} / rho_k)^m`. (ii) **The slant
  of the first free cell changes more than that of the second** (sine ratio 0.870 against 0.973 between node layers 3 and 4): the
  rows beyond the stack blend from the true section `a` towards the remapped grid `b` (ADR 0072 W1, weight `1 - fade(arc / len)`,
  about 0.007 at row 3 and 0.1 at row 4), which bends the line at row 3 by a layer-dependent angle (1.3 deg at node layer 3, 0.2 deg
  at node layer 4). Part (i) is `ln 1.174 = 0.160` of the `ln (1.595 / 1.216) = 0.271` at layer 3 (59 %) and `ln 1.113 = 0.107` of
  `ln (1.239 / 1.088) = 0.130` at layer 5 (82 %); part (ii) is the rest.

Consequence, stated before the design: the face is the first face at which the layer-to-layer change of the cell size starts (the
stack and the first free cell do not change, the second does); in a thin layer the offset `H (rho - 1) / 12` is negligible, in a
thick one it is comparable to the in-plane distance of the two thin cells' centroids. Moving the start outwards only moves the
face (the offset does not depend on the cell size, the centroid distance does); what removes it is the same cell sizes in every
layer in the near-wall rows, with the change taken where the cells are large.

## Design (fixed now): construction S'''', `camber_seam_s_free_rows_construction`

One new field of `CGridOptions`, `free_row_hub_ratio: bool`, default `false` (so `Default`, every existing constructor and every
struct literal with `..` are unchanged). A new constructor `CGridOptions::camber_seam_s_free_rows_construction()` =
`camber_seam_s_wake_ramp_construction()` + `free_row_hub_ratio: true` (S''''). A new public field of `CGridMesh`,
`free_row_ratios: Option<Vec<f64>>` (the growth ratio used on every C line `i`, `None` without the field), for the report and the
checks.

With the field, in each fallback attempt:

1. The hub layer's true section (node layer 0, radius `r_hub`) is built first with the existing distribution; on every C line `i`
   its ratio `rho_i = (x_2 - x_1) / (x_1 - x_0)` of the free-row offsets `x` (the solved growth ratio of `capped_cells`, or the cap
   over the first cell where the second cell is already capped) is recorded. Lines with fewer than two free cells get none.
2. Every layer grid built afterwards (the true sections, the remapped grids `b` and the hub itself again) distributes the free
   cells of line `i` as `s_m = min(first rho_i^m, C)`, `m = 0 .. n - 1` (`n = n_side - nb_i`, `first` that layer's next stack height
   as before), with the cap `C` solved by bisection (200 halvings on `[0, first rho_i^(n - 1)]`) so that the cells fill that layer's
   line length `len`, then scaled by `len / sum` as `capped_cells` does. If `rho_i` is missing or below 1, or the uncapped cells do
   not reach `len`, the line keeps `capped_cells`.
3. Every C line is treated (blade, nose, trailing-edge, wake and outlet lines), in every span and gap layer, so that neighbouring
   lines and neighbouring layers keep one rule (a rule on some lines or layers only would put the change of the cell sizes on the
   faces between the treated and the untreated ones, and in a thin layer even a small in-plane change is a large tilt of the
   spanwise faces, ADR 0699). The stack nodes, the first free cell (ADR 0696), the line directions and targets, the seam and
   front-edge nodes, the F and core blocks and the wake-stack remap are not touched.

Derivation. With one `rho_i` the cells below the cap are `first rho_i^m` in every layer: they change from layer to layer only with
`first` (with `lean_stack` the stack heights vary by at most 1.3 % between the layers of REF, probe 2), so part (i) of the offset
vanishes in the near-wall rows. The change of the line length goes into the cap `C` alone, i.e. into the cells beyond the
growth, which are the largest of the line (2.6e-3 to 8.9e-3 m against 7e-5 to 2.4e-4 m for the first two free cells), so the same
radial offset there is a small angle (estimate below). The largest neighbour ratio of a line is at most `rho_i` (the cap is reached
when `first rho_i^m` would exceed it), against up to 5.1 now. The hub is the reference because its lines are the shortest (the
pitch grows with the radius), so its ratio is the smallest one and the caps of the other layers lie above the hub's growth:
REF line 45 with `rho` = 3.19 has four growth cells (7.27e-5, 2.32e-4, 7.40e-4, 2.36e-3 m) and the caps 2.55e-3 (hub, as now),
2.58e-3, 4.13e-3, 5.77e-3, 7.14e-3 m at node layers 3 to 6, and five growth cells with the cap 8.8e-3 to 8.9e-3 m at node layers 7
to 10. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); no constant is introduced, nothing
is searched.

Not removed: part (ii), the layer-dependent bend of the blend from `a` to `b` at row 3. It is left as it is (changing the blend
changes the far grid of every layer).

**Predicted effect** (from the probe numbers: the cell area ratios with the row-step ratio of part (i) set to 1, the prism
formula for the offsets, the measured `n` and the in-plane part of `d`; accurate to about 0.5 deg):

| face | S''' | predicted S'''' |
|---|---|---|
| REF C (45, 2) / (45, 3), layer 3 | 84.688 (`d` radial 0.710) | 80.7 (`d` radial about 0.48) |
| REF C (45, 2) / (45, 3), layer 4 / 5 / 6 | 83.89 / 84.60 / 84.28 | 81.3 / 82.6 / 83.0 |
| B2 C (45, 2) / (45, 3), layer 3 / 5 | 84.13 / 84.218 | 80.3 / 81.9 |
| the first ramp face C (56, 2) / (56, 3), layer 3 (REF / B2) | 84.397 / 83.564 | lower, same mechanism (not estimated) |
| the cap faces (first cell beyond the growth, layer 3) | | about 81 (offset 3.3e-4 m over a centroid distance of 2.9e-3 m, 6.6 deg, at an in-plane 80 deg) |

So the global maximum moves to faces the field changes little: REF about 83.8 deg (the gap-layer i-faces 83.76, the thin-tip
faces C (45, 2) / (45, 3) layers 7 to 9 at 83.6 to 83.7, which may move by up to about 0.4 deg because row 4 comes closer to the
wall there and the blend bends it less); B2 about 83.3 to 83.9.

Cost (not limited): in the long layers the near-wall rows are finer than now beyond the first free cell (REF tip, second free cell
2.3e-4 instead of 3.7e-4 m) and the capped cells larger (up to about 1.9 times the mean cell of the line instead of 1.5).

## Development protocol (registered)

The code is written to this design. Before the registered run, development builds are allowed only to fix defects (code that
does not do what this design says) and print only the build status, the construction step and the hub ratio of line 45; no
quality metric is computed or printed in development. No design variant is permitted: if S'''' misses, the miss is the result.
Every development build is listed in an amendment.

## Common set-up (fixed now)

The s4 specs, `REF`, `B2`, the fold measure, periodicity, hash, `locate`, `face_between`, `c_ids`, the W5 measure of
`f782_cgrid_wake_remap.rs` / `f784_cgrid_wake_ramp.rs` and the R2w measure of `f784_cgrid_wake_ramp.rs` are copied into the new
test file `crates/rcfd-mesh/tests/f791_cgrid_free_rows.rs`. `S'''' = camber_seam_s_free_rows_construction()`. Quality:
`quality_summary` (maxima over every face and cell) and `compute_metrics` (per face). The **first-free-row faces** are the faces
between C (i, nb) and C (i, nb + 1) (s4: rows 2 / 3) of every blade line `n_down <= i < n_i - n_down` at the cell layers 3 to 6.

## Checks (limits fixed now)

| id | clause | what | limit | estimate behind it |
|---|---|---|---|---|
| R1a | c3 | `build_cgrid_mesh_with(&REF / &B2, &s4, &S'''')` | Ok, cells > 0 | step 5 predicted (as S'''), reported |
| R1b | c3 | no folded cell: every corner Jacobian of the C- and F-block hexes of one sign, none zero; min volume > 0 | met | monotone distributions; only free rows move along their straight lines |
| R1c | c3 | periodicity of the seams | <= 1e-12 m | seam nodes untouched |
| R2 | c4 | max non-orthogonality (all faces), REF and B2 | <= 84.5 deg | REF about 83.8, B2 about 83.3 to 83.9 (above) |
| R2f | c2 (mechanism) | max non-orthogonality of the first-free-row faces, REF and B2 | <= 84.0 deg | REF 83.0 (layer 6), B2 below that (table above) |
| R2m | c2 (mechanism) | the radial component of the unit centroid vector of REF C (45, 2) / (45, 3), layer 3 | <= 0.60 | about 0.48 (part (ii) remains; S''' 0.710) |
| R2w | c4 | max non-orthogonality of the wake-cell faces (ADR 0702 R2w measure), REF and B2 | <= 84.5 deg | below S''' 84.397 / 83.791 |
| R3 | c5 | max skewness (all faces) | <= 4.0 | REF about 2.8, B2 about 2.8 (the nose i-faces at row 7; their rows below the cap keep the hub's alignment) |
| R4 | c5 | max aspect ratio | <= 1000 | 525.6 / 551.7 (the gap wake cell C (59, 9); its longest edge is a seam or row edge, the capped cell grows to at most 9e-3 m) |
| R5 | c6 | max wake-stack spanwise skewness (ADR 0699 W5 measure) | <= 1.0 on both | 0.4196 / 0.3773, unchanged (the stack rows are not moved) |
| Rrow | c2 | REF line 45: step from row 3 to row 4 at node layer 4 over that at node layer 3 (the final grid) | in [0.95, 1.05] | 1.016 (S''' 1.174; the a-steps 0.9934 times the blend factors 0.995 / 0.973 of probe 4) |
| Rfirst | c2 | REF and B2, line 45: step from row 2 to row 3 (the first free cell) at every node layer, S'''' against S''' | relative difference <= 1e-6 | both are `first` times a scale within 1e-12 of 1, the blend weight at row 3 unchanged |
| R7 | c2 (locality) | on the algebraic grids (`smoothing: false`) of S'''' and S''' of REF and B2: every grid node except the free C nodes (`stack_cells[k][i] < j < n_side`) equal in bits | every node | the field changes only `rest`; the seam node is the target itself, the F and core blocks and the stacks do not depend on `rest` |
| R8 | c2 | `free_row_ratios` of S'''' REF: line 45 | in [3.1, 3.3] | 3.19 (hub, probe 2); `None` on S''' |
| SC | c7 | scan of S'''' inside the camber_seam range: REF and B2, `front_gap` in {0.15, 0.3, 0.6, 1.0} x `n_nose` in {4, 6, 9}, the other s4 values; "builds" = R1a, R1b and R1c met | per blade at least 9 of 12 (fraction >= 0.75) | S''' built 12 of 12 per blade (ADR 0702); quality per point reported, not a gate. Heavy (24 builds): an ignored record `sc_the_scan_heavy`, run once with `--ignored` |
| SC0 | c7 | every scan point inside the range (`control_range_problems` empty) | 24 of 24 | ADR 0702 SC0 |
| HS | c8 | S on REF: FNV-1a mesh hash | `0x0105a2fcca924d23` | the new code is behind the new field |
| HS' | c8 | S' on REF and B2 | `0x0d257b2fcee093ca` / `0xefc53b8c413a845b` | as HS |
| HS'' | c8 | S'' on REF and B2 | `0x63011aa8288ac9c0` / `0x769e2e330b0cf295` | as HS |
| HS''' | c8 | S''' on REF and B2 | `0xba8d86a18ca34784` / `0x64432ac2c53ab55a` (the base probe above) | as HS; the hub pre-build is only on the new field's path |
| HB1 | c8 | `cgrid_w35d` b1 tests (G and H hashes) | pass | |
| HB2 | c8 | `f726_cgrid_staggered` `b2_camber_seam_h_on_the_default_computer_fan_is_bit_identical` | pass | |
| E1 | c8 | by name, the non-ignored tests of `f784_cgrid_wake_ramp`, `f782_cgrid_wake_remap`, `f777_cgrid_unstretched`, `f726_cgrid_staggered`, `cgrid_w35d`; `f722_rom_reference_fan` (rcfd-post); `fan01_p10_w36d` `c2_fan01_h_mesh_is_accepted_by_the_solver_and_the_rules` (rcfd-api); `fan_design_w46a` `s4_realised_default_meshes` | pass | no line they run changes |
| E2 | c8 | `cargo test -p rcfd-mesh --release --lib` | 0 failed (77 passed, 2 ignored, ADR 0702) | |
| E3 | c8 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass (3 passed, 1 ignored) | `fan_cgrid.rs` is reached from the fan case builder |
| Rep | reported | S'''' on REF and B2: step, cells, `QualitySummary`, the maxima located, the first-free-row faces of line 45 per layer, the five largest faces, the wake-cell maximum, the first ramp face, `free_row_ratios` of lines 44 to 48 and 56 | none | |

### Negative controls

| id | control | limit |
|---|---|---|
| NC-R2 | the R2 measure on S''' REF | 84.688 to 1e-3 at C (45, 2) / (45, 3), layer 3 (the measure sees the miss) |
| NC-R2m | the R2m measure on S''' | 0.710 to 0.005 |
| NC-Rrow | the Rrow measure on S''' | 1.174 to 0.005 |
| NC-R7 | the R7 comparison restricted to the free nodes | some node differs on both blades (the field does something) |
| NC-R8 | `free_row_ratios` of an S''' mesh | `None` |

## Planned mutations (scratch copy of the worktree, each run once against the new test file, HB1 and HB2)

| id | mutation | expected to be caught by |
|---|---|---|
| MU1 | the hub ratios not passed to the layer builds (S'''' builds the S''' distribution) | R2 on REF (84.688), R2f, R2m, Rrow, NC-R7 |
| MU2 | the cap `C` not solved but kept at 1.5 times the mean cell (the sizes `min(first rho^m, 1.5 mean)` scaled by `len / sum`) | Rrow and Rfirst (the scale changes every cell, the first included, from layer to layer), likely R2f |
| MU3 | the ratio inverted, `(x_1 - x_0) / (x_2 - x_1)` (below 1: every line falls back to `capped_cells`) | as MU1 |
| MU4 | the reference layer the tip (node layer `n_span`) instead of the hub | R8 (5.1); the ratio exceeds the growth the short layers can hold, so the cap is reached after fewer cells there (not predicted to break R2) |
| MU5 | the first free cell skipped (`s_m = min(first rho^(m + 1), C)`) | Rfirst (ADR 0696: the first free cell at the next stack height) |
| MU6 | an existing expression on the default path: `CAP_FACTOR` 1.5 -> 1.51 | HS, HS', HS'', HS''', HB1, HB2 |

## Decision rule

- **done**: every check met (R1a to R8, SC, SC0 on both blades, HS to HB2, E1 to E3, the negative controls).
- **partial**: HS to HB2 and E1 to E3 met, S'''' builds both blades fold-free and periodic (R1a to R1c), and some other check
  missed (each miss an ignored `*_known_miss` record with the limit intact, located and diagnosed).
- **todo**: S'''' does not build one of the blades fold-free, or HS to HB2 or E1 to E3 missed (an existing result changed).

## Limits (not covered)

- Part (ii) of the offset (the blend's bend at row 3) is not removed; the prediction keeps it.
- Two blades at s4 plus the 12-point scan per blade; no scan of `n_span`, `n_side`, `n_bl` or `growth`; the reference layer is
  fixed (the hub), not scanned.
- The predictions above combine a prism formula with measured base vectors; they are estimates (about 0.5 deg), the checks test them.
- No flow solve; no case value (a schema enum value needs the schema golden, rule 6); construction S'''' is library only.

## Addenda

### Amendment 1 (2026-11-17, after the code, before any registered run; no limit changed)

- No development build of S'''' was made: the code (`free_row_hub_ratio`, `camber_seam_s_free_rows_construction()`, the
  `CGridMesh::free_row_ratios` field, `hub_ratio_cells`, the per-line ratio of `Layer2d` and the hub pre-build in
  `fan_cgrid.rs`, the test file `f791_cgrid_free_rows.rs`) was compiled only (`--no-run`, clippy).
- The ratio recorded per line is `(x_2 - x_1) / (x_1 - x_0)` of whichever distribution the hub line used (the existing one, the
  hub being built without ratios).
- The constants of HS''' in the test file are the base probe's values (`f791-scratch/probe.log`).

## Addendum 1: results (2026-11-17)

One registered run of the new test file (`f791-scratch/run1.log`, release, `cargo test -p rcfd-mesh --release --test
f791_cgrid_free_rows -- --nocapture --test-threads 2`: 8 passed, 0 failed, 1 ignored, 53 s) and one run of the heavy scan record
(`run_sc.log`, 52 s on three threads).

| id | limit | measured | met |
|---|---|---|---|
| R1a | S'''' builds REF and B2, cells > 0 | 12 841 cells each, construction step 5 (as predicted) | yes |
| R1b | corner Jacobians of one sign, none zero; min volume > 0 | 100 096 of 100 096 negative, 0 zero on both; min volume 1.20e-13 / 1.58e-13 m^3 | yes |
| R1c | periodicity <= 1e-12 m | 1.96e-17 / 2.19e-17 m | yes |
| R2 | <= 84.5 deg (all faces) | REF **84.122** at C (41, 6) / (41, 7), layer 16 (tip gap); B2 83.887 at C (42, 6) / (42, 7), layer 16 (predicted about 83.8 and 83.3 to 83.9: the REF value is 0.3 deg above the estimate, at a face the estimate did not name) | yes |
| R2f | first-free-row faces, layers 3 to 6, <= 84.0 deg | REF 83.540 (line 44, layer 6; S''' 84.688), B2 83.178 (line 43, layer 6; S''' 84.218) | yes |
| R2m | <= 0.60 | 0.4253 (predicted about 0.48; S''' 0.710) | yes |
| R2w | wake-cell faces <= 84.5 deg | REF 83.371, B2 82.518 (both at C (58, 2) / (58, 3), layer 7); the first ramp face 82.206 / 80.993 (S''' 84.397 / 83.564) | yes |
| R3 | <= 4.0 | REF 2.5376 (C (37, 7) / (38, 7), layer 16), B2 2.7200 (S''' 2.7609 / 2.8312) | yes |
| R4 | <= 1000 | 525.6 / 551.7 (unchanged) | yes |
| R5 | wake-stack spanwise skewness <= 1.0 | 0.41955 / 0.37726, equal to S''' (the stack rows are not moved) | yes |
| Rrow | in [0.95, 1.05] | 1.01512 (predicted 1.016; S''' 1.17368) | yes |
| Rfirst | relative difference <= 1e-6 | 0 on both blades (bit-equal steps) | yes |
| R7 | nodes outside the free rows equal in bits | 0 differ on both; free nodes that differ: 6 555 (REF), 6 549 (B2) | yes |
| R8 | REF line 45 ratio in [3.1, 3.3] | 3.19051 | yes |
| SC | per blade >= 9 of 12 build | 12 of 12 and 12 of 12, fold-free and periodic, all at step 5 | yes |
| SC0 | 24 of 24 inside the range | 24 of 24 | yes |
| HS | `0x0105a2fcca924d23` | equal | yes |
| HS' / HS'' / HS''' | the base hashes | equal on both blades | yes |
| HB1 | `cgrid_w35d` b1 tests | pass (whole non-ignored file: 7 passed, 15 ignored) | yes |
| HB2 | `f726_cgrid_staggered` b2 | pass (whole non-ignored file: 7 passed, 1 ignored) | yes |
| E1 | named tests | `f784_cgrid_wake_ramp` 7 passed, 2 ignored; `f782_cgrid_wake_remap` 6 passed; `f777_cgrid_unstretched` 10 passed, 2 ignored; `f726` 7 / 1; `cgrid_w35d` 7 / 15; `fan_design_w46a` `s4_realised_default_meshes` passed; `fan01_p10_w36d` c2 passed; `f722_rom_reference_fan` 6 passed, 1 ignored | yes |
| E2 | `--lib` 0 failed | 77 passed, 2 ignored | yes |
| E3 | baseline_hashes | 3 passed, 1 ignored | yes |
| NC-R2 | S''' 84.688 at C (45, 2) / (45, 3), layer 3 (1e-3) | 84.68837 at that face | yes |
| NC-R2m | S''' 0.710 (0.005) | 0.70999 | yes |
| NC-Rrow | S''' 1.174 (0.005) | 1.17368 | yes |
| NC-R7 | some free node differs | 6 555 / 6 549 | yes |
| NC-R8 | S''' `free_row_ratios` None | None on both | yes |

Reported (Rep): the first-free-row face of REF line 45 per cell layer 0 to 9, S'''' (S'''): 80.033 (80.032), 80.457 (80.457),
80.971 (80.973), **82.803 (84.688)**, 82.645 (83.889), **83.416 (84.604)**, 83.522 (84.277), 83.667 (83.613), 83.697 (83.652), 83.721
(83.684). B2 line 45: 82.110 (84.133) at layer 3, 82.973 (84.218) at layer 5. Hub ratios REF lines 44 to 48 and 56: 3.225, 3.191,
3.159, 3.131, 3.105, 3.008; B2 3.287 to 3.142, 3.023. The five largest faces of both blades are the faces between the sixth and
seventh free cells of lines 41 / 42 in the tip-gap layers 12 to 16 (REF 84.10 to 84.12, B2 83.81 to 83.89). `wake_ramp_length`
equal to S'''. Scan quality (not a gate): within 84.5 / 4.0 / 1000 REF 4 of 12 (0.15 / 6, 0.15 / 9, 0.3 / 4, 0.3 / 6) and B2 4 of
12 (0.15 / 6, 0.15 / 9, 0.3 / 4, 0.3 / 6); S''' of ADR 0702: 2 and 4. The non-orthogonality still grows with front_gap and n_nose
(to 87.92 / 87.70 deg at 1.0 / 9, now on the first-free-row faces at layer 3), and the skewness exceeds 4.0 at front_gap 1.0
with n_nose 9 (5.29 / 6.97) and on B2 at 1.0 / 6 (4.66).

**The predictions of the first-free-row faces were too low by 0.5 to 2.1 deg** (REF line 45: predicted 80.7 / 81.3 / 82.6 / 83.0
at layers 3 to 6, measured 82.80 / 82.65 / 83.42 / 83.52; B2 80.3 / 81.9 at layers 3 / 5, measured 82.11 / 82.97), beyond the
stated 0.5 deg. The radial part was predicted to 12 % (R2m 0.425 against 0.48); the error is in the in-plane part, which the
estimate kept at its S''' value although the second free cell is now shorter at node layer 4 (diagnosis below). The limits held.

Diagnosis after the run (`f791-scratch/diag`, a `git archive` copy of the code commit with one added print test
`zz_diag.rs`, log `diag.log`, run once; a first attempt reused a stale library of the mutation copy from a shared target
directory, gave the MU6 numbers, and was discarded and rerun with its own target directory):

- **The former worst face** (REF C (45, 2) / (45, 3), layer 3): `n` is unchanged to 1e-4 ((0.9522, 0.2835, -0.1142)); `d` went
  from (-0.0271, 0.7037, 0.7100) to (-0.0856, 0.9010, 0.4253). The radial part fell as predicted, but the in-plane direction of
  `d` turned by 3.3 deg away from `n` (the second free cell at node layer 4 is now 2.30e-4 instead of 2.66e-4 m, so the centroid
  vector leans more on the slanted first cell), which the estimate had held fixed: in-plane 79.4 instead of 75.6 deg, total 82.80.
- **The new maximum** (REF C (41, 6) / (41, 7), tip-gap layer 16, 84.122 deg; S''' 83.292): `d` is the same as on S''' to 1e-3,
  the face normal turned by 0.8 deg in plane. In the long tip layers row 7 now ends five growth cells (steps 7.2e-5, 2.4e-4,
  8.1e-4, 2.69e-3, 8.95e-3 m on line 41) instead of lying in the capped part, so its distance along the line goes as `rho_i^5`;
  the hub ratios of neighbouring lines differ by about 1 % (REF lines 44 to 48: 3.225 to 3.105), which shifts row 7 of line 41
  against that of line 42 by about 5 % of that distance and tilts the row edge. The same face is 84.03 deg in the thin tip layer 9
  and 82.82 deg at layer 5.

Also run: `cargo fmt -p rcfd-mesh --check` clean; `cargo clippy -p rcfd-mesh --all-targets -- -D warnings` clean; `cargo test -p
rcfd-cli --release --test docs_site` passed (with this ADR and the `docs/meshing.md` paragraph).

### Mutations (`f791-scratch/mut`, `mutate.py`, logs `mut_MU*.log`; each run once against the new test file, HB1 and HB2)

| id | mutation | caught by |
|---|---|---|
| MU1 | hub ratios not used (the S''' distribution) | R2 on REF (84.688), R2f (84.688 / 84.218), R2m (0.710), Rrow (1.174), NC-R7 (0 free nodes differ), as predicted |
| MU2 | cap kept at 1.5 mean, sizes scaled to fit | Rrow (1.102), Rfirst (0.191 / 0.191), as predicted; not R2f (predicted "likely") |
| MU3 | ratio inverted (every line falls back) | as MU1, and R8 (0.313) |
| MU4 | reference layer the tip | R8 only (5.140), as predicted; R2 met (REF 83.808, B2 83.407) |
| MU5 | first free cell skipped | Rfirst (2.21 / 2.26), as predicted |
| MU6 | `CAP_FACTOR` 1.5 -> 1.51 | HS, HS' / HS'' / HS''' (the first failing assertion of each test), HB1 (three hashes), HB2, also NC-R2 (84.667) |

Six of six caught; MU4 only by the ratio check (it does not break the property the item asks for, as the registration said).

### Deviations

- R2 on REF met with 0.38 deg of margin at a face (the sixth / seventh free cells of line 41 in the tip-gap layers) that the
  registration did not name; the estimate (about 83.8) was 0.3 deg low there.
- The first-free-row face predictions were 0.5 to 2.1 deg low (above); the radial-offset prediction held.
- MU2 was predicted to be caught "likely" also by R2f; it was not (caught by Rrow and Rfirst). MU4 was predicted to be caught by R8
  only (see the table).
- No development build; no other deviation.

### Not run

The other ignored records of the named files; the whole `rcfd-mesh` integration suite and the workspace (the daily gate); flow
solves on S'''' meshes; other specs; a scan of `n_span`, `n_side`, `n_bl`, `growth` or of the reference layer.

### Decision by the registered rule

**done.** Every registered check is met: S'''' builds the reference and the second blade at s4 fold-free and periodic within
84.5 deg / 4.0 / 1000 (REF 84.122 / 2.538 / 525.6, B2 83.887 / 2.720 / 551.7), the reference's 84.688 deg face is 82.803 deg,
the wake-stack spanwise skewness stays 0.420 / 0.377, the scan builds 24 of 24, and S, S', S'', S''' and H are unchanged in bits.

### Remaining questions (each with options and a recommendation)

1. **The margin of the reference is 0.38 deg**, now at the faces between the sixth and seventh free cells of the lines 41 / 42 in
   the tip-gap layers (84.12 deg; B2 83.89). Options: (a) a follow-up that diagnoses those faces and the residual blend bend of
   the first free rows (part (ii) of the registration); (b) leave it, the clause is met. Recommendation: (b) for now; (a) only if a
   later change moves them above 84.5.
2. **The reference layer.** MU4 (the tip's ratios instead of the hub's) also met every quality limit and gave a lower maximum
   (REF 83.808, B2 83.407 deg). Options: (a) a registered item that compares hub, tip and mid-span references on both blades and
   the scan; (b) keep the hub (registered, derived: the smallest ratio and the largest neighbour ratio bound). Recommendation:
   (b); the MU4 number is one unregistered run and is not a basis for a change.
3. **A case value for S''''** needs a schema enum value and the schema golden (rule 6). Open for the maintainer, with F-778.
4. **The scan's quality at front_gap >= 0.6 and n_nose 9** (to 87.9 deg, skewness to 7.0) is unchanged in kind from ADRs 0696 and
   0702. Recommendation: keep the documented S-family quality range front_gap <= 0.3 with n_nose <= 6 (`docs/meshing.md`).

## For the lead: roadmap entries

> Proposed status: done

> Follow-up of F-784 (ADR 0702 question 1). ADR 0713, 2026-11-17. DONE. Diagnosis on the base: the 84.688 deg face of S''' (C (45, 2) / (45, 3), layer 3, the face between the first and second free cells at the thin-to-thick span transition) is a radial offset of the cell centroids (unit centroid vector radial 0.71): in a 9 mm thick span cell the second free cell grows by a different factor between its two node layers (the line distribution is solved per layer and the line length grows with the pitch) than the first, and the centroid of a prism moves by H (A1 - A0) / (6 (A0 + A1)). New opt-in construction S'''' = CGridOptions::camber_seam_s_free_rows_construction() = camber_seam_s_wake_ramp + free_row_hub_ratio: the free cells of every C line grow by the ratio the line has in the hub layer's true section, in every layer, and only the cap that ends the growth is solved per layer (CGridMesh::free_row_ratios reports the ratios). At the ADR 0118 s4 specs ComputerFan::rom_reference() blade 0 and the ADR 0696 second blade build with no folded cell, periodic, within 84.5 deg / 4.0 / 1000 (84.122 / 2.538 / 525.6 and 83.887 / 2.720 / 551.7); the former worst face is 82.803 deg, the radial part of its centroid vector 0.425; the first free cell is unchanged in bits, the second varies by 1.5 % between the node layers (S''' 17 %); wake-stack spanwise skewness 0.420 / 0.377 (unchanged); a front_gap 0.15 to 1.0 x n_nose 4 / 6 / 9 scan builds 12 of 12 on each blade (limit 9 of 12). Locality: only the free C rows change. camber_seam_h, camber_seam_s, camber_seam_s_unstretched, camber_seam_s_wake_remap and camber_seam_s_wake_ramp unchanged in bits; f784, f782, f777, f726, cgrid_w35d, f722, fan01_p10_w36d c2, fan_design_w46a s4, --lib and baseline_hashes pass. Six of six mutations caught. Library only; no case value.
