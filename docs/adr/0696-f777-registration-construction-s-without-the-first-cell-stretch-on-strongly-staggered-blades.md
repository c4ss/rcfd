# ADR 0696: F-777 registration: construction S without the first-cell stretch on strongly staggered blades

Status: **REGISTERED 2026-11-17, written before any code of F-777 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-777 was written and nothing was built or run for it. Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-777 (deps F-726), proposed by [ADR 0692](0692-f726-registration-camber-seam-c-grid-on-strongly-staggered-blades.md)
"For the lead" (follow-up) and its remaining questions 1 and 4. Parents: ADR 0692 (construction S,
`CGridOptions::camber_seam_s_construction()`), [ADR 0643](0643-f722-registration-reduced-order-reference-fan-in-the-schema.md)
(`ComputerFan::rom_reference()`), [ADR 0118](0118-wave46a-computer-fan-schema-checker-perturbations.md) (the s4 smoke specs),
ADR 0079 / ADR 0083 (constructions G and H, the camber_seam control range). Number 0696 given by the lead. Branch `f777-work`
from 79b70068 (F-722 and F-726 merged), worktree `w49i-work/f777`, scratch `w49i-work/f777-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests and ignored records not
edited; defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

Clean-room: elementary plane geometry (a geometric stack continued by its next term, the side of a point against a line); the
quality measures are the repository's (`quality::compute_metrics`, `quality_summary`). No source outside the repository was read.

## What F-777 asks

> Every limit not stated here is registered in the ADR before the run. Construction S with the first free cell of the wake lines (or of every line) at the next stack height builds ComputerFan::rom_reference() blade 0 at the ADR 0118 s4 specs and a second strongly staggered blade (outer stagger at least 75 deg) with no folded cell within 85 deg non-orthogonality, skewness 4.0 and aspect 1000 (the reference's 84.69 deg / 3.24 from an ADR 0692 mutation disclosed as already seen), a registered front_gap and n_nose scan of S inside the camber_seam range builds a registered fraction, and camber_seam_h is unchanged in bits.

Clauses: **c1** every limit registered here; **c2** the first free cell at the next stack height (taken: on every line, below);
**c3** it builds the reference blade 0 at s4 with no folded cell within 85 deg / 4.0 / 1000; **c4** the same for a second
strongly staggered blade (outer stagger >= 75 deg); **c5** a registered front_gap and n_nose scan inside the camber_seam range
builds a registered fraction; **c6** camber_seam_h unchanged in bits. From the brief: **c7** the new behaviour opt-in (a new field
and a new constructor), camber_seam_s of ADR 0692 unchanged in bits; **c8** a nose-region check that catches MU1-like changes
(ADR 0692 MU1 survived).

## What was read before this text (disclosed)

Read: ADR 0692 (all, including its addendum, the MU4 numbers and the diagnosis), `fan_cgrid.rs` (the C-line construction
`build_layer` lines 1320 to 1500, the centre line and front edge of S1, `remapped_radii`, `write_layer`, the fallback chain,
`control_range_problems`, the `CGridOptions` constructors), `fan_design.rs` (`rom_reference`, `axial_fan`, `Deviation`,
`BladeDeviation`, the parameter bounds), `fan.rs` (`station_at`, `place`, `camber_line`), the tests `f726_cgrid_staggered.rs` and
`cgrid_w35d.rs` (R-G list, `run_scan_g`). No log of ADR 0692 was opened (in particular not the MU4 log, so the location of the
84.69 deg face is not known to this text).

**Already seen (disclosed, as the acceptance says):** ADR 0692 mutation MU4 (construction S with the first cell `first` instead of
`first / max(cos_t, 0.1)`, everything else S) measured on the reference at s4: fold-free, **84.688 deg, skewness 3.240, aspect
525.6**. The construction registered below on the reference is that mutation exactly, so the reference rows of A2 to A4 are a
confirmation of a seen number, not evidence. The second blade, the scan, the nose check and the stack-growth check are not seen.

Arithmetic done for this text (pencil, below): the stagger, chord and pitch of the second blade; the M-a bound of ADR 0692 for it.

## Design (fixed now): construction S', `camber_seam_s_unstretched`

One new field of `CGridOptions`, `unstretched_first_cell: bool`, default `false` (so `Default`, every existing constructor and
every struct literal with `..` are unchanged), and a new constructor `CGridOptions::camber_seam_s_unstretched_construction()` =
`camber_seam_s_construction()` + `unstretched_first_cell: true`. With `straight_lines` and the new field, the first cell along the
straight segment of line `i` is `first = d1s[i] g^nb` (the next term of the stack's geometric series, the expression without
`straight_lines`) instead of `first / max(cos_t, 0.1)`; the field has no effect without `straight_lines`. Every other expression
is S's (`camber_front`, `straight_lines`, no radial filter, H's smoothing chain, `post_untangle`, `lean_stack`, `radial_remap`,
`gap_fit`, `checked_range`). The one changed expression sits in an `if` on the new field, so S, H, G and the default keep their
bits.

**On every line, not on the wake lines only (c2; taken on the maintainer's standing instruction of 2026-11-11, the
recommendation).** Derivation: the stretch `1 / cos_t` makes the wall-normal height of the first free cell continue the stack
only if the face between the last stack cell and the first free cell stays parallel to the wall, i.e. if the row-`nb + 1` nodes
of neighbouring lines are at nearly the same wall-normal distance. That needs `cos_t` to vary slowly from line to line. ADR 0692
located the failure in the wake (`cos_t` 0.17, 0.34, 0.65 on neighbouring lines), but the same condition fails wherever the wall
normal turns fast against nearly parallel segments: around the trailing-edge wedge and around the leading-edge nose, where the
fan of segments meets a turning normal. Without the stretch the first free cell of every line has length `d1s[i] g^nb` along its
segment, so the row `nb + 1` lies on the curve offset from the stack edge by the same continued geometric term on every line
(exactly the row structure of the pursuit lines, ADR 0079): a quad between rows `nb` and `nb + 1` is then a parallelogram-like
cell whose two side lengths differ only by the stack growth of the neighbours, whatever `cos_t`. The cost: where `cos_t` is
small (on the blade sides `cos_t ~ cos(90 deg - g) ~ 0.21` at the tip), the wall-normal height of the first free cell is
`cos_t d1 g^nb`, about 0.2 times the last stack cell, a jump in the wall-normal growth (not a fold, and not measured by the
three limits; recorded under limits below). A wake-only rule would need a line classification (blade, wedge, wake, nose) and a
second switch for exactly the places where the same premise fails; one rule on every line is the simpler one.

### The second blade (c4; fixed now)

`B2 = ComputerFan { deviation: Some(Deviation { blades: vec![BladeDeviation { stagger_deg: -1.0, chord_rel: 0.2,
..Default::default() }; 7], ..Default::default() }), ..ComputerFan::rom_reference() }.axial_fan(0, 0.03, 0.04)`: the reference
with a per-blade deviation (the schema's blade-parameter API, `axial_fan` applies blade 0's entry: stagger `+ stagger_deg` at
every station, chord `* (1 + chord_rel)`, relative thickness `/ (1 + chord_rel)` so the absolute thickness is kept). Both values
inside the schema bounds (`deviation.blades[].stagger_deg` +-10 deg, `chord_rel` +-0.3). Pencil:

- stagger 68.8 / 74.4 / **76.6 deg** (root / mid / tip; outer stagger >= 75: met by construction); chord 0.0192 / 0.0348 / 0.0432
  m; tip pitch `2 pi 0.058 / 7` = 0.0521 m = 1.21 chords (reference 1.45); thickness 0.0458 / 0.035 / 0.0292 of the chord.
- Why these: a longer chord at nearly the same stagger changes what the ADR 0692 miss depended on (the trailing-edge and wake
  axial spacing, which scale with the chord, the pitch-to-chord ratio, the nose size), while the outer stagger stays between the
  item's 75 deg threshold and the reference's 77.6 deg, so that the `90 - g` shear floor of the topology (ADR 0692: about `g` deg
  of non-orthogonality for uniform parallelograms, 76.6 here) is of the reference's size. A higher stagger would raise the floor
  itself, which this item does not address.
- ADR 0692 mechanism M-a on B2 (camber_seam_h's axial front edge): at the tip `front_gap c tan g` = 0.3 x 0.0432 x 4.20 = 0.0545
  m against the corner offset `0.95 tan(g) front_gap c / 3 + p / 2` = 0.0172 + 0.0261 = 0.0433 m; at the root 0.0148 against
  0.0143 m. Both exceed, so every suction-side nose target of H lies on the pressure side: H is predicted to refuse B2 (NC1').

## Development protocol (registered)

The code is written to this design. Before the registered run, development builds are allowed **only to fix defects** (code that
does not do what this design says) and print only the build status, the construction step and, on a refusal, the generator's
located message; no quality metric is computed or printed in development. No design variant is permitted: if S' as registered
misses, the miss is the result (a `*_known_miss` record). Every development build is listed in an amendment. The B-S hash (below)
is computed on a `git archive` copy of 79b70068 with only the new test file added, before the registered run.

## Common set-up (fixed now)

The s4 specs of `fan_design_w46a.rs` `smoke_specs()` copied verbatim (as ADR 0643, 0692): `CGridSpec { n_up 8, n_chord 24,
n_down 8, n_side 10, n_nose 6, n_core 2, n_span 10, n_gap 7, n_bl 2, growth 1.2, first_cell Height(5e-5), front_gap 0.3,
include_blade true }`. `REF = ComputerFan::rom_reference().axial_fan(0, 0.03, 0.04)`, `B2` above. `S' =
camber_seam_s_unstretched_construction()`. The fold measure (corner Jacobians), periodicity and hash are those of
`f726_cgrid_staggered.rs` (copied into the new test file `crates/rcfd-mesh/tests/f777_cgrid_unstretched.rs`). Quality:
`quality_summary(&mesh)` (maxima over every face and cell).

## Checks (limits fixed now)

| id | clause | what | limit | estimate behind it |
|---|---|---|---|---|
| A1a | c3 | `build_cgrid_mesh_with(&REF, &s4, &S')` | Ok, cells > 0 | MU4 of ADR 0692 (seen) |
| A1b | c3 | no folded cell: every corner Jacobian of the C- and F-block hexes of one sign, none zero; `min_volume` > 0 | met | seen (MU4) |
| A1c | c3 | periodicity of the seams (every layer) | <= 1e-12 m | the seam lists are translates |
| A2 | c3 | max non-orthogonality | <= 85 deg | seen: 84.688 |
| A3 | c3 | max skewness | <= 4.0 | seen: 3.240 |
| A4 | c3 | max aspect ratio | <= 1000 | seen: 525.6 |
| B2a..B2f | c4 | A1a, A1b, A1c, A2, A3, A4 on B2 (same limits) | as A1a..A4 | not seen. Estimate: builds (the S1 and S2 orderings of ADR 0692 hold at every stagger below 90 deg); A4 about 600 (the wake cells grow with the chord, the radial first layer is fixed: 525.6 x 1.2 if the reference's worst cell scales with the chord); A2 about 84 +- 2 deg (the floor is 1 deg below the reference's, the stack-edge kinks scale with the stack, not the chord), so A2 may miss; A3 about 3 +- 1 |
| NR | c8 | nose-region check, on the algebraic grid of S' (`S'` with `smoothing: false`, the fallback chain's step-5 grid) of REF, every layer `k`: with `rho_k` the remapped radius of the layer (the ADR 0072 W1 formula of `remapped_radii`, reproduced in the test from `radii` and `n_span`; the front-edge node of a layer is the node of the in-plane grid at `rho_k`, `write_layer` blend weight 0 there), `st = station_at(rho_k)`, `(z_le, s_le)` = `camber_zs(st, 0)`, `sigma` = the generator's slope `(camber_zs(st, 1e-6) - camber_zs(st, 0))`: the expected nose point is `N = (z_N, s_N)`, `z_N = z_le - front_gap c / sqrt(1 + sigma^2)`, `s_N = s_le + sigma (z_N - z_le)`; the nose node `c_node(n_i / 2, n_side, k)` at `(z, rho_k theta)` gives `dz = z - z_N`, `ds = rho_k theta - s_N` | `|dz|` <= 1e-12 m and `|ds|` <= 1e-9 m on every layer | S1 puts the nose target on the extended camber line exactly (roundoff only, about 1e-17 m). ADR 0692 MU1 moves it by about 0.08 c in `s` at the tip (0.11 c at the root), i.e. 2.9e-3 m: 6 orders above the limit |
| NR2 | c8 | on the same grid, the front-edge nodes `c_node(h_c + b, n_side, k)`, `b = 0..2 n_nose` (`h_c = (n_i - 2 n_nose) / 2`), have one `z` and strictly increasing `s` with `b`, so the lower half (`b < n_nose`) lies on the side of the extended camber line opposite to the upper half | met on every layer | the front edge is one `z` (S1); the nodes are `s_lo + lo_part`, `s_n + hi_part` (monotone) |
| A6 | c2 | first free cell at the next stack height, on the algebraic grid of S' of REF: on every C line `i` with `stack_cells[k][i] = nb >= 1`, in the layers without the remap blend (`k = 0`, `k = n_span` and the tip-gap layers, where the in-plane grid of the layer is written unblended; in-plane coordinates `(z, rho_k theta)`), the ratio of the length of the first free cell (row `nb` to `nb + 1`) to the last stack cell (row `nb - 1` to `nb`) | `|ratio - growth| <= 1e-9 growth` on every such line | the stack is `w + n sum d1 g^q` and the first free cell `d1 g^nb` along the segment: ratio `g` up to roundoff |
| NC-A6 | c2 | the A6 measure on the algebraic grid of S (ADR 0692, with the stretch) of REF | at least one line with `|ratio - growth| > 0.01 growth` | the stretch `1 / cos_t` is 1 only where the segment is along the normal; on the blade sides `cos_t` about 0.2 |
| SC | c5 | scan of S' inside the camber_seam range: REF and B2, each `front_gap` in {0.15, 0.3, 0.6, 1.0} x `n_nose` in {4, 6, 9}, the other s4 values (all 12 combinations have `control_range_problems` empty: `n_nose / n_chord` 0.167, 0.25, 0.375 within [0.15, 0.375], `front_gap` within [0.15, 1.0]); "builds" = A1a, A1b and A1c met | per blade, at least 9 of 12 build (fraction >= 0.75) | the S1 / S2 orderings of ADR 0692 do not depend on `front_gap` or `n_nose`; the risks are the corner station at small `front_gap` and the long nose fan at `front_gap` 1.0 (the front edge 0.68 pitch along the camber line at the tip). Quality (A2 to A4) per combination reported, not a gate |
| SC0 | c5 | every scan combination is inside the range (`control_range_problems(spec)` empty) | 24 of 24 | the arithmetic above |
| BS | c7 | S (`camber_seam_s_construction()`) on REF at s4: FNV-1a mesh hash on the branch equal to the hash computed by the same test on a `git archive` copy of 79b70068 (test file only) | equal | the new expression is behind the new field |
| B1 | c6 | `cgrid_w35d` `b1_existing_constructions_are_bit_identical`, `b1_construction_g_is_bit_identical`, `b1_construction_h_is_bit_identical` | pass | no expression of these paths changes |
| B2h | c6 | `f726_cgrid_staggered` `b2_camber_seam_h_on_the_default_computer_fan_is_bit_identical` (H on the default ComputerFan s4, recorded hash of ADR 0692) | pass | as B1 |
| B3 | c6 | by name: `cgrid_w35d` `p1_p8_fan01_mesh_builds_closes_is_periodic_and_exact`, `c1_fan01_h_mesh_builds_closes_is_periodic_and_exact`, `control_range_is_checked_only_by_the_camber_seam_construction`, `r_g_out_of_range_combinations_are_located_errors`; `fan01_p10_w36d` `c2_fan01_h_mesh_is_accepted_by_the_solver_and_the_rules`; `fan_design_w46a` `s4_realised_default_meshes`; the whole non-ignored `f722_rom_reference_fan` and `f726_cgrid_staggered` files | pass | no line they run changes |
| B4 | c6 | `cargo test -p rcfd-mesh --release --lib` | 0 failed, the pass count of ADR 0692 addendum 1 (77 passed, 2 ignored) | no unit test touches the new field |
| B5 | c6 | `cargo test -p rcfd-api --release --test baseline_hashes` (`fan_cgrid.rs` is called by the fan case builder) | pass | |
| R1 | reported | S' on REF and B2: construction step, cells, `QualitySummary`, location of the maxima | none | |

### Negative controls

| id | control | limit |
|---|---|---|
| NC1' | camber_seam_h on B2 at s4 | refused (any located error), predicted by M-a (pencil above) |
| NC2' | S (with the stretch) on B2 at s4 | reported: build, step, A2 / A3 / A4 (predicted: builds, A2 or A3 above its limit, as on the reference) |
| NC-A6 | above | above |
| NC-NR | the NR measure applied to S's algebraic grid of REF with `front_gap` 0.6 against the expected point `N` computed for `front_gap` 0.3 | `|dz|` > 1e-6 m or `|ds|` > 1e-6 m on some layer (the check sees a moved nose) |

## Planned mutations (scratch copy of the worktree, each run once against the new test file and B1)

| id | mutation | expected to be caught by |
|---|---|---|
| MU1 | ADR 0692 MU1: under `camber_front` the existing axial turn over `front_gap c / 3` kept upstream of the leading edge (centre line) | NR (nose off the line by about 2.9e-3 m) |
| MU2 | the new field ignored (the stretch kept under `straight_lines`) | A6 (ratio), and A2 / A3 on REF (ADR 0692's 86.97 / 7.43) |
| MU3 | the stretch removed for S too (`unstretched_first_cell` treated as true whenever `straight_lines`) | BS (hash of S changes) |
| MU4 | the first cell `0.5 d1 g^nb` under the new field | A6 |
| MU5 | an existing expression on the default path: `lam_u = front_gap c / 3` -> `/ 2.9` | B1 (G and H hashes), B2h |
| MU6 | the front edge `z_f` of S1 with `front_gap * 1.01` | NR (`dz`) |

## Decision rule

- **done**: A1a to A4 and B2a to B2f met, NR, NR2, A6, NC-A6, NC-NR, NC1' met, SC met on both blades, SC0, BS, B1 to B5 met.
- **partial**: BS, B1 to B5 met and S' builds both blades fold-free (A1a to A1c, B2a to B2c), but a quality limit (A2 to A4, B2d to
  B2f), SC, NR, NR2 or A6 missed (each miss an ignored `*_known_miss` record with the limit intact, located by R1).
- **todo**: S' does not build one of the blades fold-free, or BS or B1 to B5 missed (an existing result changed).

## Limits (not covered)

- The wall-normal growth jump at the stack edge on the blade sides (the first free cell `cos_t` times the continued stack height,
  about 0.2 at the tip) is a cost of the rule and is not limited by the three quality measures; no check here bounds it.
- Two blades at one specification plus a 12-point scan per blade; `n_side`, `n_chord` and `n_span` are not scanned. The topology
  keeps the `90 - g` shear floor; a blade above about 80 deg outer stagger is not expected to meet 85 deg with any construction of
  this topology.
- No flow solve on the meshes; no case value (ADR 0692 follow-up F-778); h_slit is not touched.
- The reference rows A2 to A4 are a seen number (above).

## Addenda

### Amendment 1 (2026-11-17, after the code, before any registered run; no limit changed)

- No development build of S' was made: the code (`unstretched_first_cell`, `camber_seam_s_unstretched_construction()`, the test
  file `f777_cgrid_unstretched.rs`) was compiled only (`--no-run`).
- The BS hash was computed as registered on a `git archive` copy of 79b70068 (`f777-scratch/base`) with one added test file
  `zz_bs_base.rs` holding only the s4 spec, the reference, the hash function and a build of `camber_seam_s_construction()` (the
  base has no S', so the whole new test file does not compile there): `0x0105a2fcca924d23`, 12 841 cells, step 5
  (`f777-scratch/base_bs.log`). The constant `BS_HASH` of the new test file is that value.
