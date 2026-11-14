# ADR 0537: F-574 registration: an opt-in model check for the constant-strain Shell3 membrane on warped meshes (the largest angle between the normals of Shell3 facets sharing an edge, against a registered threshold; the message names the ANDES membrane of ADR 0466)

Status: **REGISTERED 2026-11-14, written before any code of F-574 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-574 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done with one plain Python script in the scratch folder
(`w49i-work/f574-scratch/angles.py`, the closed geometry of the strip only, no rcfd code; disclosed, its numbers quoted below).
The bit record B1 is a test file written and run on the base after this commit and before any library code (D5). Results go in
the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-574 (deps F-556), structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)), the
optional follow-up proposed by [ADR 0483](0483-f556-registration-shell3-twisted-strip-rest-stiffness-diagnosis.md) (F-556,
remaining question 1, option b). Number 0537 given by the lead. Branch `f574-work` from c1122807, worktree `w49i-work/f574`,
scratch `w49i-work/f574-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a missed limit becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; existing tests not
edited; no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-574 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. An opt-in model check reports Shell3 blocks with the constant-strain membrane whose adjacent facets meet at angles above a registered threshold, naming the ANDES membrane of ADR 0466; on the twisted strip of ADR 0473 the check fires at 15 and 30 degrees of twist and not on the flat strip; without the option every result and message bit-identical.

Clauses: **a0** every limit registered here; **a1** an opt-in model check that reports Shell3 blocks with the constant-strain
membrane whose adjacent facets meet above a registered threshold, the report naming the ANDES membrane of ADR 0466; **a2** on the
twisted strip it fires at 15 and at 30 degrees of twist and not on the flat strip; **a3** without the option every result and
message is bit-identical.

## What was read (c1122807, read only)

ADR 0483 whole (the diagnosis: the constant-strain membrane overstiffens the first torsion of the twisted strip, 4.05 percent at
12 x 36 and 30 degrees, through the membrane-bending coupling of the warped triangles; its variation matrix, quoted below; its
remaining question 1 and the follow-up text); ADR 0466 (the three Shell3 membranes, `Shell3Membrane`, D2/D3, the mixed-edge
refusal); ADR 0473 arithmetic 7 (the strip: chord 0.05, span 0.15, `theta(r) = twist r / L`); the sources `shell3.rs`
(`facet`, `Facet3::axes`, `model_facet`, `model_membrane`, `check_membrane_edges`, the pattern for a mesh-wide check over shared
edges), `shell.rs` (`check_shell_model`, the other checks of a shell model), `mesh.rs` (`BlockProperty::Shell`, `blocks`,
`block_of`, `element_nodes`); the test `sa_shell3_twist_bits.rs` (the strip builder and the digest pattern).

Existing warnings: the crate has no warning channel (it returns `Result`; a check is an `Err` or an `Ok`), and the existing
mesh-wide check `check_membrane_edges` refuses. A warning is advisory and must not refuse a model, so the new check is a
function that returns a report and never an error for a warped mesh.

No published source is needed: the measure is the angle between two unit vectors.

## Arithmetic before this registration (disclosed)

1. **The strip geometry.** `S(r, s) = (r, s cos theta, s sin theta)`, `theta = twist r / L`, nodes at `r = L j / nsp`,
   `s = -c/2 + c i / nch`, cells cut `[a, b, c], [a, c, d]` (ADR 0473; ADR 0483 D2). Plain Python, normals of the flat triangles
   from the cross product of two edges, angle `acos(n_1 . n_2)` over every edge shared by exactly two triangles (the boundary
   edges have one):

   | mesh | twist | edges with two facets | largest angle (degrees) | smallest | edges above 0.4 |
   |---|---|---|---|---|---|
   | 12 x 36 | 0 | 1248 | 0 | 0 | 0 |
   | 12 x 36 | 15 | 1248 | 0.5892452638404285 | 0.4159956867168883 | 1248 |
   | 12 x 36 | 30 | 1248 | 1.1784282153278391 | 0.827991716746086 | 1248 |
   | 24 x 72 | 15 | 5088 | 0.2946265270621718 | 0.2079690125793428 | 0 |
   | 24 x 72 | 30 | 5088 | 0.5892452638410471 | 0.4137673679836671 | 5088 |
   | 48 x 144 | 15 | 20544 | 0.1473137504481993 | 0.10397678621767764 | 0 |
   | 48 x 144 | 30 | 20544 | 0.2946265270621718 | 0.20682255900765642 | 0 |

   The largest angle is proportional to the twist per span (15 against 30 degrees is exactly a factor 2) and to the mesh
   spacing along the span (a factor 2 per refinement), to the printed digits.
2. **The excess to be warned about (ADR 0483, variation matrix, the Shell3-specific excess `Delta` of the constant-strain
   membrane over Hex20):** 12 x 36, 30 degrees `3.516e-2`; 12 x 36, 15 degrees `1.921e-2`; 24 x 72, 30 degrees `9.051e-3`;
   48 x 144, 30 degrees `2.391e-3`; flat strip (twist 0) `-2.04e-3` (the common element error, the ANDES membrane the same).
   ADR 0483 prediction P1 called `|Delta| <= 5e-3` negligible (the flat-strip level); the same level is used here to choose the
   threshold.
3. **The threshold.** The pairs (largest angle, `Delta`) around the 5e-3 level: (0.2946 deg, 2.39e-3) below it and (0.5892 deg,
   9.05e-3) above it. The linear interpolation of `Delta` between them reaches `5e-3` at `0.2946 + 0.2946 (5.0 - 2.39) / (9.05 -
   2.39) = 0.410` degrees. The threshold is registered as the round number **0.4 degrees**. It is a screening level and not a
   predictor: the angle does not fix the excess, since 12 x 36 at 15 degrees and 24 x 72 at 30 degrees have the same largest
   angle (0.5892) and `Delta` of `1.92e-2` and `9.05e-3` (the excess also grows with the membrane share of the mode and with the
   thinness, ADR 0483 P3). The check says "may be overstiff", not by how much. With 0.4: 12 x 36 fires at 15 and 30 degrees
   (smallest edge angle 0.416 and 0.828, so every interior edge is above; margin 4 percent at the smallest), 24 x 72 fires at 30
   degrees and not at 15, 48 x 144 fires at neither (its `Delta` at 30 degrees, 2.4e-3, is at the flat-strip level). The
   threshold was fixed from the ADR 0483 records and this table before any run of the check.

## Design

### D1. The facet angle

For a Shell3 element `e` the facet normal is `n_e = axes[2]` of `shell3::facet` (the unit normal of the node order,
`(x_1 - x_0) x (x_2 - x_0)`). Two Shell3 elements `e`, `f` share an edge when two of their nodes coincide. The **facet angle** of
the edge is the angle between the oriented normals, in degrees,

`phi = atan2(|n_e x n_f|, n_e . n_f) * 180 / pi`, in `[0, 180]`,

where the orientation of `f` is made consistent with `e` by the edge direction: if the edge `p -> q` is traversed in the same
direction by both elements (an inconsistent node order), `n_f` is negated first. A consistently ordered mesh traverses a shared
edge in opposite directions and takes no flip. A smooth surface has `phi` near 0; a fold has `phi` of the fold angle; `phi` is
the discrete measure of how far two adjacent facets are from one plane.

Only edges shared by **exactly two** Shell3 elements enter (a junction of three or more facets is a stiffener or a T-joint, not a
warped surface, and the angle is not defined for it). Shell4 and the other element kinds neither enter nor are reported.

### D2. Which blocks are reported

A block is considered when it is a Shell3 block whose membrane is `Shell3Membrane::ConstantStrain` (the default; a block with no
`Shell` property counts as the default, as `model_membrane` does). A Shell3 block with the ANDES (`AndesOpt`) or Allman membrane
is **not** considered: ADR 0483 located the excess in the constant-strain membrane and the ANDES membrane removes it (Shell3
ANDES `-3.3e-4` from Hex20); Allman's membrane has drilling unknowns and is `6.5e-3` there (and is not the default). The edges
of a block are the edges of its elements shared with another Shell3 element (of any block, any membrane); the block is reported
when at least one such edge has `phi > threshold` (strictly above). A distinct edge counts once per block.

### D3. The function and its report

```
pub const WARP_ANGLE_DEGREES: f64 = 0.4;
pub struct WarpedBlock { pub block: usize, pub n_edges: usize, pub max_angle: f64 }   // degrees
pub struct WarpReport { pub threshold: f64, pub blocks: Vec<WarpedBlock> }            // blocks in increasing index
impl WarpReport { pub fn is_empty(&self) -> bool; pub fn messages(&self) -> Vec<String> }
pub fn warped_membrane_report(model: &SolidModel, threshold_degrees: f64) -> Result<WarpReport>
```

in `shell3.rs` (appended; the default path calls none of it). `max_angle` is the largest `phi` over the edges counted for the
block, `n_edges` the number of those with `phi > threshold`. `threshold_degrees` must be finite and in `(0, 180]`, else
`FemError::InvalidValue { key: "threshold", .. }`. A model without a considered block gives an empty report; a degenerate Shell3
facet gives the error `model_facet` gives. The function reads the mesh only. It is not called by any solver, validator or the
case reader: the check exists only for a caller that asks (opt-in, no case-file key; see "Limits").

**The message** (one per reported block, in block order; `{thr}` the threshold with `{}`, `{max}` with `{:.4}`):

`block {b} (Shell3, constant-strain membrane): {n} edges shared with another Shell3 facet meet at a normal angle above {thr} degrees (largest {max} degrees); the constant-strain membrane follows a strain that varies over a facet only with parasitic in-plane shear and may overstiffen a warped shell (ADR 0483: 4.05 percent on the first torsion of the twisted strip of ADR 0473); the ANDES membrane of ADR 0466 (BlockProperty::shell_with_membrane with Shell3Membrane::AndesOpt) removes it`

### D4. Opt-in and defaults

The new function is not called from any existing path; no existing function changes. The only library file edited is `shell3.rs`
(appended code, and a `use` if needed). `check_shell_model`, `validate`, the solvers and their messages keep the bytes of the
base.

### D5. The bit record (B1)

`tests/sa_shell3_warp_bits.rs`, written and run on the base c1122807 (this ADR committed, no library line changed) before any
library code. Digests (FNV-1a 64 over the count and the bits, ADR 0291) of: the constant-strain Shell3 strip stiffness at twist
0, 15 and 30 degrees (12 x 36), the rest frequencies of the 30 degree strip (6 modes), and the **text** of the messages of
`check_shell_model` and `validate` for models that give them: a Shell3 block without a Shell property, a degenerate Shell3
facet, and a mesh with a constant-strain and an ANDES Shell3 element sharing an edge (the refusal of ADR 0466 D3), and the `Ok`
of the warped strip. Uses only API of c1122807. It is run again after the code and must give the recorded digests (clause a3).

## Checks (limits fixed now)

The test strip is the builder of `sa_shell3_twist_bits.rs` with the twist as a parameter (0, 15, 30 degrees; 12 x 36 and, for
W4, 24 x 72 and 48 x 144), cells cut `[a, b, c], [a, c, d]`, one Shell3 block of thickness 0.001. The reference angles are the
Python values of arithmetic 1; the Rust and the Python computations differ only in rounding (`atan2` against `acos`, the order
of sums), at most about `1e-12` degrees; the limit on every angle comparison is `1e-8` degrees.

| id | check | limit | estimate |
|---|---|---|---|
| W1 | 12 x 36, 30 degrees, constant-strain: the report has exactly one block (block 0), `n_edges = 1248`, `max_angle = 1.1784282153278391` | count exact; `abs(max - ref) <= 1e-8` degrees | arithmetic 1; it fires (clause a2) |
| W2 | 12 x 36, 15 degrees: one block, `n_edges = 1248`, `max_angle = 0.5892452638404285` | same | arithmetic 1; it fires (a2) |
| W3 | 12 x 36, twist 0 (the flat strip): the report is empty with threshold 0.4, and also with threshold `1e-300` (no edge of the flat strip is above any positive angle) | empty | the flat normals are exactly `(0, 0, +-1)` and the cross product is exactly 0 (a2) |
| W4 | threshold behaviour on the refinement records: 24 x 72 at 30 degrees fires (`n_edges = 5088`, `max = 0.5892452638410471`); 24 x 72 at 15 degrees, 48 x 144 at 15 and at 30 degrees give an empty report | counts exact, `1e-8` on the angle | arithmetic 1, 3 |
| W5 | the message of W1 equals the registered text of D3 with `b = 0, n = 1248, thr = 0.4, max = 1.1784`, character for character; it contains "ADR 0466", "AndesOpt" and "ADR 0483" | exact string | D3 (clause a1, the naming of the ANDES membrane) |
| W6 | the same 30 degree strip with the ANDES membrane (`shell_with_membrane(t, AndesOpt)`): empty report; with the Allman membrane: empty | empty | D2 |
| W7 | a Shell4 strip (the same nodes, quadrilateral cells) at 30 degrees: empty report | empty | D1 |
| W8 | a model of two Shell3 blocks, each its own warped 30 degree strip on disjoint nodes, block 0 constant-strain and block 1 ANDES: only block 0 is reported | one block, index 0 | D2 (block index and ANDES exclusion in a mixed model) |
| W9 | a fold: two triangles sharing an edge, the second turned 60 degrees about it, consistent node order: `max_angle = 60`, `n_edges = 1`; the same fold with the second triangle's node order reversed (the shared edge traversed in the same direction): `max_angle = 60` | `abs(max - 60) <= 1e-8` | D1, the orientation by the edge direction (unflipped it reads 120) |
| W10 | three triangles sharing one edge (a T-joint, 90 degrees between them): empty report (the edge has three facets and does not count) | empty | D1 |
| W11 | strictness: with `threshold = max_angle` of W1 (the reported value, bitwise), the report is empty; with the next smaller `f64`, one block | as stated | D3 "strictly above" |
| W12 | threshold validation: `0`, `-1`, `NaN`, `inf`, `180.0001` each give `FemError::InvalidValue` with key `threshold`; `180` is accepted | variant and key exact | D3 |
| W13 | a model with no Shell3 block (a Shell4 strip, and a Hex8 box) gives an empty report and no error | empty | D3 |
| W14 | `WARP_ANGLE_DEGREES == 0.4` and a call with it reports the threshold 0.4 | exact | D3 |
| B1 | the default bits (D5) are the recorded digests, recorded on the base before the code and equal after it | digests equal | clause a3 |
| B2 | the existing tests of the crate (`cargo test -p rcfd-fem --release`, whole crate) and the `rcfd-api` `baseline_hashes` test pass unedited, the hash table unchanged | all pass | clause a3 |

W1 to W4 are clause a2 with its threshold neighbours; W5 is clause a1's naming of the ANDES membrane; B1 and B2 are clause a3.

## Negative controls and mutations

Negative controls (must not fire): W3 (flat strip), W6 (ANDES and Allman), W7 (Shell4), W10 (T-joint), W4's three empty cases,
W13, and W11's equal-threshold case.

Mutations, each applied to a scratch copy of the final code (its own `CARGO_TARGET_DIR`) and run on the check tests; each must be
caught by a check named:

| id | mutation | expected catcher |
|---|---|---|
| MU1 | the angle compared in radians against the threshold in degrees (no conversion) | W1, W2, W4 (never fires) |
| MU2 | every Shell3 block considered (the membrane filter dropped, ANDES and Allman included) | W6, W8 |
| MU3 | Allman not excluded (the filter `!= AndesOpt`) | W6 |
| MU4 | the orientation flip omitted | W9 (the inconsistent order reads 120) |
| MU5 | `>=` in place of `>` | W11 |
| MU6 | a three-facet edge not skipped (the first two facets taken) | W10 |
| MU7 | the minimum angle reported in place of the maximum | W1, W2 |
| MU8 | the edge count taken per element and not per distinct edge | W1, W2, W4 |
| MU9 | the threshold validation removed | W12 |
| MU10 | the constant changed (0.4 to 0.3) | W4, W14 |

## Decision rule

**Done** when W1 to W14 and B1, B2 are met (limits as registered), each mutation is caught by a registered catcher, `cargo fmt
-p rcfd-fem --check` and `cargo clippy -p rcfd-fem --all-targets -- -D warnings` are clean, and the `baseline_hashes` and
`docs_site` tests pass. **Partial** if a check missed with a diagnosed cause (kept as an ignored `*_known_miss` test with its
limit and number) or a mutation is not caught. **Todo** if the check cannot be built.

## Limits (not covered)

- The check is a library function. A case-file key, a CLI or MCP flag, and calling it from the model validators or the solver
  set-up would change goldens (schema, CLI help) or default messages and are **not** done: open question in the addendum.
- The threshold is a screening level from one family of geometry (the twisted strip, the rest frequency of its first torsion).
  It is not an error bound: the angle does not fix the excess (arithmetic 3), and a shell of another kind (double curvature, a
  fold) may differ. The check runs no solution.
- Shell4 blocks with the bilinear membrane (ADR 0483: 5.3e-3 on the strip) are not reported.
- The edge angle does not measure the twist of the mesh as a whole, the thickness or the membrane share of a mode, which scale
  the excess (ADR 0483 P3).
- Only edges shared by two Shell3 elements are measured; Shell3 facets adjacent to Shell4 facets are not.
