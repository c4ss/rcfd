# ADR 0583: F-662 registration: two-sided shell edge ties for non-matching cyclic boundaries

Status: **REGISTERED 2026-11-15, written before any code of F-662 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-662 was written, compiled or run: the files under "What was read" were
read, and the arithmetic below was done by hand (no script, no rcfd run; disclosed). Results go in the addenda at the end;
nothing above the addenda is changed after a run.

Roadmap item F-662 (dep F-648), the follow-up proposed by [ADR 0575](0575-f648-registration-shells-with-interpolated-cyclic-boundaries-and-axis-nodes.md)
(its I2b known miss and remaining questions 1 and 2). Number 0583 given by the lead. Branch `f662-work` from 9559dc28, worktree
`w49i-work/f662`, scratch `w49i-work/f662-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no edit of existing tests; no new dependency, no
unsafe code; clean-room. `docs/ROADMAP.md` and `docs/roadmap.json` are not edited. Every choice below that the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-662 asks

> Every limit not stated here is registered in the ADR before the run. With CyclicOptions::shell_extensions, a non-matching shell cyclic boundary also ties the low nodes that no high-node image reaches (to the high edge rotated back, or by a node-to-edge mortar), so that neither side is left free; the ADR 0575 frustum with the low side finer (2:3) has the four lowest frequencies of each harmonic within 2e-2 of the matched sector, falling under refinement, with no mode local to the interface among them; the rotating paths (ADR 0561) on the ADR 0575 cap at two speeds against the full model within 1e-8; models without the option keep their bits.

Clauses: **b0** every limit registered here; **b1** with `shell_extensions` and `interpolation` the low nodes that the high ties
do not determine are tied to the high shell edges rotated back, so that no part of either side is free of the other; **b2** the
ADR 0575 frustum with the low side finer (mesh I2b): the four lowest frequencies of each harmonic within 2e-2 of the matched
sector; **b3** falling under refinement; **b4** no mode local to the interface among them; **b5** the rotating paths of ADR
0561 on the ADR 0575 cap at two speeds against the full model within 1e-8; **b6** models without the option keep their bits.

## What was read (no run)

`AGENT_RULES.md`; the brief; ADR 0575 (D1 to D4, the checks, the I2b diagnosis, the remaining questions); ADR 0561 (D1, D3, D4:
the rotating sets A, W and the list protocol); ADR 0556 D2 (the rotation vector turns with `R`); ADR 0456 D4 (the interpolated
rows); the sources at 9559dc28: `cyclic.rs` (`boundary`, `shell_edge_weights`, `CyclicBoundary`), `mpc.rs` (`cyclic_rows`, the
elimination `process` with its chain refusal: a row whose designated unknown appears in the expression of a dependent it
substitutes is refused), `rotating.rs` (`RotatingSector::new`, `frame`: no refusal of an extended boundary), `rotating/whirl.rs`
(`whirl_modes` of the part and of the sector); the tests `sa_cyclic_shell_extensions.rs` (every test), `sa_rotating_cyclic_shells.rs`
(the `compare` protocol, sets A and W).

### Audit of the existing tests on the path this item changes

Only a boundary with `shells`, `shell_extensions` and `interpolation` reaches the new code, and among those only one on which the
high ties leave some low nodes undetermined (D2: the set `F` is not empty) changes. The tests of ADR 0575 on that path:

| Test | Mesh | `F` | Effect of this item |
|---|---|---|---|
| `i1_shifted_high_column_against_the_matched_sector` | I1 (7 high, 7 low, high nodes 1e-6 of a segment off) | empty (arithmetic 2) | none: the same boundary, bits checked by B4 |
| `i2a_i3_met_parts_of_the_two_to_three_boundaries` | I2a level 1, 2 | empty | none on I2a (B4) |
| the same test | I2b level 1, 2 | 8 / 16 nodes | the I3 ratio of I2b it asserts (`<= 0.7`) is computed on the new tie: its numbers change; expected still met (J3 estimate) |
| `i2_i3_..._known_miss` (ignored record) | I2a, I2b | as above | its I2b numbers change; the record is expected to pass when run with `--include-ignored` (the miss it records is the one this item removes). Not edited; retiring it is the maintainer's decision |
| `r1_interpolated_rotation_rows_hold` | I2b | 8 nodes | it checks the rows of `bd.interpolated()`; after D3 that list holds the 5 kept high nodes (was 9): it checks fewer nodes, every row it checks is still a row of the transformation; expected still met |
| `nc1_to_nc7_refusals` NC3, NC4, NC6 | I2b variants | 8 nodes | the same refusals in the same order (D4: the high ties and their unknown counts are checked before the low ties are built) |
| A1, A2, B3, NC1, NC2, NC5, NC7 | cap, matched | not interpolated | none (no line on their path changes) |

No existing assertion needs to change; if one fails, it is reported, not edited.

## Design

### D1. Why the low nodes need a tie, and why not every low node (derivation)

ADR 0575 ties every high node `H` to the low edges rotated by `R` (weights `w_{H,l}`, linear on the edge): `U_H = P sum_l w_{H,l}
U_l`, `P = e^{i theta} R` on the complex amplitude `U = a + i b` (ADR 0386: `a_H = R (c a_L - s b_L)`, `b_H = R (s a_L + c b_L)`).
Write `W` for the matrix of the weights (rows the high nodes, columns the low nodes). A low field `v` with `W v = 0` moves no high
node: it is restrained only by the low sector's own elements, through one side. In I2b (low 12 segments, high 8) the high node at
`s = 1.5/12` takes `(L_1 + L_2)/2`, so `L_1 - L_2` is such a field, and the I2b record found the mode it gives among the four
lowest (1091.48 Hz at `k = 0` and `k = 1` alike). These are "the low nodes that no high-node image reaches": precisely, the
support of the null space of `W`,

`F = { l : v_l != 0 for some v with W v = 0 }`

(the support of a subspace does not depend on the basis). In I1 and I2a `W` has full column rank (arithmetic 2), so `F` is empty
and nothing changes.

The obvious two-sided rule, every high node tied to the low edges and every node of `F` tied to the high edges as well, over-
constrains. Counting on one group of I2b (`L_0 .. L_3` against `H_0 .. H_2`, `H_0 = L_0`, `H_2 = L_3` coincident): the null space
has dimension 1 (`L_1 - L_2`), but the rule adds two rows (`L_1` and `L_2`) beside the row of `H_1`. By hand: `L_1 = (H_0 + 2
H_1)/3`, `L_2 = (2 H_1 + H_2)/3`, `H_1 = (L_1 + L_2)/2` give `L_1 = (2 L_0 + L_3)/3`, `L_2 = (L_0 + 2 L_3)/3`: the interface is
locked to one straight piece over the group, coarser than either side (in a general non-nested pair, linear between the few common
nodes). This is the over-constraint of a double-pass node-to-segment tie with linear elements; it is not taken.

### D2. The rule: the finer side follows on the part where it is finer

The classical rule of the one-sided tie and of the mortar method is that the finer side is the slave (the dependent side) and the
coarser side the master; ADR 0575 I2a (high side finer) met its limit at 7.4e-3 with the high side the slave. D2 applies it
locally, where `W` is rank deficient:

1. `F` as in D1, computed from `W` (D3).
2. A high node whose weights lie wholly on `F` (every weight above `tau = 1e-10` on a node of `F`) is **released**: it takes no
   row of its own and becomes a master. The other high nodes keep their ADR 0575 rows unchanged.
3. Every node `f` of `F` follows the high shell edges rotated back by `R^T`: its position `x_f` is projected on the mapped high
   edges exactly as D3 of ADR 0575 projects a high node on the mapped low edges (the same choice of the best edge, the same
   tolerance rule with the mean mapped high edge length, `FemError::TieGap` above it), weights `v_{f,b}` over high nodes `b`.
4. The row of `f` (derivation): sector 1's low node `f` sits at `R x_f`, where sector 0's high field is `sum_b v_{f,b} U_b`;
   sector 1's value there is `P U_f`. So `P U_f = sum_b v_{f,b} U_b`, that is `U_f = P^{-1} sum_b v_{f,b} U_b`, `P^{-1} = e^{-i
   theta} R^T`; in real form `a_f = R^T (c A + s B)`, `b_f = R^T (-s A + c B)` with `A + i B = sum_b v_{f,b} U_b`. For a kept
   high node `b` the ADR 0575 row gives `P^{-1} U_b = sum_l w_{b,l} U_l` exactly; for an axis node `p` on the high edge, `P^{-1}
   U_p = U_p` (ADR 0456 arithmetic 5, its own image). Substituting these, with `A_{f g}` the weight so collected on nodes `g` of
   `F`, `C` the low nodes not in `F` (axis nodes included), `D` the released high nodes:

   `(I - A) U_F = B_C U_C + P^{-1} B_D U_D`.

   The weights are scalars (the same for every component), so the solve `G = (I - A)^{-1} [B_C | B_D]` commutes with `P^{-1}` and
   is done once per boundary (Gaussian elimination with partial pivoting, a pivot below `1e-10` refused, key `cyclic`). The rows,
   each component `i` of each triple (`0 .. 2`, and `3 .. 5` for six-unknown nodes under `shells`, ADR 0556 D2):

   `a_{f,i} - sum_{g in C} G_{fg} a_{g,i} - sum_{d in D} G_{fd} sum_j R_{j i} (c a_{d,j} + s b_{d,j}) = 0`,
   `b_{f,i} - sum_{g in C} G_{fg} b_{g,i} - sum_{d in D} G_{fd} sum_j R_{j i} (-s a_{d,j} + c b_{d,j}) = 0`,

   designated `a_{f,i}` and `b_{f,i}`. No term of these rows is a dependent whose expression contains `f` (the kept high nodes,
   whose expressions do, were substituted by hand), so the chain refusal of the elimination is not met; the kept high rows that
   contain `f` are updated by its Gauss-Jordan step.
5. Unknown counts: the counts of the high nodes against their weights are checked (the message of ADR 0575 D1.4) before step 3,
   and in the rows of step 4 every node of a row must have the count of `f` (refused, key `cyclic`, naming both nodes and
   counts).

Count on one I2b group: rows `H_0`, `H_2` (kept), `L_1`, `L_2` (tied), `H_1` released: four rows over 4 + 3 = 7 nodes, 3
independent, the coarse count, as the one-sided tie with the fine side slave. The interface left is the coarse high edge with
the fine low nodes on it; between `L_1` and `L_2` the high edge has its node `H_1`, so a gap of the second difference
`(2 H_1 - H_0 - H_2)/6` remains (the size of the gap of I2a, `O(h^2 u'')`), and no low field is free of the high side.

### D3. `F` from the weights (cyclic.rs)

`W` dense, rows the ADR 0575 slaves in their order, columns the low nodes (ascending; axis nodes included). Reduced row echelon
form by columns left to right with partial pivoting over the remaining rows, a pivot taken when `|pivot| > 1e-10` (the weights
are at most 1); every column without a pivot gives the null vector `e_j - sum_{pivot rows r} R_{r j} e_{p(r)}`; `F` = the columns
with a component above `tau = 1e-10` in some such vector, axis nodes excluded. Cost `O(n_H n_L^2)` per call of `boundary`: a few
milliseconds at a few hundred boundary nodes; a boundary of thousands of nodes is slow (Limits).

`CyclicBoundary` gets a field `two_sided: Vec<(usize, Vec<(usize, f64)>, Vec<(usize, f64)>)>` (node of `F` ascending, its `G`
over `C`, its `G` over `D`, zero entries dropped) and a public accessor `two_sided()`; `interpolated()` lists the kept high nodes
only; `is_extended()` also counts `two_sided`. With `F` empty `boundary` returns exactly what it returned before (the same
vectors, the new one empty), so every boundary without an undetermined low node keeps its bits (B4).

### D4. Order of the checks

In `boundary` with `shells` and `interpolation`: (i) the ADR 0575 high weights (its `TieGap`, as before); (ii) `F`; if `F` is
empty, return as before; (iii) the unknown counts of the high weights (the ADR 0575 message); (iv) the released nodes and the
low weights (`TieGap` naming a node of `F`); (v) `G`. In `mpc::cyclic_rows`: pairs, kept high nodes (as before), the rows of
`two_sided`, axis nodes. The k = 0 transformation and the harmonic transformations take the same rows (the same function).

### D5. Test geometries (fixed now)

The ADR 0575 D4 frustum and cap, unchanged (axis `n = (1, 2, 2)/3`, `o = (0.3, -0.2, 0.5)`, `N = 6`, steel, `t = 0.05`, Shell3
merged strips; modal options `ResidualTest::Backward`, tolerance `1e-12`). **J** meshes: I2b at level 1 (`n_c = 4`, column 0
with 12 segments, the others 8) and level 2 (`n_c = 8`, 24 and 16), against the matched sector (all 8, all 16), `extended()
.interpolation(NodeToSurface)`. **Cap**: A1 (Shell4 with Shell3 at the pole) and A2 (all Shell3), sector with `axis_nodes`, full
model of 12 columns.

Arithmetic (by hand). I2b level 1: column 0 nodes `a = 0 .. 12` at `s = a/12`, high `b = 0 .. 8` at `b/8 = 1.5 b/12`. High `b`
even coincides with low `1.5 b`; `b` odd sits mid-way between low `(3b-1)/2` and `(3b+1)/2`. `W` 9 x 13, rank 9, null space
spanned by `L_{3g+1} - L_{3g+2}`, `g = 0 .. 3`: `F` = column 0 nodes `a in {1, 2, 4, 5, 7, 8, 10, 11}` (8 nodes); released
high `b in {1, 3, 5, 7}`; kept `b in {0, 2, 4, 6, 8}`. Low weights: `L_{3g+1}` on the high edge `(H_{2g}, H_{2g+1})` at `t = 2/3`
(`v = 1/3, 2/3`), `L_{3g+2}` on `(H_{2g+1}, H_{2g+2})` at `t = 1/3` (`2/3, 1/3`); `A = 0`; `G`: `L_{3g+1}`: `1/3` on `L_{3g}`,
`2/3` on `H_{2g+1}`; `L_{3g+2}`: `2/3` on `H_{2g+1}`, `1/3` on `L_{3g+3}`. Level 2: the same with 16 nodes of `F` and 8 released.
I1: `W` = identity plus `O(1e-6)` off the diagonal (and roundings), full rank; I2a: `W` 13 x 9 with each low node in some row
and the rows `H_{3g}` = `L_{2g}` picking every even column, the odd columns picked by the rows at thirds: rank 9 (full column
rank), `F` empty.

## Checks (limits fixed now)

Measures as ADR 0575 (`d_k` over the four lowest of each harmonic, sorted; `max_k d_k`). **Localisation** `rho` of a mode: per
node the translation amplitude `sqrt(|a_u|^2 + |b_u|^2)`; `rho = max over the nodes of the boundary columns (0 and n_c) / max
over the nodes of the interior columns`. A mode of the rotor of harmonic `k` has the same amplitude in every column for `0 < 2k <
N` and `k = 0`; at `2 k = N` it varies as `|cos(3 phi + phi_0)|`, and the interior columns of level 1 span a quarter period, so
`rho <= 1/cos(pi/4) = 1.41` for any `phi_0`, plus the discretisation difference of the columns (a few per cent). A mode local to
the interface has its amplitude in the boundary column and decays into the sector within an element or two.

| Id | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| J0 | b1 | I2b level 1: `two_sided()` lists exactly the 8 column-0 nodes of arithmetic, each with the `G` of arithmetic (to `1e-12`); `interpolated()` exactly the 5 kept high nodes; I1 and I2a: `two_sided()` empty, `interpolated()` of length 7 and 13 | exact sets; `1e-12` | D2, D3, arithmetic |
| J1 | b1, b2 | I2b level 1 against the matched sector, `max_k d_k` (each `d_k` reported) | `<= 2e-2` | the fine side follows the coarse edge as in I2a: the discretisation difference of one refined strip, measured 7.4e-3 for I2a (ADR 0575); `~ 5e-3 .. 1e-2` |
| J2 | b4 | every one of the four lowest modes of each harmonic of J1 (and of J3 level 2): `rho` | `<= 2` | global modes `<= 1.41` (above) plus a few per cent |
| J3 | b3 | I2b level 2 `max_k d_k` over level 1 | `<= 0.7` | `O(h^2)`: 0.25 (I2a measured 0.125) |
| R2 | b1 | I2b level 1, `k = 1`, every mode: for every node `f` of `two_sided()`, `max |U_f - P^{-1} sum_b v_{f,b} U_b|` over the translations and rotations, over `max |U|` of the mode, with `v_{f,b}` from the test's own geometry (`b = floor(8 s_f)`, `t = 8 s_f - b`, the high nodes of column 4) | `<= 1e-13` | substitution of the transformation, as ADR 0575 R1 (1.65e-15) |
| RA4, RA3 | b5 | the cap A1 (RA4) and A2 (RA3): `RotatingSector` (set A of ADR 0561: stress stiffening and spin softening, `opts(4)`) against `RotatingPart` of the full cap (`opts(24).shells(true)`) at `Omega in {0.2 omega_1, 0.4 omega_1}`, `omega_1` the lowest sector frequency at rest over `k = 0 .. 3`: the ADR 0561 D4 list protocol: ring list complete, counts equal, per harmonic `max |f_s/f_r - 1|` | complete; equal; `<= 1e-8` | the same discrete problem harmonic by harmonic (ADR 0575 D2, A1 measured `<= 2.9e-15` at rest; ADR 0561 MA `<= 2.3e-15`): rounding |
| RW4, RW3 | b5 | the same with `coriolis(true)`: `RotatingSector::whirl_modes` against `RotatingPart::whirl_modes`, the omegas | as RA | ADR 0561 W measured `<= 1.9e-14` |
| RW0 | b5 (set-up) | the cap A1 at `0.4 omega_1`, `k = 1`: the largest `|omega_W/omega_A - 1|` of the sorted whirl list against the sorted set-A list below both cut-offs (as ADR 0561 W0) | `>= 1e-4` | the Coriolis split of ADR 0561 W0 was 0.25 on the frustum; far above rounding |
| B1 | b6 | every existing rcfd-fem test passes (release), in particular `sa_cyclic_shell_extensions`, `sa_cyclic_shells`, `sa_rotating_cyclic_shells`, `sa_tie` | pass | D3, D4 |
| B2 | b6 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no default path changes |
| B4 | b6 | FNV-1a 64 of the bits of the `k = 0 .. 3` frequencies of I1 (shifted sector) and I2a level 1, against the hash recorded by running the same function on the unchanged library (this registration plus only the new test file; a set-up run before any library change, disclosed in the addendum) | equal | D3: `F` empty, the same boundary |

## Negative controls

| Id | Control | Expected |
|---|---|---|
| NC8 | I2b level 1 with the low node `(0, 1)` moved along the radius by `0.2` of the mean low segment length and `TieOptions::tolerance(0.15 x mean)` (by hand: the high node `(4, 1)` then lies `~0.09 x mean` off the moved low edge, within the tolerance; the low node `~0.18 x mean` off the high edge `(H_0, H_1)`) | `FemError::TieGap` naming the low node `(0, 1)` |
| NC9 | the NC6 model of ADR 0575 (a three-unknown Hex8 node in place of the high node `(4, 3)` of I2b): the existing NC6 assertion, repeated | refused, key `cyclic`, "high node `<corner>` has 3 unknowns", "has 6" |

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| M1 | `F` taken empty (the two-sided tie off: ADR 0575 behaviour) | J0, J1, J2 (the record of ADR 0575: a local mode among the four lowest), R2 |
| M2 | no high node released (the double-pass of D1) | J0 (and J1 if the locking of D1 exceeds the limit; not required) |
| M3 | `R` in place of `R^T` in the rows of the released high nodes | R2, J1 |
| M4 | the sign of `s` flipped in those rows | R2 (`k = 1`), J1 |
| M5 | the low edge weights swapped (`(A, t), (B, 1 - t)`) | J0, R2, J1 |
| M6 | the rotation rows (components 3 to 5) of the tied low nodes not emitted | R2, J1 |
| M7 | the substitution of the kept high nodes left out (their share of `v` dropped) | J0, R2, J1 |
| M8 | the unknown-count check before the low ties removed | NC9 (a `TieGap` in its place) |
| M9 | the `TieGap` check of the low ties removed | NC8 |
| M10 | the axis rows of the rotations not emitted (ADR 0575 D1.5; to show the rotating checks see the pole) | RA4, RA3, RW4, RW3 |

No mutation is declared equivalent.

## Decision rule

**done**: every check J0 to B4 met, NC8 and NC9 as stated, every mutation caught, and the audited existing tests pass. **partial**:
any registered check missed (recorded as a `*_known_miss`), any mutation uncaught, or any check met only after a post-run
amendment. **todo**: J1 to RW4 not run.

## Limits (what is not covered)

- The node-to-edge mortar (still refused with shells); D2 is a collocation rule.
- Boundaries other than the nested 2:3 groups checked here: D2 is derived in general, but on a boundary where a released high
  node is absent while `F` is not empty (for example a 1:2 low-finer boundary, every high node coincident) the tied low nodes
  follow kept high edges only; it is the conforming tie there by the same argument, but not run. Non-nested ratios with long
  stretches between common nodes are not checked.
- The cost of D3 grows as `n_H n_L^2`; boundaries of thousands of nodes are not timed.
- The rotating paths with an interpolated (two-sided) shell boundary are not checked against a full model (the full model of a
  non-matching sector is not a rotor of identical sectors); `full_rotor` still refuses an extended boundary.
- Static and harmonic static solves go through the same rows but are not checked here.
- A tied low node that is also on a second constraint (a user linear row) is not covered.
