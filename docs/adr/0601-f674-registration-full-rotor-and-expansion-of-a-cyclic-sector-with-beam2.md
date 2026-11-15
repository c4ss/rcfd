# ADR 0601: F-674 registration: full_rotor and the expansion of a cyclic sector with Beam2

Status: **REGISTERED 2026-11-15, written before any code of F-674 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-674 was written, compiled or run: the files under "What was read" were
read, and the arithmetic below was done by hand (no script, no rcfd run; disclosed). Results go in the addenda at the end;
nothing above the addenda is changed after a run.

Roadmap item F-674 (dep F-672), the follow-up proposed by
[ADR 0595](0595-f672-registration-beam2-elements-in-the-cyclic-constraint.md) (its remaining question 1, its D4). Number 0601
given by the lead. Branch `f674-work` from the tip of `f675-work` (e493c6df: F-672 `CyclicOptions::beams` and F-675
`RotatingOptions::beams`), worktree `w49i-work/f674`, scratch `w49i-work/f674-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests not edited; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-674 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. cyclic::full_rotor of a sector with CyclicOptions::beams gives each sector copy of a Beam2 block the orientation R^j v, and FullRotor::expand, expand_field and expand_load of the ADR 0595 wheel sector give the fields of the independently built ring within 1e-10; the defaults bit-identical.

Clauses: **c0** every limit registered here; **c1** `full_rotor` of a sector with the beams option gives copy `j` of a Beam2
block the orientation `R^j v`; **c2** `FullRotor::expand` of the sector modes gives modes of the independently built ring within
`1e-10`; **c3** `FullRotor::expand_field` of the sector's static solutions gives the ring's static fields within `1e-10`; **c4**
`FullRotor::expand_load` of the sector's harmonic loads gives the ring's loads within `1e-10`; **c5** the defaults
bit-identical.

## What was read (no run)

`AGENT_RULES.md`; the brief; ADR 0595 (the wheel sector W, the independent ring, D3 the objectivity `R e_1 x R v = R (e_1 x v)`,
D4 the refusal of `full_rotor`, its addendum: the measured S, M, P numbers); ADR 0598 (its amendment 1: compare `+0` and `-0` as
numbers, not bits, unless bits are meant); ADR 0386 D1 (the full rotor, the expansion and its scale `sqrt(2/N)`, `sqrt(1/N)`);
ADR 0395 (`expand_field`, `expand_load`); ADR 0561 D1 (`triplets`: a six-unknown node expands its rotations by the same `R^j`).
Sources at e493c6df: `cyclic.rs` (`rotation_matrix`, `apply`, `rotate_point`, `beams_opted_in`, `check_beams_option`,
`boundary`, `FullRotor` and its `expand`, `phase`, `expand_field`, `expand_load`, `triplets`, `full_rotor`), `model.rs`
(`CyclicOptions`, `add_constraint`'s check of D1a of ADR 0595, `material_of`: materials are assigned by element set, not by
block), `mesh.rs` (`BlockProperty::Beam { section, orientation }`, `BlockProperty::beam`, `add_block`, `set_block_property`,
`block_of`: elements are numbered block by block); the tests `sa_cyclic_beams_f672.rs` (the whole file: W, `ring`, `field_error`,
the NC functions), `sa_rotating_cyclic_shells.rs` (`ring_of`, the XL/XF checks of `expand_load` and `expand_field`, NC1 and NC2),
`sa_ortho_cyclic.rs` (the use of `expand_load` and `expand_field`), `sa_tie.rs` N6.

### Audit of the existing tests that assert the `full_rotor` Beam2 refusal

| Test | What it asserts | Options |
|---|---|---|
| `sa_cyclic_beams_f672.rs::nc1_to_nc4_refusals_amended`, NC4 (and the ignored run-1 record) | `full_rotor` of W refused `UnsupportedKind { kind: "Beam2" }` | `shells(true).beams(true)` |
| `sa_rotating_cyclic_shells.rs` NC1 | `full_rotor` of a Beam2 model refused `UnsupportedKind { kind: "Beam2" }` | `shells(true)` |
| `sa_rotating_cyclic_shells.rs` NC2 | `full_rotor` of a Hex8 sector with a Shell4 element refused `UnsupportedKind { kind: "Shell4" }` | default |
| `sa_tie.rs` N6 | `full_rotor` of an extended boundary refused, key `cyclic` | axis nodes |

The first asserts the refusal **with** `CyclicOptions::beams`. Lifting the refusal for every constraint with the beams option
would break it, and existing tests are not edited. So the capability is a further **opt-in** that qualifies the beams option
(D1); all four tests keep passing unedited (none sets it). The acceptance says "a sector with CyclicOptions::beams": the sector
here has the beams option and the new qualifier, which is the narrowest reading that keeps the audited test (taken on the
maintainer's standing instruction of 2026-11-11; listed under the remaining questions for a later default lift).

## Design

**D1 (the option).** A new field `CyclicOptions::full_rotor_beams: bool` (default `false`), builder
`CyclicOptions::full_rotor_beams(on)`. It qualifies the beams option: `SolidModel::add_constraint` refuses a cyclic constraint
with `full_rotor_beams` and without `beams` (key `cyclic`, naming ADR 0601), before its set check (as D1a of ADR 0595). It is
read only by `full_rotor`; no row, matrix, load or other path reads it. With it (and `shells`, `beams`, which it implies by the
refusal) `full_rotor` takes Beam2 blocks; without it every refusal of `full_rotor` stays as it is.

**D2 (one block per sector copy).** A block has one orientation (`mesh.rs`), and copy `j` of a Beam2 block needs `R^j v` (ADR
0595 D3: the sector stands for the ring whose sector `j` holds the beams rotated by `R^j`, orientation included, because the
element is objective). So `full_rotor` with the option builds a Beam2 block `b` of the sector as `N` consecutive blocks of the
full mesh, copy `j` (`j = 0 .. N-1`) holding the `n_b` elements of copy `j` with the property `Beam { section, R^j v }`. Copy
`0` takes the sector property cloned (bits; `R^0` is the identity only up to the sign of zero entries, ADR 0598 amendment 1);
copy `j >= 1` takes `BlockProperty::beam(section, apply(R^j, v))` with `R^j = rotation_matrix(axis, 2 pi j / N)`, the matrix
`full_rotor` already uses for the nodes of copy `j`. A Beam2 block without a property is copied without one, `N` times. Solid
and shell blocks stay one block holding their `N` copies.

Consequences: (i) the **element numbering is unchanged** (block by block, sector by sector: the `N` consecutive blocks of a split
block hold exactly the elements `start_b + j n_b + e`), so element sets, surface sets and the materials (assigned by element
set) are those of the present code; (ii) the **block indices** of a mesh with a split block shift: a new accessor
`FullRotor::block(sector, block)` gives the full block of copy `sector` of sector block `block` (`block` itself for every block
of a model without the option, so the documented "blocks keep their index" holds there); (iii) `FullRotor` gains a private
field (the block map); it is not `#[non_exhaustive]` but has private fields already, so no caller constructs it.

**D3 (the expansions).** `expand`, `expand_field` and `expand_load` are not changed: they read only the sector and full
`DofMap`s, `node_of`, `is_high` and the axis, and `triplets` already expands the rotations of a six-unknown node (ADR 0561 D1),
which every Beam2 node is. The derivation of ADR 0595 D2 (the field of harmonic `k` in sector `j` is `R^j (cos(j theta) a -
sin(j theta) b)` for translations and rotation vectors alike) is the formula they implement; the full ring's fields are those
fields because the full mesh with D2 is the ring of ADR 0595 (same nodes up to rounding, same elements, orientations `R^j v`).
F-674 checks that, and the full mesh itself.

**D4 (defaults).** A model whose cyclic constraint does not set `full_rotor_beams` runs the code it ran before: the block loop
of `full_rotor` takes the existing branch (one `add_block` per sector block with its property cloned) for every block, the new
branch only for a Beam2 block with the option. The block map is the identity there. Nodes, connectivity, properties and sets
are built by the same expressions in the same order.

## The models

W and the independent ring are those of ADR 0595 ("The models"), rebuilt in the new test file by the same code (copied, as
tests do not share modules): axis `n = (1, 2, 2)/3`, origin `O = (0.3, -0.2, 0.5)`, `N = 6`, ring and spoke blocks with
oblique orientations `v_r`, `v_s`, hub clamped; the ring built from `P` directly with orientations `R^j v`, `R^j =
rotation_matrix(n, 2 pi j / N)`. W's constraint: `CyclicOptions::new().shells(true).beams(true).full_rotor_beams(true)`.

**The full-rotor model F.** `full_rotor(W)`, its mesh with a node set `r<p>` on every node (as `ring_of` of
`sa_rotating_cyclic_shells.rs`), material on `all`, `hub` (the union of the copies of W's hub) clamped in six components.

**Node map.** Full node `rotor.node(j, q)` is ring node `ring_node(j, q)` of ADR 0595 (`4 j + q mod 24` for `q <= 4`, `24 + j`,
`30 + j`). A vector over F's unknowns is mapped to the ring's by this map (both have six unknowns per node). Set-up check G1
verifies the map is a bijection onto the ring's 36 nodes with matching positions.

**Loads.** Static `k = 0` (S0 of ADR 0595): nodal forces and moments `nodal_load(q)` on `q in {0, 1, 2, 3, 5}` and the
centrifugal load `Omega = 100` rad/s. Harmonic loads `k = 0 .. 3` for the load check L: the cosine part `nodal_load(q)` and the
sine part `nodal_load(q + 7)` (zero for `k = 0, 3`) on `q in {0, 1, 2, 3, 4, 5}`: the high node 4 is loaded here too, so that
the summation of the two sectors' contributions at a shared node is exercised. Harmonic static `k = 1, 2, 3` for the field
check F: the loads of S1 .. S3 of ADR 0595 (node 4 not loaded).

**Modal options**: as ADR 0595: `tolerance = 1e-12`, `ResidualTest::Backward`; sector `m = 4` per harmonic, ring and F `m = 30`.

## Arithmetic (by hand, disclosed)

1. **Positions.** F's nodes are `O + R^j (x_q - O)` with `|x| <~ 1.6`, the ring's are `P(...)` computed directly: both within a
   few `eps |x|` of the exact point, `|d| <~ 6 x 2.2e-16 x 1.6 = 2e-15`. Limit G1 `1e-14` (absolute, m).
2. **Orientations.** Copy `j >= 1`: `full_rotor` computes `apply(rotation_matrix(n, 2 pi j / N), v)`, the test computes the same
   expression (`rot(j)` and `apply` of ADR 0595's test are the same operations in the same order): expected difference 0. Limit O1
   `|d| <= 1e-15` per component (a margin for an evaluation-order difference, `~ 4 eps |v|`), copy 0 bits equal (intended: D2
   clones it).
3. **Loads.** `expand_load` sums `R^j (c f_a - s f_b)` with `phase` exact at `j k = 0` and `2 j k = N` (`c = -1`, `s = 0`); the
   ring's hand load (in the test, in ring numbering, the same formula with `t.cos()`, `t.sin()`: at `2 j k = N` `sin(pi) =
   1.2e-16`, a difference `<~ 1.2e-16 |f_b|`, and `f_b = 0` there; and the order of the two sums at a shared node may differ,
   `eps |f|`). Expected `<~ 1e-15` relative. Limit L0 .. L3 `1e-10` (the acceptance).
4. **Static fields.** `expand_field` of the sector solution against `solve_static` of the ring: the quantity of S0 .. S3 of ADR
   0595 (measured `6.6e-13`, worst-case estimate `1e-11` there) computed by the library's expansion. Limit F0 .. F3 `1e-10`.
5. **F against the ring (statics).** Two full models whose element matrices differ by the node rounding of arithmetic 1: as
   arithmetic 6 of ADR 0595, `~1e-13`, worst `1e-11`. Limit Z0 (S0 loads on F, from `expand_load` of the `k = 0` part plus the
   centrifugal load; against the ring's S0) `1e-10`.
6. **F against the ring (modes).** As arithmetic 4 of ADR 0595 (worst `3e-11`, `~5e-12` with the measured `lambda_1`; measured
   sector against ring `8.2e-13`). Limit ZM: the 18 lowest eigenvalues of F and of the ring (ADR 0595 counted 18 below its
   cut-off), `max |lambda_F / lambda_r - 1| <= 1e-10`.
7. **Expanded modes.** P (backward error of the expanded mode in the ring at the free unknowns, as P0 .. P3 of ADR 0595: `<= 6
   eta`, measured `1.5e-13`): limit `1e-10`. RQ (the Rayleigh quotient `phi^T K phi / phi^T M phi` of the expanded mode in the
   ring against the nearest ring eigenvalue): in exact arithmetic the sector's Rayleigh quotient (the expansion maps the reduced
   forms to the full ones, ADR 0386), hence the M check of ADR 0595 (`8.2e-13`); limit `1e-10`. G (the M-Gram matrix of all the
   expanded modes below the cut-off, every harmonic together, in the ring's mass matrix, against the identity: the scale
   `sqrt(2/N)` or `sqrt(1/N)` and the discrete orthogonality `sum_j cos(j theta) sin(j theta) = 0`, `sum_j cos(2 j theta) = 0`
   for `2 k != 0 mod N`, give an orthonormal set from the sector's M-orthonormal modes; rounding of a sum of `~ 200` products,
   `~1e-14`): limit `max |G - I| <= 1e-10`.
8. **Controls.** NC-O on F (F's beam copies reset to the unrotated `v`): the structure of ADR 0595 NC-O (measured `0.778`): `e >=
   1e-3`. NC-H (a `k = 1` mode expanded with the harmonic argument 2): the field is kinked at every sector boundary (the
   boundary node takes sector `j`'s low value, which is not the `k = 2` image of sector `j - 1`'s high value), so its Rayleigh
   quotient is not an eigenvalue: `|RQ / lambda - 1| >= 1e-3`.

## Checks (limits fixed now)

Measures: `e` is `max(e_u, e_theta)` of ADR 0595 (translations and rotations separately, max-norm relative to the ring field),
computed between the mapped vector and the ring vector. Cut-off `lambda_c` as ADR 0595 (`(1 - 1e-6) min_k values_k[3]`).

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| G1 | c1 (set-up) | `full_rotor(W)` succeeds; the node map is a bijection onto the ring's 36 nodes; `max |x_F - x_ring|` | bijection; `<= 1e-14` m | arithmetic 1: `2e-15` |
| O1 | c1 | F has `2 N` blocks; `rotor.block(j, b) = N b + j` (both W blocks are Beam2), of kind Beam2, holding copy `j` of block `b` (connectivity mapped by `node_of`, element `start_b + j n_b + e` in it); its section bits equal W's; its orientation against `apply(rot(j), v_b)`: copy 0 bits equal, `j >= 1` `max |d| <= 1e-15` | as stated | arithmetic 2: 0 |
| L0 .. L3 | c4 | `expand_load` of the harmonic loads (node 4 loaded), mapped, against the ring's load built by hand in ring numbering: `max |d| / max |f|` | `<= 1e-10` each | arithmetic 3: `1e-15` |
| F0 .. F3 | c3 | `expand_field` of the sector solution (`k = 0`: `solve_static_mpc` with the S0 loads; `k = 1 .. 3`: `solve_static_harmonic` with the S-k loads), mapped, against `solve_static` of the ring under the ADR 0595 loads: `e` | `<= 1e-10` each | arithmetic 4 |
| Z0 | c1 | `solve_static` of F under `expand_load` of the S0 nodal loads (as nodal forces and moments on `r<p>`) and the centrifugal load, mapped, against the ring's S0: `e` | `<= 1e-10` | arithmetic 5 |
| ZM | c1 | the 18 lowest eigenvalues of F and of the ring | `<= 1e-10` | arithmetic 6 |
| P0 .. P3 | c2 | `expand` of each sector mode below `lambda_c`, mapped: backward error in the ring | `<= 1e-10` each | arithmetic 7 |
| RQ0 .. RQ3 | c2 | its Rayleigh quotient in the ring against the nearest ring eigenvalue | `<= 1e-10` each | arithmetic 7 |
| G | c2 | `max |G - I|` over all the expanded modes below `lambda_c` | `<= 1e-10` | arithmetic 7 |
| B1 | c5 | the existing tests that cover the changed lines (release): `sa_cyclic_beams_f672`, `sa_rotating_cyclic_beams_f675`, `sa_rotating_cyclic_shells`, `sa_rotating_cyclic_shells_default_bits`, `sa_cyclic_shells`, `sa_ortho_cyclic`, `sa_ortho_cyclic_default_bits`, `sa_tie`, `sa_tie_default_bits`, `si_cyclic`, `si_cyclic_harmonic`, `si_rotating_cyclic`, `sa_gyro`, the lib unit tests | pass | D4 |
| B2 | c5 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | D4 |
| B3 | c5 | the ADR 0595 shell frustum sector (Shell4) under `shells(true)` and under `shells(true).beams(true).full_rotor_beams(true)`: `full_rotor` equal (`==`), every node coordinate bit-equal, and `expand_load`, `expand_field` of a `k = 1` field bit-equal | equal | D4 |

## Negative controls

| Id | Control | Expected |
|---|---|---|
| NC1 | W with `shells(true).beams(true)` (no `full_rotor_beams`): `full_rotor` | refused `UnsupportedKind { kind: "Beam2" }` (unchanged) |
| NC2 | `add_constraint` of a cyclic constraint with `shells(true).full_rotor_beams(true)` (no `beams`) on W's mesh | refused, key `cyclic`, naming ADR 0601 |
| NC-O | F with every Beam2 block reset to the sector's `v`, the Z0 solve | `e >= 1e-3` |
| NC-H | a `k = 1` sector mode expanded with harmonic argument 2, its Rayleigh quotient against its eigenvalue | `>= 1e-3` |

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| X1 | the copies of a Beam2 block keep `v` (no rotation) | O1, Z0, ZM |
| X2 | copy `j` takes `R^{-j} v` (angle `-2 pi j / N`) | O1, Z0, ZM |
| X3 | copy `j` takes `R^{j+1} v` (off by one) | O1, Z0, ZM |
| X4 | `full_rotor` ignores the option (Beam2 refused as before) | every check of F (refused) |
| X5 | the refusal of `full_rotor_beams` without `beams` in `add_constraint` removed | NC2 |
| X6 | `triplets` expands the translations only (`&[0]` for every node) | L, F, P, G |

X6 changes a line that also serves shells; it is run to show that the beam checks see the rotation expansion.

## Decision rule

**Done** if G1, O1, L0 .. L3, F0 .. F3, Z0, ZM, P0 .. P3, RQ0 .. RQ3, G, B1, B2, B3 and NC1, NC2, NC-O, NC-H all hold in the
registered run and every mutation is caught by at least one check. **Partial** if a clause c1 .. c5 is met only in part (a miss
recorded as a `*_known_miss`, a check not run, a mutation not caught). **Todo** if `full_rotor` with Beam2 cannot be built
without changing a default result.

## Limits (not covered)

- The opt-in qualifier: `full_rotor` of a sector with `beams` but without `full_rotor_beams` stays refused (the audited test).
- The rotating paths on F (`RotatingPart` of the full rotor of a beam sector): not run here (F-675 checked `RotatingSector`
  against an independently built ring; F is that ring up to rounding by ZM, but its rotating matrices are not compared).
- Beam ends on interpolated boundaries or on the axis (refused by `full_rotor` for any extended boundary, ADR 0456 D4).
- The block indices of a full mesh with a split block differ from the sector's (D2 ii); a caller that addresses blocks by the
  sector index must use `FullRotor::block`.
- The case schema: no case key for the option.

## Addendum 1: results (2026-11-15)

The registered run (`cargo test -p rcfd-fem --release --test sa_cyclic_beams_full_rotor_f674`) ran once, after the library
commit 0b0952af and the test commit 5472a1ad. Every test passed. Logs `run1.log`, `b1.log`, `b2.log`, `mutations.log` are in
`w49i-work/f674-scratch`.

| Id | Limit | Measured | Met |
|---|---|---|---|
| G1 | bijection; `<= 1e-14` m | bijection onto 36 nodes; `6.7e-16` | yes |
| O1 | 12 blocks, structure; copy 0 bits; `<= 1e-15` | 12 blocks, `block(j, b) = 6 b + j`, connectivity and element numbering as stated; copy 0 bits equal; `0` | yes |
| L0 .. L3 | `<= 1e-10` each | `0`, `1.4e-16`, `0`, `0` | yes |
| F0 .. F3 | `<= 1e-10` each | `6.57e-13`, `6.08e-13`, `3.92e-13`, `3.96e-13` | yes |
| Z0 | `<= 1e-10` | `4.25e-13` | yes |
| ZM | `<= 1e-10` | `2.49e-13` (18 eigenvalues) | yes |
| P0 .. P3 | `<= 1e-10` each | `6.1e-16` (3 modes), `1.5e-13` (6), `9.6e-17` (6), `6.1e-17` (3) | yes |
| RQ0 .. RQ3 | `<= 1e-10` each | `8.1e-14`, `4.3e-14`, `5.6e-14`, `3.4e-14` | yes |
| G | `<= 1e-10` | `1.1e-15` (18 modes) | yes |
| B1 | pass | the registered list and the lib unit tests all pass (ignored records stay ignored) | yes |
| B2 | pass | `baseline_hashes`: 3 passed, 1 ignored (as before) | yes |
| B3 | equal | the `FullRotor`s are equal and 720 values (node coordinates, `expand_load`, `expand_field`) are bit-equal | yes |
| NC1 | `UnsupportedKind { kind: "Beam2" }` | as stated | yes |
| NC2 | refused, key `cyclic`, ADR 0601 | as stated | yes |
| NC-O | `e >= 1e-3` | `0.614` | yes |
| NC-H | `>= 1e-3` | `515` | yes |

F0 .. F3 reproduce the S0 .. S3 numbers of ADR 0595 to the printed digits. That is expected: `expand_field` is the same formula
as that test's hand expansion. The Z0 and ZM numbers show that the full mesh built by `full_rotor` is the independently built
ring up to rounding.

**Mutations** (on a scratch copy with its own target directory; each one caught):

| Id | Caught by |
|---|---|
| X1 (copies keep `v`) | O1 (`1.07`), Z0 (`0.61`), ZM (`0.27`) |
| X2 (`R^{-j} v`) | O1, Z0 (`0.57`), ZM (`0.37`) |
| X3 (`R^{j+1} v`) | O1, Z0 (`8.7e-2`), ZM (`8.7e-2`) |
| X4 (the option ignored) | G1, L, F, Z0, ZM, P (all refused) |
| X5 (no refusal in `add_constraint`) | NC2 |
| X6 (translations only in `triplets`) | L (`0.13`), F (`e_theta 1.0`), P, RQ, G, Z0 |

**Deviations.** None from the registration. One commit only reformats the new refusal message (72de6092, text only).
`cargo fmt -p rcfd-fem --check` and `cargo clippy -p rcfd-fem --all-targets -- -D warnings` are clean. Not run: the whole
workspace suite (the lead's gate).

**Changed library lines.** `crates/rcfd-fem/src/model.rs`: the `full_rotor_beams` field and its doc, the builder, and the
refusal in `add_constraint`. `crates/rcfd-fem/src/cyclic.rs`: the module doc, the `FullRotor` field `block_of` and
`FullRotor::block`, `full_rotor_beams_opted_in`, and the block loop of `full_rotor` (the old branch is unchanged apart from the
`debug_assert`).

**Decision by the registered rule: done.** Every acceptance check, set-up check, control and mutation holds. No limit was
missed and nothing was amended.

### Remaining questions

1. **The qualifier.** The acceptance names `CyclicOptions::beams`. The capability needs `beams` plus the new
   `full_rotor_beams`, because the F-672 test NC4 asserts that `full_rotor` refuses Beam2 under `beams` and existing tests are
   not edited. Options: (a) keep the qualifier; (b) let `beams` alone take `full_rotor`, which retires NC4 of ADR 0595 (a
   maintainer decision on an existing test). Recommendation: (a) for now. Take (b) only if the maintainer agrees to retire that
   assertion; it changes no default result.
2. **Block indices.** In a full mesh with a split block, the block indices are no longer the sector's. `FullRotor::block` gives
   them. Options: (a) keep the split; (b) give each element its own orientation in `BlockProperty` (a change to `mesh.rs` that
   every path reads). Recommendation: (a).
3. **Rotating paths on F.** `RotatingPart` on the full rotor of a beam sector was not run. F-675 compared `RotatingSector` with
   an independently built ring, and ZM shows that F is that ring. Recommendation: no separate item. A one-line smoke check can be
   added if the lead wants it.

## For the lead: roadmap entries

> Proposed status: done.

> Structural solver, advanced tier. Follow-up of F-672 (ADR 0595). DONE. ADR 0601, 2026-11-15: cyclic::full_rotor takes Beam2 blocks when CyclicOptions::beams is qualified by the new opt-in CyclicOptions::full_rotor_beams (refused without beams by add_constraint). Without the qualifier the refusal stays, so the existing test that asserts it under beams passes unchanged. Each Beam2 block of the sector becomes N consecutive blocks of the full mesh, and copy j carries the orientation R^j v (copy 0 the sector's property cloned). The element numbering, the sets and the materials are unchanged, and FullRotor::block gives the block of each copy. FullRotor::expand, expand_field and expand_load needed no change (the rotations of six-unknown nodes were already expanded by R^j). On the ADR 0595 wheel sector against the independently built ring: positions within 6.7e-16, orientations exact; expand_load of k = 0..3 (the high node loaded) within 1.4e-16; expand_field of the static solutions k = 0..3 within 6.6e-13; the full-rotor model's static field within 4.3e-13 and its 18 lowest eigenvalues within 2.5e-13; the expanded modes are eigenvectors of the ring to a backward error of 1.5e-13, their Rayleigh quotients within 8.1e-14, and they are M-orthonormal within 1.1e-15 (limit 1e-10). Unrotated copies are seen at 0.61, and a mode expanded at the wrong harmonic at 515. Models without the option keep their bits (existing tests, baseline hashes, a shell sector's full rotor and expansions bit-equal with and without it). Six of six mutations caught. Not covered: full_rotor with beams but without the qualifier, RotatingPart on the full rotor of a beam sector, extended boundaries.
