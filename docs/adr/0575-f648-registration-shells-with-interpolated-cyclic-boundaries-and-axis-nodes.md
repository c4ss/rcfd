# ADR 0575: F-648 registration: shells with interpolated cyclic boundaries and nodes on the axis (the rows of ADR 0456 D4 on the rotation unknowns)

Status: **REGISTERED 2026-11-15, written before any code of F-648 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-648 was written, compiled or run: the files under "What was read" were
read, and the arithmetic below was done by hand (no script, no rcfd run; disclosed). Results go in the addenda at the end;
nothing above the addenda is changed after a run.

Roadmap item F-648 (dep F-628), the follow-up proposed by [ADR 0556](0556-f628-registration-shells-in-the-cyclic-constraint.md)
(its Limits: interpolated or axis-node boundaries with shells refused, NC5). Number 0575 given by the lead. Branch `f648-work`
from 61033836, worktree `w49i-work/f648`, scratch `w49i-work/f648-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no edit of existing tests; no new dependency, no
unsafe code; clean-room. `docs/ROADMAP.md` and `docs/roadmap.json` are not edited (the entries go in "For the lead"). Every choice
below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-648 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. CyclicOptions::shells together with interpolation or axis_nodes gives the rotation unknowns the rows of ADR 0456 D4 (the interpolated rows and the harmonic rule on the axis); a shell sector with non-matching boundaries against the matched sector: frequencies within 1e-6 at the registered mesh, and an axis node of a closed shell cap against the full model within 1e-8; models without shells keep their bits.

Clauses: **a0** every limit registered here; **a1** with `CyclicOptions::shells` and `interpolation` the rotation unknowns of an
interpolated high node get the interpolated rows of ADR 0456 D4, `theta_H - R (c sum w theta_L - s sum w phi_L) = 0` and the sine
pair; **a2** with `shells` and `axis_nodes` the rotation unknowns of a node on the axis get the harmonic rule of ADR 0456
arithmetic 5 (the null space of their own rows); **a3** a shell sector with non-matching boundaries against the matched sector:
frequencies within 1e-6 at the registered mesh; **a4** an axis node of a closed shell cap: the sector against the full model,
frequencies within 1e-8; **a5** models without shells keep their bits.

## What was read (no run)

`AGENT_RULES.md`; the brief; the F-648 and F-628 entries of `docs/roadmap.json`; ADR 0556 (D1 to D4, its checks and NC5, the
addendum); ADR 0456 (arithmetic 4 and 5, D1, D2, D4, checks C1 to C4 and their measured values: the 2:3 solid ring 3.88e-3
against the matching sector); ADR 0561 (its Limits: interpolated boundaries and axis nodes with shells left to F-648; `full_rotor`
refuses an extended boundary); the sources at 61033836: `cyclic.rs` (`boundary`, `CyclicBoundary`, `shells_opted_in`,
`check_cyclic_model`, `modal_analysis_harmonic`, `full_rotor` and its refusal of `is_extended`), `mpc.rs` (`cyclic_rows`: the
matched rows with the ADR 0556 rotation rows, the interpolated rows and the axis rows on translations only;
`Transformation::independent`, `harmonic_transformation`, the elimination `process`: a row that reduces to `0 = 0` is dropped),
`model.rs` (`CyclicOptions`, the node-set validation of `add_constraint` under `shells`), `tie.rs` (`weights`, surface based;
`TieOptions`, `TieMethod::{NodeToSurface, Mortar}`, `GAP_FACTOR = 0.05`), `error.rs` (`FemError::TieGap`), `shell.rs` and
`shell3.rs` (module docs: bilinear and linear interpolation of `u` and `theta`, MITC4/MITC3 shear, drilling penalty, facet frames
from the node positions only); `tests/sa_cyclic_shells.rs` (its helpers, the E protocol, NC5), `tests/sa_rotating_cyclic_shells.rs`
(no refusal of interpolation or axis nodes asserted), `tests/sa_tie.rs` (the solid C1, C2, C3 set-ups).

### Audit of the existing tests that assert the refusal this item lifts

`tests/sa_cyclic_shells.rs::nc1_to_nc6_refusals`, NC5: the D3 Shell4 sector with `shells(true).axis_nodes(true)` and with
`shells(true).interpolation(NodeToSurface)` on `solve_static_mpc`, each asserted refused with key `cyclic` and a message containing
"ADR 0556". No other test asserts it (grep for `axis_nodes(true)`, `interpolation(` and "0556" in `crates/*/tests`). Those two
assertions must keep passing and may not be edited, so the capability is opt-in by a second flag (D1): with `shells` and without
the new flag, `interpolation` or `axis_nodes` is refused as before, with the same message. NC1 below repeats that assertion.

## Design

### D1. The opt-in route: `CyclicOptions::shell_extensions`

A new field `shell_extensions: bool` of `CyclicOptions` (default `false`; builder `CyclicOptions::shell_extensions(on)`). In
`cyclic::boundary`:

1. `shells` with `interpolation` or `axis_nodes` and without `shell_extensions`: refused exactly as now (the ADR 0556 message).
2. `shell_extensions` without `shells`: refused (key `cyclic`, naming ADR 0575: the flag only qualifies the shells option).
3. `shells` and `shell_extensions`: the boundaries are node sets (ADR 0556 D1); `axis_nodes` takes the nodes on both sets as in
   ADR 0456 D4 (on the axis within `MATCH_TOLERANCE L`, else refused naming the node); `interpolation` ties the high nodes (other
   than axis nodes) to the **low shell edges** rotated by `R` (D3), because a shell has no faces and `tie::weights` works on
   faces. `TieMethod::Mortar` is refused with shells (key `cyclic`, naming ADR 0575; a mortar on edges is not derived here).

In `mpc::cyclic_rows`, under `shells` (the existing guard of the ADR 0556 rotation rows):

4. each interpolated high node whose unknown count is 6, with every low node of its weights of count 6, gets after its three
   translation rows the three rotation rows, `theta_H - R (c sum w theta_L - s sum w phi_L) = 0` and, with a sine part,
   `phi_H - R (s sum w theta_L + c sum w phi_L) = 0` (the translation rows of ADR 0456 D4 with components 3 to 5); a high node
   whose count differs from that of a low node of its weights is refused (key `cyclic`, naming both nodes and both counts);
5. each axis node with 6 unknowns gets, after the rows of its translations, the rows of its rotations from the same null space
   (D2): the constraint matrix `C = I - P` is the same matrix for the rotation triple, so its null-space basis in solved form is
   computed once and emitted twice, once on components 0 to 2 and once on 3 to 5.

With `shells = false` no executed line changes for any model (the new checks read a flag that is false; the interpolated and axis
loops keep their arithmetic and order on components 0 to 2), so a5 holds by construction; it is checked by B1 to B3. `full_rotor`
still refuses an extended boundary (ADR 0456 D4), with or without shells.

### D2. Why the same rows (derivation)

ADR 0556 D2: under the proper rotation `x -> o + R (x - o)` the small-rotation vector of a shell node transforms as the
translation, `theta' = R theta`, and the element matrices are objective. So every relation ADR 0456 derives for the translation
triple from the block-circulant structure holds for the rotation triple with the same `R`:

- **Interpolated rows.** ADR 0456 D1/D4: a high node `H` at `x_H` follows the field of the rotated low boundary at its
  projection. On a shell the low boundary is a chain of element edges; the restriction of the Shell4 (bilinear) and Shell3
  (linear) interpolation of `u` and of `theta` to an edge `(A, B)` is linear in the edge parameter, `u(t) = (1 - t) u_A + t u_B`.
  So the node-to-edge interpolation with weights `(1 - t, t)` is the node-to-surface rule of ADR 0456 D1 restricted to the edge,
  for the six unknowns alike; carried to the high position by the cyclic phase and `R`, it gives the rows of D1.4.
- **Axis rows.** ADR 0456 arithmetic 5: a node on the axis is its own image, so its harmonic unknowns `U` (complex) satisfy `U =
  e^{i theta_k} R U`; a vector that transforms with `R` obeys the same rule, so the rotation triple has the same null space: at
  `k = 0` the rotation about the axis is free and the two others held; at `k = 1` (`N >= 3`) two of the four in-plane rotation
  unknowns (`b = J a`, `J = n x`) are free and the axial ones held; at every other `k` (here `N = 6`: `k = 2, 3`) all held. For
  the translations the same (ADR 0456: at `k = 0` the axis node moves along the axis only). Check (by hand): `k = 1`, `b = J a`,
  `c = cos alpha`, `s = sin alpha`, in-plane `R = c I + s J`: `R (c a - s J a) = (c I + s J)(c I - s J) a = a`, and `R (s a + c J
  a) = (c I + s J)(s I + c J) a = (cs - sc) a + (c^2 + s^2) J a = J a = b`; right.

The axis node of a full shell model is a node like any other: the harmonic decomposition of the full model's unknowns at that
node is exactly the rule above, so the sector with the axis rows is the same discrete problem as the full model, harmonic by
harmonic (as ADR 0556 for the matched pairs). Hence the 1e-8 of a4 is a rounding limit.

### D3. Node-to-edge weights (`cyclic.rs`, new private function)

Master segments: every edge of every Shell4 (`(n0,n1), (n1,n2), (n2,n3), (n3,n0)`) and Shell3 (`(n0,n1), (n1,n2), (n2,n0)`)
element whose two nodes are both in the low set, stored as ascending pairs, deduplicated, in ascending order; none: refused (key
`cyclic`, "the low node set has no shell edge"). Each endpoint mapped by `x -> o + R (x - o)`. For each high node `p` (ascending,
axis nodes excluded) and each mapped segment `(A, B)`: `t* = (p - A).(B - A) / |B - A|^2`, inside when `0 <= t* <= 1`, `t` the
clamp of `t*` to `[0, 1]`, distance `|p - A - t (B - A)|`; the best segment is chosen as in `tie::weights` (an inside one before
an outside one, then the smaller distance, the first on ties). Tolerance: `TieOptions::tolerance` or `GAP_FACTOR` times the mean
mapped segment length; above it `FemError::TieGap { node: p, distance, tolerance }`. Weights `(A, 1 - t), (B, t)` with a zero
weight dropped. A high node that is also a low node is an axis node (excluded) or refused before (ADR 0456 D4).

### D4. Test geometries (fixed now)

Common: steel `E = 2e11` Pa, `nu = 0.3`, `rho = 7850` kg/m^3, `t = 0.05` m, default element options, `N = 6`, axis `n = (1, 2,
2)/3` through `o = (0.3, -0.2, 0.5)`, `e_a = (2, -1, 0)/sqrt(5)`, `e_b = n x e_a` (as ADR 0556 D3, so `R` is a full matrix).
Modal options: `ResidualTest::Backward`, tolerance `1e-12` (as ADR 0556).

**The frustum (interpolation checks).** ADR 0556 D3: `x(phi, s) = o + r(s) (cos phi e_a + sin phi e_b) + z(s) n`, `r = 1 - 0.4
s`, `z = 0.8 s`; the sector `phi in [0, pi/3]` with node columns `i = 0 .. n_c` at `phi = (pi/3) i / n_c`, column `i` with `m_i`
segments along the generator (node `(i, a)` at `s = a / m_i`), clamped (six components) at `s = 0`, free at `s = 1`; node set
`low` = column 0, `high` = column `n_c`. Between two columns with equal `m` the quads `(i, a), (i+1, a), (i+1, a+1), (i, a+1)`
(isosceles trapezoids, planar; Shell3: cut along the diagonal from `(i, a)` as ADR 0556); between columns of `m` and `m'`
segments (Shell3 only) the strip triangulated by merging the two sorted `s` sequences: from `(a, b) = (0, 0)` advance the side
whose next node has the smaller `s` (the left column `i` on equal `s`), triangle `(L_a, R_b, R_{b+1})` when the right advances,
`(L_a, R_b, L_{a+1})` when the left advances.

- **Mesh I1 (the registered mesh of a3).** Shell4, `n_c = 4`, `m = 6` in every column. The non-matching sector: the interior
  high nodes `(4, a)`, `a = 1 .. 5`, at `s = (a + epsilon) / 6` with `epsilon = 1e-6` (moved along the generator by `epsilon`
  times the segment length 0.149 m, `1.49e-7` m; on the cone, so on the rotated low edge to rounding), the end nodes `a = 0, 6`
  in place; interpolation `NodeToSurface` (the node-to-edge rule of D3). The matched sector: the same mesh with `epsilon = 0` on
  the matched path. The shift is 15 times the match tolerance (`1e-8 L`, `L = 1` m), so the matched path refuses the shifted
  mesh (NC7): it is a non-matching boundary. Disclosed reason for choosing a small shift: two different meshes of a boundary
  differ in their frequencies by their discretisation difference, `O(h^2)`, of order 1e-3 at any mesh that runs here (ADR 0456
  C1: 3.9e-3 for a 2:3 solid ring), far above 1e-6; at a non-matching boundary that is a small move of a matched one, the
  frequencies move by `O(epsilon)` times a coefficient of the size of the discretisation error. So 1e-6 is checked at this
  registered mesh, and the genuinely non-matching 2:3 boundary is checked by I2 and I3 at limits of its own.
- **Meshes I2a, I2b (2:3).** Shell3, `n_c = 4`. Matched reference: `m = 8` in every column, matched path. I2a (the high side
  finer): `m_4 = 12`, the other columns 8 (strip between columns 3 and 4 merged); I2b (the low side finer): `m_0 = 12`, the
  others 8 (strip between columns 0 and 1 merged). The high nodes of I2b at `s = b/8` fall on the rotated low edge at `s =
  1.5/12` and `4.5/12`, ... (mid-segment) or on its nodes; those of I2a at `s = a/12` fall on the 8-segment edge at thirds.
  I2a is conforming (the fine high edge follows the coarse linear low edge exactly), I2b is the one-sided tie of ADR 0456 (the
  intermediate low nodes are not followed by the coarse high edge of the neighbour sector).
- **Level 2 (I3).** The same with every count doubled: `n_c = 8`, matched `m = 16`, the fine column `m = 24`.

**The cap (axis checks).** A spherical cap of radius `R_s = 1` m about the axis, polar angle `psi in [0, pi/3]`, `x(phi, psi) = o +
R_s sin psi (cos phi e_a + sin phi e_b) + R_s cos psi n`; pole `x(., 0) = o + n` on the axis; rings `a = 1 .. 3` at `psi = (pi/3)
a / 3`, `n_c = 2` columns per sector (`phi = (pi/3) i / 2`); the rim ring `a = 3` clamped (six components). Elements: around the
pole Shell3 `(pole, (i, 1), (i+1, 1))`; between rings, A1: Shell4 quads (isosceles trapezoids on two parallel circles, planar),
A2: the same quads cut into Shell3 as above. The sector: the pole and columns `0 .. 2`, node sets `low` = pole and column 0,
`high` = pole and column 2, `CyclicOptions::new().shells(true).shell_extensions(true).axis_nodes(true)`. The full model: the pole
and 12 columns closing on column 0, the same rim clamp, no cyclic constraint.

Arithmetic (by hand). I1: 35 nodes, 210 unknowns, 30 clamped; the 7 high nodes give 42 rows, the 6 of the clamped high node `(4,
0)` (weight 1 on the clamped low node `(0, 0)`, both sides prescribed) reduce to `0 = 0` and are dropped: `210 - 30 - 36 = 144`
independent at `k = 0, 3`, 288 at `k = 1, 2`. I2: matched 45 nodes, I2a and I2b 49 nodes. Level 2: matched 153 nodes, I2a/I2b
161. The cap: sector 10 nodes (60 unknowns, 18 clamped, the high column 3 nodes with the clamped one redundant: 12 rows, the pole
6 unknowns of which free 2 at `k = 0`, 4 of 12 at `k = 1`, none at `k = 2, 3`); full model 37 nodes, 222 unknowns, 72 clamped,
150 free. Each harmonic real problem is at most a few hundred unknowns; every modal run is a fraction of a second in release.

## Checks (limits fixed now)

Measures. **Sector against sector** (I1, I2, I3): for each `k = 0 .. 3`, `modal_analysis_harmonic(k, 4)` of both sectors (the
four lowest eigenvalues of the real problem; a pair at `k = 1, 2` counts twice), sorted, `d_k = max_i |f_i / f_ref,i - 1|`; the
measure is `max_k d_k` (each `d_k` reported). **Sector against full model** (A1, A2): the protocol of ADR 0556 E1: cut-off
`lambda_c = (1 - 1e-6) min_k lambda_{k,4}`, the sector list (every eigenvalue below `lambda_c` of every harmonic, a pair twice),
the full list (`modal_analysis(24)` below `lambda_c`), counts equal and the largest full eigenvalue `>= lambda_c` (set-up), sorted
pairing, `max |f_s / f_r - 1|` per harmonic, and the pair splits of `k = 1, 2`.

| Id | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| I1 | a1, a3 | mesh I1 (Shell4, interpolated, `epsilon = 1e-6`) against the matched sector, `max_k d_k` | `<= 1e-6` | `epsilon` times the sensitivity of the frequencies to a move of boundary nodes along the boundary, of the size of the discretisation error of the registered mesh (a few 1e-2): `~ 5e-8`; the acceptance limit |
| I2a | a1 | mesh I2a (2:3, high side finer) against the matched `m = 8` sector, `max_k d_k` | `<= 2e-2` | the discretisation difference of one refined strip of four: the error of the coarse sector (a few 1e-2 for Shell3 at this mesh) times the area fraction 1/4 times `1 - (8/12)^2 = 0.56`: `~ 5e-3`; ADR 0456 C1 3.9e-3 on solids |
| I2b | a1 | mesh I2b (2:3, low side finer, one-sided) against the matched sector | `<= 2e-2` | as I2a plus the one-sided gap `O(h^2 u'')` at the interface: `~ 5e-3 .. 1e-2` |
| I3 | a1 | level 2 of I2a and of I2b: `max_k d_k` at level 2 over level 1, each | `<= 0.7` each | `O(h^2)`: 0.25; 0.5 if the one-sided term is `O(h)` in the energy |
| R1 | a1 | I2b `k = 1`, every mode: for every interpolated high node, the rotation rows hold, `max |theta_H - R (c sum w theta_L - s sum w phi_L)|` and the sine pair over `max |theta|` of the mode, with the weights of `cyclic::boundary` | `<= 1e-13` | substitution of the transformation, a few products rounded: `~ 1e-16` |
| A1 | a2, a4 | the cap, Shell3 at the pole and Shell4 elsewhere: counts equal; per harmonic `k = 0 .. 3` `max |f_s/f_r - 1|`; pair splits `k = 1, 2` | equal; `<= 1e-8` each; `<= 1e-8` | the same discrete problem (D2): rounding, `~ 1e-14` (ADR 0556 E1 measured 1.7e-15) |
| A2 | a2, a4 | the cap all Shell3, as A1 | as A1 | as A1 |
| A3 | a2 | the A1 sector: the pole's unknowns in `Transformation::independent()` of the harmonic modes, cosine and sine parts | 2, 4, 0, 0 at `k = 0, 1, 2, 3` | D2 (ADR 0456 arithmetic 5 on both triples) |
| B1 | a5 | every existing rcfd-fem test passes unchanged (release), in particular `sa_cyclic_shells` (NC5), `sa_rotating_cyclic_shells`, `sa_tie`, `sa_tie_default_bits`, `si_cyclic*` | pass | D1 |
| B2 | a5 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | D1 |
| B3 | a5 | the ADR 0556 D3 Shell4 sector (matched) with `shell_extensions(true)` against without: `modal_analysis_harmonic` `k = 0 .. 3` frequencies and the `k = 0` `solve_static_mpc` displacements under the ADR 0556 S1 centrifugal load, bit for bit | equal bits | the flag changes no row of a matched boundary |

## Negative controls

| Id | Control | Expected |
|---|---|---|
| NC1 | the D3 sector of ADR 0556 with `shells(true).axis_nodes(true)` and with `shells(true).interpolation(NodeToSurface)`, no `shell_extensions` | refused, key `cyclic`, "ADR 0556" (the existing NC5, repeated) |
| NC2 | `shell_extensions(true)` without `shells` on a Hex8 sector with surface sets | refused, key `cyclic`, naming ADR 0575 |
| NC3 | mesh I2b with `interpolation(Mortar)` | refused, key `cyclic`, naming ADR 0575 |
| NC4 | mesh I2b with the high node `(4, 3)` moved along the radius by `0.2` times the mean low segment length (`GAP_FACTOR` is 0.05) | `FemError::TieGap` naming that node |
| NC5 | the A1 cap with the pole moved by `1e-4` m perpendicular to the axis | refused, the node on both boundaries off the axis |
| NC6 | mesh I2b with a disjoint Hex8 whose one node is placed at the position of the high node `(4, 3)` and replaces it in the high set | refused, key `cyclic`, naming both counts (3 and 6) |
| NC7 | mesh I1 with `shells(true)` alone (matched path) | refused, a low node without an image ("has no node of the high boundary") |

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| M1 | the rotation rows of interpolated nodes not emitted | I1, I2a, I2b, R1 |
| M2 | those rows with the identity in place of `R` | I1, I2a, I2b, R1 |
| M3 | in those rows the sign of `s` flipped (`theta_H - R (c sum w theta_L + s sum w phi_L)`) | I1 or I2 at `k = 1, 2`, R1 |
| M4 | the axis rows of the rotations not emitted | A1, A2, A3 |
| M5 | the axis rows of the rotations emitted on components 0 to 2 (the translation rows twice) | A1, A2, A3 |
| M6 | the edge weights swapped (`(A, t), (B, 1 - t)`) | I1, I2b |
| M7 | the low edges not rotated (the identity map) | I1, I2a, I2b (refused, `TieGap`) |
| M8 | `shell_extensions` ignored (the ADR 0556 refusal kept) | every positive check (refused) |
| M9 | the unknown-count check of interpolated nodes removed | NC6 |
| M10 | the axis rotation rows emitted only with a sine part (missing at `k = 0` and `k = 3`) | A1, A2, A3 |

No mutation is declared equivalent.

## Decision rule

**done**: every check I1 to B3 met, NC1 to NC7 as stated, every mutation caught. **partial**: any registered check missed
(recorded as a `*_known_miss`), any mutation uncaught, or any check met only after a post-run amendment. **todo**: the positive
checks I1 to A3 not run.

## Limits (what is not covered)

- The mortar method on shell edges (refused, NC3); curved boundary edges (the edges are straight, as the elements' own edges).
- Interpolation and axis nodes together in one shell sector (the code takes them, axis nodes excluded from the slaves as in ADR
  0456; not checked here).
- The rotating paths (ADR 0561) take these rows through `harmonic_transformation` (Campbell, whirl, pre-stressed modes) but are
  not checked here with an interpolated or axis-node shell boundary; `full_rotor` refuses an extended boundary (ADR 0456 D4).
- Static and harmonic static solves with these boundaries go through the same rows; not checked here (the acceptance asks for
  frequencies).
- A shell node shared with a solid and Beam2 stay refused (ADR 0556).

## Addendum 1: results (2026-11-15)

Library at 452cd139 (the run), then 70e0535c and eb90bd4c after the run. Those two commits change the whitespace of one refusal
message and add a type alias for clippy; neither changes a number or a branch. Files: `model.rs` (`CyclicOptions::shell_extensions`
and its builder), `cyclic.rs` (the module docs; in `boundary`, the guard of the ADR 0556 refusal, the refusal of the flag without
`shells`, and the node-to-edge branch with the mortar refusal; the new private `shell_edge_weights`), `mpc.rs` (in `cyclic_rows`,
the rotation rows and the unknown-count refusal of interpolated nodes, and the rotation rows of six-unknown axis nodes). Tests:
`crates/rcfd-fem/tests/sa_cyclic_shell_extensions.rs`. No existing test was edited, and no golden or hash table was touched.

Every registered check was run once (`f648-scratch/run1.log`, 2.3 s in release).

| Id | Limit | Measured | Met |
|---|---|---|---|
| I1 shifted high column (Shell4, `epsilon = 1e-6`) against the matched sector | 1e-6 | 2.89e-8 (k = 0..3: 6.6e-9, 5.1e-10, 1.0e-9, 2.9e-8); 7 interpolated nodes, 0 matched pairs | yes |
| I2a 2:3, high side finer (conforming) | 2e-2 | 7.40e-3 (k = 0..3: 7.4e-3, 6.1e-3, 2.0e-3, 4.5e-3) | yes |
| I2b 2:3, low side finer (one-sided) | 2e-2 | **4.17e-2** (k = 0: 4.2e-2, k = 1: 3.9e-2, k = 2: 5.6e-3, k = 3: 9.9e-3) | **no** |
| I3 level 2 over level 1 | 0.7 each | I2a 9.25e-4 / 7.40e-3 = 0.125; I2b 1.40e-3 / 4.17e-2 = 0.034 | yes |
| R1 interpolated rotation rows in the k = 1 modes | 1e-13 | 1.65e-15 (72 node-modes) | yes |
| A1 cap, Shell4 with Shell3 at the pole, against the full model: counts; k = 0..3; splits | equal; 1e-8; 1e-8 | 11 = 11 (largest full value 3.39e8, above the cut-off 1.26e8); 3.3e-16, 8.9e-16, 6.7e-16, 4.4e-16; 7.2e-16, 7.7e-16 | yes |
| A2 cap, all Shell3, as A1 | as A1 | 13 = 13 (1.75e8 above 1.35e8); 2.9e-15, 8.9e-16, 4.4e-16, 4.4e-16; 2.3e-15, 1.1e-15 | yes |
| A3 independent pole unknowns, k = 0..3 | 2, 4, 0, 0 | 2, 4, 0, 0 | yes |
| B1 every rcfd-fem test (release) | pass | 1156 passed, 0 failed, 310 ignored (at 70e0535c) | yes |
| B2 `baseline_hashes` | pass | 3 passed, 1 ignored | yes |
| B3 matched shell sector with and without `shell_extensions` | equal bits | equal | yes |
| NC1 to NC7 | as stated | each as stated: the ADR 0556 refusal twice; ADR 0575 for the flag alone and for the mortar; `TieGap` of node 43 at 1.33e-2 against 3.73e-3; the pole 1.0e-4 from the axis; "high node 49 has 3 unknowns ... has 6"; on the matched path no image within 1e-8, the nearest at 1.49e-7 | yes |

Also run: `cargo test -p rcfd-cli --release --test docs_site` passed (2 tests), and `cargo fmt -p rcfd-fem --check` is clean. The
first `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` failed on `type_complexity` (the return type of
`shell_edge_weights`). A type alias fixed it (eb90bd4c) and clippy is now clean. After that change the new test file,
`sa_cyclic_shells` and `sa_tie` were run again and pass.

**I2b known miss, diagnosis.** The low side has 12 segments and the high side has 8. In the ring, the coarse high edge of one
sector meets the fine low edge of the next sector, and only the high nodes are tied. The intermediate low nodes that no high-node
image reaches are left free, so the interface has a one-sided gap. The fourth value of k = 0 (1091.48 Hz against 1138.93 Hz) and
the fourth value of k = 1 (1091.48 Hz against 1135.33 Hz) are the same mode. A frequency that does not change with the harmonic
points to a mode local to the interface: the free low nodes form a soft line. At level 2 this local mode rises above the four
lowest values, and the difference falls by a factor of 30 (I3). The rows are not at fault: I2a, the conforming orientation, meets
its limit at 7.4e-3, and R1 shows the rows hold to 1.7e-15. The registration estimated 5e-3 to 1e-2 and did not foresee a local
mode among the four lowest values. The test is kept as the ignored record
`i2_i3_two_to_three_boundaries_against_the_matched_sector_known_miss`, with its limits intact. The checks of that run that were
met (I2a and both I3 ratios) are also asserted, with the same set-up and limits, by the fast test
`i2a_i3_met_parts_of_the_two_to_three_boundaries`.

Mutations were run on a `git archive` copy of 70e0535c with its own target directory (`f648-scratch/mut`, `mutate.py`,
`run_mutations.sh`, logs `mut_M*.log`). Each mutation was applied, its diff logged, and the tests run once with
`--include-ignored`.

| Id | Caught by |
|---|---|
| M1 interpolated rotation rows not emitted | I1, I2a/I3, I2b, R1 (the eigensolver no longer converges because the boundary rotations are free) |
| M2 identity in place of `R` | I1 (3.4e-2), I2a (3.7e-2), I3, I2b, R1 (0.81) |
| M3 sign of `s` flipped | I1 (0.25), I2a (0.23), I3, I2b, R1 (1.29) |
| M4 axis rotation rows not emitted | A1, A2 (counts 16 against 11), A3 |
| M5 axis rotation rows on the translations | A1, A2, A3 |
| M6 edge weights swapped | I1 (0.28), I2a (0.50), I3, I2b |
| M7 low edges not rotated | I1, I2a, I2b, R1, NC4, NC6 (`TieGap`, refused) |
| M8 `shell_extensions` ignored | every positive check (refused), NC3 to NC6 |
| M9 unknown-count check removed | NC6 (`Ok`) |
| M10 axis rotation rows only with a sine part | A1, A2 (k = 0, 3), A3 |

Ten of ten mutations were caught. The I2b record fails under every mutation because it is the known miss. It is never the only
test that catches a mutation.

Deviations: none from the registered set-ups or limits. The two commits made after the run change only a message string and a
type name. Every registered check was run.

**Decision by the registered rule: partial.** A registered check, I2b, missed (4.17e-2 against 2e-2) and is now a known miss.
Every clause of the acceptance was met: a1 and a2 (the rows; R1, A3), a3 (I1, 2.9e-8 against 1e-6 at the registered mesh), a4
(A1 and A2, within 3e-15) and a5 (B1 to B3). NC1 to NC7 behaved as stated, and every mutation was caught.

### Remaining questions

1. On the one-sided 2:3 shell boundary (I2b), the low nodes that are not tied form a soft line. Options: (a) a follow-up that
   ties the shell edges on both sides, so that the low nodes no image reaches also follow the high edge rotated back by `R^T`
   (the chain refusal of the elimination needs to be checked for this), or a node-to-edge mortar; (b) state in the docs that the
   finer side should be the high side (I2a is conforming). Recommendation: (b) now, and (a) as the follow-up proposed below.
2. The rotating paths take the rows with interpolated or axis-node shell boundaries but are not checked with them, and
   `full_rotor` refuses an extended boundary. Recommendation: put a spinning-cap check in the same follow-up.
