# ADR 0699: F-782 registration: wake stacks of construction S' at the thin end-wall layers

Status: **REGISTERED 2026-11-17, written before any code of F-782 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-782 was written and nothing was built or run for it. Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-782 (deps F-777), proposed by [ADR 0696](0696-f777-registration-construction-s-without-the-first-cell-stretch-on-strongly-staggered-blades.md)
"For the lead" (follow-up) and its remaining question 1. Parents: ADR 0696 (construction S',
`CGridOptions::camber_seam_s_unstretched_construction()`, the second blade B2, the B2e miss and its diagnosis), ADR 0692
(construction S), [ADR 0118](0118-wave46a-computer-fan-schema-checker-perturbations.md) (the s4 specs), ADR 0072 (W1, the
remapped radius), ADR 0083 (H3, the wake factor faded over a quarter of the wake lines). Number 0699 given by the lead. Branch
`f782-work` from 2acf29d9 (F-726 / ADR 0692 and F-777 / ADR 0696 merged), worktree `w49i-work/f782`, scratch `w49i-work/f782-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests and ignored records not
edited; defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

Clean-room: elementary geometry (a chord of a circular arc, a smooth step), the repository's own constructions (ADR 0072 W1,
ADR 0083 H3) and quality measures (`mesh.rs` `polygon_geometry` and the cell centre of `compute_geometry`, `quality.rs`
skewness). No source outside the repository was read.

## What F-782 asks

> Every limit not stated here is registered in the ADR before the run. A new opt-in field of S' builds the wake-stack nodes of the thin end-wall layers so that the spanwise faces of the wake stack meet skewness 4.0 on ComputerFan::rom_reference() blade 0 and on the ADR 0696 second blade at the ADR 0118 s4 specs (the measured 3.24 and 4.0112 of ADR 0696 disclosed), with no folded cell within 85 deg non-orthogonality and aspect 1000 on both, and camber_seam_h, camber_seam_s and camber_seam_s_unstretched unchanged in bits.

Clauses: **c1** every limit registered here; **c2** a new opt-in field of S' that builds the wake-stack nodes of the thin
end-wall layers; **c3** the spanwise faces of the wake stack within skewness 4.0 on the reference blade 0 (REF) at s4; **c4** the
same on the ADR 0696 second blade (B2); **c5** on both: no folded cell, non-orthogonality <= 85 deg, aspect <= 1000; **c6**
camber_seam_h, camber_seam_s and camber_seam_s_unstretched unchanged in bits. From the brief: the bit equality shown by hashes
(the S hash `0x0105a2fcca924d23` of ADR 0696, an S' hash, the B1 / H hashes), the named existing tests rerun, negative controls
and mutations registered.

## What was read before this text (disclosed)

Read: ADR 0696 (all, including the addendum and its diagnosis), `fan_cgrid.rs` (`CGridOptions` and its constructors,
`build_layer` lines 1015 to 1538: the wake curve, the stacks, `lean_stack`, the C lines; `remapped_radii`, `write_layer`, the layer
loop of `build_cgrid_mesh_with` with the cache and the fallback chain), `mesh.rs` (`polygon_geometry`, the cell centre of
`compute_geometry`), `quality.rs` (the skewness and non-orthogonality definitions), the test file `f777_cgrid_unstretched.rs`
(its helpers are reused).

**Already seen (disclosed, as the acceptance says):** ADR 0696 measured S' on REF at s4 (skewness 3.2404 at the spanwise face
between the wake-stack cells C (4, 1) of layers 8 and 9; 84.688 deg at C (45, 2) / (45, 3) layer 3; aspect 525.6 at C (59, 9)
layer 16) and on B2 (4.0112 at the same face; 84.218 deg at C (45, 2) / (45, 3) layer 5; aspect 551.7 at C (59, 9) layer 16). Its
diagnosis log `f777-scratch/diag_b2e.log` (read here) prints the six largest skewness faces of S' on both blades (REF: the four
wake-stack faces 3.2404 to 3.2054 of lines 4 and 59 between layers 8 and 9, then 2.7609 at C (37, 7) / (38, 7) layer 6 and 2.7065
layer 16; B2: four faces 4.0112 to 3.9787 at the same places, then 3.3419 and 3.3325 at C (4, 1) and C (4, 0) between layers 7
and 8), the radii of the node layers and the in-plane nodes `(z, r theta)` of rows 0 to 3 of lines 4 and 5 in layers 7 to 11.

**Arithmetic done for this text (disclosed; `f782-scratch/pre_skew.py`, `pre_skew2.py`, one computation each, no variant).**
From the printed nodes alone (no build of the generator), the repository's face-centre, cell-centre and skewness formulas
(transcribed into the script) reproduce the logged values: 3.2404 and 4.0112 at the face between C (4, 1) of layers 8 and 9 and
3.2284 / 4.0000 at C (4, 0), 2.6994 / 3.3419 between layers 7 and 8; the face between layers 9 and 10 (the tip, the first gap
cell above) has skewness 0.485 / 0.490 but non-orthogonality 76.19 / 79.04 deg. Then the rule registered below, approximated by
giving the nodes of layers 7, 8 and 9 the `(z, theta)` of layer 10 (the remapped radii of those layers lie within 3e-6 m of the
tip radius, neglected), gives skewness 0.0084, 0.0103, 0.0115 (REF) and 0.0067, 0.0082, 0.0092 (B2) at the faces 7 / 8, 8 / 9,
9 / 10 of C (4, 0) and C (4, 1), non-orthogonality 1.32 / 1.11 deg. This is an estimate on four nodes per layer of two lines; the
rest of the mesh was not computed.

Further pencil numbers (s4, both blades share the radii): mean span cell `(0.058 - 0.0215) / 10 = 3.65e-3 m`; the thin span cells
(thinner than the mean) are cells 0, 1, 2 (5e-5, 6e-5, 7.2e-5 m) at the hub and 7, 8, 9 (7.2e-5, 6e-5, 5e-5 m) at the tip; the
remapped radius moves by `Delta r^2 / mean x 1.0099` per thin cell, at most 1.43e-6 m (against 5e-5 to 7.2e-5 m of true radius);
`rho_k - r_k` in the thick layers lies between -9e-5 m (node layer 4) and +1.8e-4 m (node layer 7). The pitchwise drift of the
wake-stack nodes of line 4 per unit radius at the tip, `r dtheta / dr`, is 0.39 (REF) and 0.49 (B2) from the printed nodes.

## Diagnosis that the design rests on (from the disclosed arithmetic)

The wake cells near the outlet are long: line 4 to line 5 is 0.0104 m axially and 0.0193 m pitchwise on REF, i.e. the cell spans
`Delta theta` = 0.333 rad (19 deg) of the cylinder. Its spanwise faces (constant `r`) are cylinder strips whose sagitta
`r (1 - cos(Delta theta / 2))` = 8e-4 m is 15 times the radial thickness of a thin layer (5e-5 to 7.2e-5 m), while the generator
represents every face by its four corners: `polygon_geometry` takes the face centre from a triangle fan about the corner mean, and
the cell centre is a pyramid sum about the mean of the face centres. For such strips the computed cell centre moves along the
strip by a fraction of a millimetre when the strip is sheared between its two node layers (the thin-layer wake-stack nodes drift
pitchwise by 0.39 to 0.49 times the layer thickness because every layer takes the wake stack of its true section), and against a
centre distance of 5e-5 to 7e-5 m this shift is a skewness of 3 to 4 (and the 76 to 79 deg at the tip face where the sheared thin
cell meets the unsheared gap cell). In the thick layers the same shift is divided by a centre distance of 9e-3 m. The disclosed
arithmetic confirms that without the shear the same strips give skewness about 0.01.

## Design (fixed now): construction S'', `camber_seam_s_wake_remap`

One new field of `CGridOptions`, `wake_stack_remap: bool`, default `false` (so `Default`, every existing constructor and every
struct literal with `..` are unchanged), and a new constructor `CGridOptions::camber_seam_s_wake_remap_construction()` =
`camber_seam_s_unstretched_construction()` + `wake_stack_remap: true` (S''). The field acts only in `write_layer` and only when the
layer has a remapped grid `b` (with `radial_remap`, `rho_k != r_true`, i.e. the span node layers 1 to `n_span - 1`; the hub, tip and
gap layers have no `b` and are unchanged). With it, on every C wake line `i` (`i < n_down` or `i > n_i - n_down`; wake index
`p = n_down - i` or `i - (n_i - n_down)`), every node `j = 0 .. n_side` is written from

- `a' = a + omega_p (b - a)` in `(z, theta)` for the stack rows `j <= nb` (instead of `a`), and
- `b + w (a' - b)` for `j > nb` (instead of `b + w (a - b)`), with the existing blend weight `w` and blend length,

where `omega_p = 1 - fade(p / max(n_down / 4, 1))` (so 0 on the trailing-edge line `p = 0`, 0.5 at `p = 1` and 1 from `p = 2` on
at s4). Every other node and every other expression (the blend length `ell` from the stack-edge differences of all lines, the
blade lines, the F block, the core, the hub, tip and gap layers, `build_layer`, the fallback chain) is unchanged, so S, S', H, G
and the default keep their bits (the new expression sits in an `if` on the new field).

Derivation. (i) The wake stack carries no wall: its in-plane position at each radius is a construction choice, and ADR 0072 W1
already builds every node away from the walls from the grid at the remapped radius, whose increments are proportional to
`Delta r^2` for layers thinner than the mean span cell, so that the grid does not rotate between the thin end-wall layers. Taking
the wake stacks from the same grid extends W1 to them: per thin cell the remapped radius moves by at most 1.43e-6 m instead of 5e-5
to 7.2e-5 m, so the pitchwise drift between thin layers falls by the factor `Delta r / mean` (at most 0.02, from about 2.3e-5 m to
about 6e-7 m per layer at the tip), and the thin-layer wake cells become near radial extrusions of the end layer, the shape the
diagnosis shows to have skewness about 0.01. The drift is not removed but moved into the adjacent thick cell (9e-3 m), where it
adds `(sum of the thin layers) x 0.4` = about 7e-5 m to its 3.5e-3 m. (ii) The trailing-edge line `p = 0` is the last line of the
blade wall and its stack, so it keeps the true section (`omega_0 = 0`); the change to the remapped grid is faded along the wake
over a quarter of the wake lines with the smooth step `fade`, as ADR 0083 H3 fades its trailing-edge stack factor. Near the
trailing edge the wake cells are short (the first axial wake cell is the trailing chord spacing, about 7.7e-5 m axially, 3.5e-4 m
along the wake), so their spanwise faces are nearly flat and the shear left at `p <= 1` does not give the strip effect. (iii) The
far rows `j > nb` of a wake line blend from the new stack edge `a'` instead of `a`, so the first free row stays continuous with the
stack; at `omega = 1` the whole line is the remapped grid's own (self-consistent) line.

Cost (not limited): in the thick span layers the wake stack lies on the wake curve of the section at the remapped radius, up to
1.8e-4 m radially away from the true radius (a pitchwise offset of the order of one stack cell); the wake curve is a construction
line, so no wall moves. The wake-stack nodes of the thin layers sit pitchwise up to about 7e-5 m off the true section's wake curve.

## Development protocol (registered)

The code is written to this design. Before the registered run, development builds are allowed **only to fix defects** (code that
does not do what this design says) and print only the build status, the construction step and, on a refusal, the generator's
located message; no quality metric is computed or printed in development. No design variant is permitted: if S'' as registered
misses, the miss is the result (a `*_known_miss` record). Every development build is listed in an amendment. The S' hashes (HS'
below) are computed on a `git archive` copy of 2acf29d9 with one added test file holding only what they need, before the
registered run.

## Common set-up (fixed now)

The s4 specs (`fan_design_w46a.rs` `smoke_specs()`, as copied in `f777_cgrid_unstretched.rs`), `REF =
ComputerFan::rom_reference().axial_fan(0, 0.03, 0.04)`, `B2` = the ADR 0696 second blade (stagger -1 deg, chord +20 % on every
blade; outer stagger 76.6 deg). `S'' = camber_seam_s_wake_remap_construction()`. The fold measure (corner Jacobians), periodicity,
hash and `locate` of `f777_cgrid_unstretched.rs` are copied into the new test file `crates/rcfd-mesh/tests/f782_cgrid_wake_remap.rs`.
Quality: `quality_summary(&mesh)` (maxima over every face and cell) and `compute_metrics` (per face).

Sets used by the checks: the **wake-stack cells** are the C cells `(i, j, k)` with `i < n_down` or `i >= n_i - n_down` and `j <
n_bl`; their **spanwise faces** are the internal faces between `(i, j, k)` and `(i, j, k + 1)`, `k = 0 .. n_k - 2` (span and gap).
The **thin span cells** are the cells `k < n_span` with `radii[k + 1] - radii[k] < (r_tip - r_hub) / n_span` (0, 1, 2, 7, 8, 9 at
s4). The **full-weight wake lines** are those with `p >= max(n_down / 4, 1)` (`omega_p = 1`).

## Checks (limits fixed now)

| id | clause | what | limit | estimate behind it |
|---|---|---|---|---|
| W1a | c5 | `build_cgrid_mesh_with(&REF, &s4, &S'')` | Ok, cells > 0 | the change moves wake nodes by at most about 1e-4 m in-plane against stack cells of 5e-5 m and line spacings of 3.5e-4 m and more; construction step predicted 5 (as S'), reported |
| W1b | c5 | no folded cell: every corner Jacobian of the C- and F-block hexes of one sign, none zero; `min_volume` > 0 | met | as W1a |
| W1c | c5 | periodicity of the seams (every layer) | <= 1e-12 m | the seam nodes are not wake-line nodes except the outlet ones, whose two copies use the same `omega` and the same pair of grids |
| W2 | c5 | max non-orthogonality (all faces) | <= 85 deg | predicted 84.688 unchanged (its face is on blade line 45, not touched) if the step stays 5; the new thick-to-thin wake faces about 21 deg |
| W3 | (c3, all faces) | max skewness (all faces) | <= 4.0 | predicted 2.7609 (the largest face not on a wake stack, line 37, unchanged) |
| W4 | c5 | max aspect ratio | <= 1000 | predicted 525.6 unchanged (gap layer 16, not touched) |
| W5a | c3 | max skewness over the spanwise faces of the wake-stack cells | <= 4.0 | the disclosed estimate 0.01 at lines 4 / 5; the trailing-edge lines `p <= 1` keep part of the shear but their cells are short; estimate below 0.3 everywhere |
| W5b | c3 (mechanism) | the same set | <= 1.0 | as W5a; a looser value would not show that the strip shear is removed |
| W6 | c2 | on the S'' mesh of REF: over every thin span cell `k`, every full-weight wake line `i` and every stack row `j <= n_bl`, the drift `max(|z_{k+1} - z_k|, r_{k+1} |theta_{k+1} - theta_k|)` of node `(i, j)` | <= 5e-6 m | `Delta rho <= 1.43e-6 m` per thin cell times `r dtheta / dr` 0.39 to 0.49 at the tip (unknown at the hub; a slope up to 3 is covered) |
| W7 | c2 (locality) | on the algebraic grids (`smoothing: false`, step 5 by construction) of S'' and S' of REF: every grid node outside the C wake lines `p >= 1` of the node layers 1 to `n_span - 1` equal in bits | every node | the field acts only there |
| B2a..B2h | c4, c5 | W1a to W5b on B2 (same limits) | as W1a..W5b | W2 predicted 84.218 unchanged, W4 551.7 unchanged, W5a / W5b estimate 0.01 to 0.3; W3 not predicted to the digit: the largest non-wake-stack face of B2 was not printed by ADR 0696 (below 3.3325); estimate the REF runner-up 2.76 times the chord ratio 1.2: about 3.4 |
| B2-W6 | c2 | W6 on B2 | <= 5e-6 m | as W6 |
| HS | c6 | S (`camber_seam_s_construction()`) on REF at s4: FNV-1a mesh hash | `0x0105a2fcca924d23` (ADR 0696 BS) | the new expression is behind the new field; also the existing `f777_cgrid_unstretched` `bs_construction_s_is_bit_identical` |
| HS' | c6 | S' (`camber_seam_s_unstretched_construction()`) on REF and on B2 at s4: FNV-1a mesh hash on the branch equal to the hash of the same build on a `git archive` copy of 2acf29d9 (amendment) | equal, both blades | as HS |
| HB1 | c6 | `cgrid_w35d` `b1_existing_constructions_are_bit_identical`, `b1_construction_g_is_bit_identical`, `b1_construction_h_is_bit_identical` | pass | no expression of these paths changes |
| HB2 | c6 | `f726_cgrid_staggered` `b2_camber_seam_h_on_the_default_computer_fan_is_bit_identical` (H hash of ADR 0692) | pass | as HB1 |
| E1 | c6 | by name: the whole non-ignored `f777_cgrid_unstretched`, `f726_cgrid_staggered` and `cgrid_w35d` files; `fan01_p10_w36d` `c2_fan01_h_mesh_is_accepted_by_the_solver_and_the_rules` (rcfd-api); `fan_design_w46a` `s4_realised_default_meshes`; the non-ignored `f722_rom_reference_fan` (rcfd-post) | pass | no line they run changes |
| E2 | c6 | `cargo test -p rcfd-mesh --release --lib` | 0 failed, 77 passed, 2 ignored (ADR 0696) | no unit test touches the new field |
| E3 | c6 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass (3 passed, 1 ignored) | `fan_cgrid.rs` is reached from the fan case builder; default unchanged |
| R1 | reported | S'' on REF and B2: step, cells, `QualitySummary`, location of the maxima, the five largest wake-stack spanwise skewness faces | none | |

### Negative controls

| id | control | limit |
|---|---|---|
| NC-W5 | the W5a measure on the S' meshes of REF and B2 | reproduces ADR 0696: 3.2404 and 4.0112 (to 1e-4), at C (4, 1) between layers 8 and 9 (the measure sees the miss) |
| NC-W6 | the W6 measure on the S' mesh of REF | > 1e-5 m (pencil: about 2e-5 to 3e-5 m per thin layer at the tip) |
| NC-W7 | the W7 comparison of S'' against S' restricted to the changed set | at least one node of a full-weight wake line in a thin layer differs (the field does something) |

## Planned mutations (scratch copy of the worktree, each run once against the new test file, HB1 and HB2)

| id | mutation | expected to be caught by |
|---|---|---|
| MU1 | the new field ignored (S'' = S') | W5a / B2-W5a (B2 4.0112), W5b, W6, NC-W7 |
| MU2 | `omega = 1` also on the trailing-edge line (`p = 0`) | W7 (the trailing-edge line of the span layers moves) |
| MU3 | the far rows keep blending from `a` (`b + w (a - b)` for `j > nb`) | W5b (the first free row of the thin layers keeps the shear next to an unsheared stack), possibly W7 does not see it (inside the changed set) |
| MU4 | the fade over half of the wake lines (`max(n_down / 2, 1)`) instead of a quarter | W6 (the lines `p = 2, 3` keep part of the drift) |
| MU5 | the rule on the lower wake lines only (`i < n_down`) | W5a on B2 (line 59, 3.9899), W5b, W6 |
| MU6 | an existing expression on the default path: the blend length `0.05 a.chord` -> `0.051 a.chord` in `write_layer` | HS, HS', HB1 (G and H hashes), HB2 |

## Decision rule

- **done**: W1a to W7 and B2a to B2h, B2-W6, HS, HS', HB1, HB2, E1 to E3 and the three negative controls met.
- **partial**: HS, HS', HB1, HB2, E1 to E3 met and S'' builds both blades fold-free and periodic (W1a to W1c on both), but another
  check missed (each miss an ignored `*_known_miss` record with the limit intact, located by R1). If W5a is met on both blades
  and only W3 misses at a face outside the wake stacks, the acceptance's skewness clause is met but the item is still proposed
  partial with that face located.
- **todo**: S'' does not build one of the blades fold-free, or HS, HS', HB1, HB2 or E1 to E3 missed (an existing result changed).

## Limits (not covered)

- The wake-stack offset from the true section's wake curve (up to about 7e-5 m in the thin layers, about one stack cell in the
  thick layers) is a cost of the rule and is not limited; the wake curve is a construction line, not a wall.
- The trailing-edge wake lines (`p = 0, 1`) keep the true-section shear in the thin layers; no separate limit beyond W5a / W5b.
- Two blades at one specification; no front_gap / n_nose scan (ADR 0696's scan was of S'), no scan of `n_down`, `n_side`, `n_span`.
- No flow solve; no case value (F-778, rule 6); construction S'' is library only.
- The disclosed estimate covers four nodes per layer of lines 4 and 5 only.

## Addenda

### Amendment 1 (2026-11-17, after the code, before any registered run; no limit changed)

- No development build of S'' was made: the code (`wake_stack_remap`, `camber_seam_s_wake_remap_construction()`, the `write_layer`
  change, the test file `f782_cgrid_wake_remap.rs`) was compiled only (`--no-run`).
- HS' was computed as registered on a `git archive` copy of 2acf29d9 (`f782-scratch/base`) with one added test file
  `zz_hs_base.rs` holding only the s4 spec, the two blades, the hash function and builds of
  `camber_seam_s_unstretched_construction()` (`f782-scratch/base_hs.log`): REF `0x0d257b2fcee093ca`, B2 `0xefc53b8c413a845b`, 12 841
  cells and step 5 on both. The constants `HS_PRIME_REF` and `HS_PRIME_B2` of the new test file are those values.

## Addendum 1: results (2026-11-17)

One registered run of the new test file (`f782-scratch/run1.log`, release, `cargo test -p rcfd-mesh --release --test
f782_cgrid_wake_remap -- --nocapture --test-threads 2`: 6 passed, 0 failed, 33 s).

| id | limit | measured | met |
|---|---|---|---|
| W1a | S'' builds REF, cells > 0 | 12 841 cells, construction step 5 (as predicted) | yes |
| W1b | corner Jacobians of one sign, none zero; min volume > 0 | 100 096 of 100 096 negative, 0 zero; min volume 1.20e-13 m^3 | yes |
| W1c | periodicity <= 1e-12 m | 1.96e-17 m | yes |
| W2 | <= 85 deg | **84.953 deg** (face between C (56, 2) and (56, 3), layer 3: the cell between the trailing-edge line 56 and the first ramp line 57, `omega` 0.5). Predicted 84.688 unchanged: **the prediction was wrong** (see deviations) | yes (margin 0.047 deg) |
| W3 | <= 4.0 | 2.7609 (C (37, 7) / (38, 7), layer 6; as predicted, the S' runner-up unchanged) | yes |
| W4 | <= 1000 | 525.6 (C (59, 9), layer 16; as predicted, unchanged) | yes |
| W5a | <= 4.0 | 0.2353 (C (2, 1), between layers 4 and 5, a thick span layer; S' 3.2404) | yes |
| W5b | <= 1.0 | 0.2353 | yes |
| W6 | <= 5e-6 m | 6.48e-7 m (thin cell 7, line 5, row 2; S' 3.27e-5 m) | yes |
| W7 | every node outside the changed set equal in bits | 0 nodes differ outside; 649 inside differ | yes |
| B2a | S'' builds B2 | 12 841 cells, step 5 | yes |
| B2b | no folded cell | 100 096 of 100 096 negative, min volume 1.58e-13 m^3 | yes |
| B2c | periodicity | 2.19e-17 m | yes |
| B2d | <= 85 deg | 84.472 deg (the same face C (56, 2) / (56, 3), layer 3; S' 84.218) | yes |
| B2e | <= 4.0 | 2.8312 (C (37, 7) / (38, 7), layer 16; estimate about 3.4) | yes |
| B2f | <= 1000 | 551.7 (C (59, 9), layer 16; unchanged) | yes |
| B2g / B2h (W5a / W5b) | <= 4.0 / <= 1.0 | 0.2574 (C (62, 1), layers 4 / 5; S' 4.0112) | yes |
| B2-W6 | <= 5e-6 m | 7.17e-7 m (cell 7, line 5, row 2) | yes |
| NC-W5 | the measure on S' reproduces ADR 0696 | REF 3.240429, B2 4.011244, both at C (4, 1) layers 8 / 9 | yes |
| NC-W6 | S' drift > 1e-5 m | 3.27e-5 m | yes |
| NC-W7 | some changed node differs | 649 | yes |
| HS | S hash `0x0105a2fcca924d23` | equal (also the f777 `bs_` test) | yes |
| HS' | S' hashes equal to the base copy | REF `0x0d257b2fcee093ca`, B2 `0xefc53b8c413a845b`, equal | yes |
| HB1 | `cgrid_w35d` three b1 tests | pass (in the whole non-ignored file: 7 passed, 15 ignored, `e1_mesh.log`) | yes |
| HB2 | `f726_cgrid_staggered` b2 (H hash) | pass (whole non-ignored file: 7 passed, 1 ignored) | yes |
| E1 | named tests | `f777_cgrid_unstretched` 10 passed, 2 ignored; `f726_cgrid_staggered` 7 / 1; `cgrid_w35d` 7 / 15; `fan_design_w46a` `s4_realised_default_meshes` passed; `fan01_p10_w36d` c2 passed; `f722_rom_reference_fan` 6 passed, 1 ignored | yes |
| E2 | `--lib` 77 passed, 2 ignored | 77 passed, 2 ignored | yes |
| E3 | baseline_hashes | 3 passed, 1 ignored | yes |

Also run: `cargo fmt -p rcfd-mesh --check` clean; `cargo clippy -p rcfd-mesh --all-targets -- -D warnings` clean; `cargo test -p
rcfd-cli --release --test docs_site` passed (with this ADR and the `docs/meshing.md` paragraph).

### Mutations (`f782-scratch/mut`, `mutate.py`, logs `mut_MU*.log`; each run once against the new test file, HB1 and HB2)

| id | mutation | caught by |
|---|---|---|
| MU1 | field ignored | W3 / W5a on B2 (4.0112), W5b on both, W6 (3.3e-5 / 3.6e-5 m), NC-W7 (as predicted) |
| MU2 | `omega = 1` on the trailing-edge line | W7 (80 nodes outside the set differ), also W1b (folded cells around C (55, 2..3), layer 2), W2 (88.0 deg) and W3 (5.5) on both blades |
| MU3 | the far rows blend from `a` | W1a on both blades (refused: folded line family). Predicted W5b; caught earlier, by the build |
| MU4 | the fade over half of the wake lines | W6 (1.44e-5 / 1.59e-5 m at line 6, as predicted) |
| MU5 | the rule on the lower wake lines only | W1a on both blades (refused: the wake-cut nodes `j = 0` are shared by the lower and upper lines, so the upper line overwrote the lower line's remapped node with the true section). Predicted W5a on line 59; caught earlier, by the build |
| MU6 | blend length `0.05` -> `0.051` chords | HS, HS', HB1 (three hashes), HB2 |

Six of six caught.

### Deviations

- **W2 / B2d prediction wrong.** Registered: the non-orthogonality maximum unchanged at blade line 45 (84.688 / 84.218). Measured:
  a new maximum of 84.953 / 84.472 deg at the in-plane face between rows 2 and 3 of the cell between the trailing-edge line 56
  and the first ramp line 57 (`omega` 0.5), layer 3 (between the last thin hub node layer and the first thick one). The limit
  holds, but on the reference by only 0.047 deg. Probable cause (from the design, not run): the ramp moves line 57 by half of
  `b - a` next to a line that does not move, over the shortest line spacing of the wake (about 3.5e-4 m along it), and `b - a` is
  largest where `rho_k - r_k` is largest (up to 1.8e-4 m at node layers 3 and 7). The registration's estimate of a few degrees of
  rotation there was the right size; the text wrongly assumed the rotated faces were far from 85 deg.
- MU3 and MU5 were caught by the build (W1a) rather than by the checks predicted.
- No development build; no other deviation.

### Not run

The ignored records of the named files; the whole `rcfd-mesh` integration suite and the workspace (the daily gate); flow solves
on S'' meshes; a front_gap / n_nose scan of S''; other specs.

### Decision by the registered rule

**done.** Every registered check is met. S'' builds both blades with no folded cell, periodic, within 85 deg / 4.0 / 1000 (REF
84.953 deg / 2.761 / 525.6, B2 84.472 deg / 2.831 / 551.7). The spanwise skewness of the wake stack falls from 3.2404 / 4.0112
(S') to 0.235 / 0.257, and the thin-layer wake-stack drift from 3.3e-5 m to 6.5e-7 / 7.2e-7 m. The change is confined to the wake
lines of the span layers. S, S' (REF and B2) and the H / G hashes are unchanged in bits, and every named existing test, `--lib`
and baseline_hashes pass. Three negative controls and six of six mutations caught. The non-orthogonality prediction was wrong,
and the reference keeps only 0.047 deg of margin (deviations).

### Remaining questions (each with options and a recommendation)

1. **The 0.047 deg non-orthogonality margin at the start of the ramp.** Options: (a) a follow-up that sets the ramp length from
   the wake line spacing, registered on both blades; (b) leave it, since the limit holds; (c) a ramp over all wake lines.
   Recommendation: (a), before S'' becomes a case value. Option (c) would put shear back on the long cells.
2. **A case value for S''** needs a schema enum value and the schema golden (rule 6). Open for the maintainer, together with F-778.
3. **A scan of S''** (front_gap, n_nose) was not run. Recommendation: run it within the follow-up of question 1.

## For the lead: roadmap entries

> Proposed status: done

> Follow-up of F-777 (ADR 0696 question 1). ADR 0699, 2026-11-17. DONE. New opt-in construction S'' = CGridOptions::camber_seam_s_wake_remap_construction() = camber_seam_s_unstretched + wake_stack_remap: in the span layers with a remapped grid (ADR 0072 W1), the wake-line nodes, stack rows included, move from the true section to the remapped grid with a weight of 0 on the trailing-edge line, rising by a smooth step to 1 over a quarter of the wake lines. Derived: the long wake cells span about 19 deg of the cylinder, so their four-node spanwise faces are curved strips; the pitchwise drift of the thin-layer wake stacks (about 0.4 layer thicknesses per layer) offset the computed cell centres by a fraction of a millimetre against centre distances of 5e-5 m. With the remapped grid the drift per thin layer falls from 3.3e-5 m to 7e-7 m. At the ADR 0118 s4 specs, ComputerFan::rom_reference() blade 0 and the ADR 0696 second blade build with no folded cell, periodic, at 84.95 / 84.47 deg, skewness 2.76 / 2.83 and aspect 525.6 / 551.7. The largest wake-stack spanwise skewness falls from 3.24 / 4.0112 (S', disclosed) to 0.235 / 0.257. Against the registered prediction, the non-orthogonality maximum moved to the cell next to the trailing-edge line at the start of the ramp, 0.047 deg below 85 on the reference. A locality check finds the change only on the wake lines of the span layers. camber_seam_h (cgrid_w35d B1, the ADR 0692 H hash), camber_seam_s (0x0105a2fcca924d23) and camber_seam_s_unstretched (reference and second-blade hashes against the base) are unchanged in bits. f777, f726, cgrid_w35d, f722, fan01_p10_w36d c2, fan_design_w46a s4, --lib and baseline_hashes pass. Six of six mutations caught. Library only; no case value.

> Follow-up proposed: ramp length of the S'' wake-stack remap from the wake line spacing. Deps F-782. Acceptance: Every limit not stated here is registered in the ADR before the run. A new opt-in field of S'' sets the length of the trailing-edge ramp of wake_stack_remap from the wake line spacing so that, at the ADR 0118 s4 specs, ComputerFan::rom_reference() blade 0 and the ADR 0696 second blade build with no folded cell within 84.5 deg non-orthogonality (the 84.953 and 84.472 deg of ADR 0699 at the first ramp cell disclosed), skewness 4.0 and aspect 1000, the wake-stack spanwise skewness stays within 1.0, a registered front_gap and n_nose scan inside the camber_seam range builds a registered fraction, and camber_seam_h, camber_seam_s, camber_seam_s_unstretched and camber_seam_s_wake_remap are unchanged in bits.
