# ADR 0595: F-672 registration: Beam2 elements in the cyclic constraint

Status: **REGISTERED 2026-11-15, written before any code of F-672 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-672 was written, compiled or run: the files under "What was read" were
read, and the arithmetic below was done by hand (no script, no rcfd run; disclosed). Results go in the addenda at the end;
nothing above the addenda is changed after a run.

Roadmap item F-672 (dep F-628), the follow-up proposed by [ADR 0593](0593-f666-registration-hyperelastic-structural-elements-on-the-cyclic-rotating-cms-and-mpc-paths.md)
(its remaining question 2) on the refusal of [ADR 0556](0556-f628-registration-shells-in-the-cyclic-constraint.md) D1. Number
0595 given by the lead. Branch `f672-work` from 8da47021, worktree `w49i-work/f672`, scratch `w49i-work/f672-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no edit of existing tests; no new dependency, no
unsafe code; clean-room. `docs/ROADMAP.md` and `docs/roadmap.json` are not edited. Every choice below that the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-672 asks

> Every limit not stated here is registered in the ADR before the run. A cyclic sector holding Beam2 elements (the orientation vector and the rotation unknowns rotated per sector) gives the static displacements and the modes of each harmonic index of the full ring of beams within 1e-10, the defaults bit-identical.

Clauses: **c0** every limit registered here; **c1** a cyclic sector holding Beam2 elements is taken, its orientation vector and
its rotation unknowns rotated per sector; **c2** its static displacements (translations and rotations) are those of the full
ring of the same beams within `1e-10`; **c3** the modes of each harmonic index are those of the full ring within `1e-10`; **c4**
the defaults bit-identical.

## What was read (no run)

`AGENT_RULES.md`; the brief; ADR 0556 (D1 the shells option and the Beam2 refusal, D2 the rotation rows `theta_H = R (c theta_L - s
phi_L)`, `phi_H = R (s theta_L + c phi_L)`, the checks S1, S2, E1 to E3, R1 and NC3); ADR 0575 (D1.4, D1.5: the rotation rows of
interpolated and axis nodes, under `shell_extensions`); ADR 0583 (the two-sided ties, the form of this document); ADR 0456 D4
(the rows of the extensions); ADR 0593 (the A-B row: Beam2 on the cyclic path refused for every material; its question 2);
ADR 0581 and 0584 (the rounding floor `c eps lambda_max / lambda_1` of the default relative residual and the opt-in backward
test); ADR 0417 (the Beam2 frame `e_1` along the axis, `e_3 = e_1 x v / |e_1 x v|`, `e_2 = e_3 x e_1`); ADR 0473 D5 (the
centrifugal load of a beam). Sources at 8da47021: `cyclic.rs` (`shells_opted_in`, `boundary`, `check_cyclic_model` and
`refuse_cyclic_beams`, `modal_analysis_harmonic`, `solve_static_harmonic`, `full_rotor`), `mpc.rs` (`cyclic_rows`: with the
shells option a pair of six-unknown nodes takes the rows of components 3 to 5 with the same `R` as 0 to 2, whatever element the
node belongs to; `check_transformation_model`: Beam2 with a cyclic constraint refused before the shells test), `model.rs`
(`CyclicOptions`, `#[non_exhaustive]`, `Default`; the `DofMap` gives six unknowns to every node of a Beam2 element), `mesh.rs`
(`BlockProperty::Beam { section, orientation }`: one orientation per block), `beam.rs` (frame and module doc), `rotating.rs`
(`RotatingPart::new`, `check_model`); the tests `sa_cyclic_shells.rs` (the whole file: the frustum, `field_error`, the list
protocol of E1), `sa_hyper_structures_f666.rs` (A-B), `si_beam_transformation.rs` (N3), `sa_rotating_cyclic_shells.rs` (NC1).
`CyclicOptions` is not used outside `rcfd-fem` (searched: no case key, schema or golden names it).

### Audit of the existing tests that assert the cyclic Beam2 refusal

| Test | What it asserts | Options of its constraint |
|---|---|---|
| `sa_cyclic_shells.rs::nc1_to_nc6_refusals`, NC3 | `solve_static_mpc` and `modal_analysis_harmonic` of a Beam2 model refused `NotYetSupported { variant: "Cyclic" }` naming `beam::FOLLOW_UP` | `shells(true)` |
| `si_beam_transformation.rs` N3 | `solve_static` and `modal_analysis` of a beam model with a cyclic constraint refused the same way | default |
| `sa_hyper_structures_f666.rs::a_b_beam2_cyclic_refused_for_its_own_reason` | `modal_analysis_harmonic` `k = 0` refused naming `beam::FOLLOW_UP` | `shells(true)` |
| `sa_rotating_cyclic_shells.rs` NC1 | `full_rotor` of a Beam2 model refused `UnsupportedKind { kind: "Beam2" }` | `shells(true)` |

Lifting the refusal for `shells(true)` would break the first and the third. So the capability is an **opt-in** (D1), and all
four keep passing unedited: none of them sets it.

## Design

**D1 (the option).** A new field `CyclicOptions::beams: bool` (default `false`) with the builder `CyclicOptions::beams(on)`. It
qualifies the shells option: `beams(true)` without `shells(true)` is refused (key `cyclic`, naming ADR 0595), and so is
`beams(true)` with `interpolation` or `axis_nodes` (the extensions of ADR 0575 tie high nodes to shell edges; a beam end on an
interpolated or axis boundary is not checked here). With `shells(true).beams(true)` the entry checks of the cyclic paths
(`check_cyclic_model`: `modal_analysis_harmonic`, `solve_static_harmonic`; `check_transformation_model`: `solve_static_mpc`,
`modal_analysis_mpc` with a cyclic constraint) take Beam2 blocks (`check_supported_with_structures` checks them as on every
path that takes beams: a section and orientation on each block, no node shared with a solid). Without the option every refusal
stays as it is (the four tests above). The option changes no row, matrix or load: it only removes the refusal.

**D2 (the rotation unknowns).** The rows are those of ADR 0556 D2, unchanged: `cyclic_rows` already gives a pair of six-unknown
nodes the rows of the rotations with the same `R` as the translations, and the nodes of a Beam2 element have six unknowns.
Derivation (first principles, as ADR 0556 D2): the small rotation of a node is a vector that turns with the body; in sector `j`
of a rotor whose sector `j` is the image of sector 0 under the rigid rotation `R^j`, the field of harmonic index `k` is `x_j =
R^j (cos(j theta) a - sin(j theta) b)` for the translation and for the rotation vector alike (`theta = 2 pi k / N`); the high
boundary of sector 0 is the low boundary of sector 1, which gives the rows for both triples.

**D3 (the orientation vector).** A Beam2 element's section axes are fixed by its axis `e_1` and the block's orientation vector `v`
(ADR 0417: `e_3 = e_1 x v / |e_1 x v|`, `e_2 = e_3 x e_1`). Under a rigid rotation `R` of the nodes, `R e_1` is the new axis;
with `R v` as the orientation vector, `R e_1 x R v = R (e_1 x v)` (a proper rotation preserves the cross product), so the new
frame is `R` times the old one, the local matrices are the same, and the global matrices are `R_b K_e R_b^T` with `R_b` four
copies of `R`: the element is objective. The same holds for its mass and for its centrifugal load about the rotation axis
(ADR 0473 D5: `v` enters through the section frame only, and the axis and origin are fixed by `R`). Hence the sector with
orientation `v` stands for the full ring whose sector `j` has orientation `R^j v`: the per-sector rotation of `v` is a property
of the ring the sector represents, not of the sector solve, and nothing in the sector computation changes. It is documented on
`CyclicOptions::beams`. A ring with the same `v` in every sector is a different structure (control NC-O).

**D4 (what stays refused).** `full_rotor` of a Beam2 model stays refused (`UnsupportedKind`, unchanged): it copies each block
into one block of the full mesh with one property, and the full ring needs `R^j v` per copy (one orientation per block,
`mesh.rs`); a per-copy orientation is a follow-up. The rotating paths (`RotatingPart`, `RotatingSector`) on a sector with the
option are not changed and not checked here (they become reachable where the stress-stiffening solve passes through
`solve_static_mpc`; listed under limits and proposed as a follow-up).

**D5 (defaults).** A model whose cyclic constraint does not set `beams` runs exactly the code it ran before: the new tests read
the field only in the two entry checks and in `boundary` behind `options.beams`. `CyclicOptions` gains a field (it is
`#[non_exhaustive]`, built by `Default` and builders; not serialised anywhere).

## The models

Axis and frame as `sa_cyclic_shells.rs`: `n = (1, 2, 2)/3`, origin `O = (0.3, -0.2, 0.5)`, `e_a = (2, -1, 0)/sqrt 5`, `e_b = n x
e_a`; `P(phi, r, z) = O + r (cos phi e_a + sin phi e_b) + z n`. `N = 6`, `alpha = 2 pi / N`. Steel `E = 2e11`, `nu = 0.3`, `rho =
7850`.

**Sector (the wheel sector W).** Ring nodes `0 .. 4` at `P(i alpha / 4, 1.0, 0)`; spoke node 5 at `P(alpha/2, 0.65, 0.125)`
(the midpoint of hub and ring node 2), hub node 6 at `P(alpha/2, 0.3, 0.25)`. Block 0 (ring) Beam2 `[0,1, 1,2, 2,3, 3,4]`,
section `BeamSection::new(1.8e-3, 1.35e-7, 5.4e-7, 3.7e-7, 5/6, 5/6)` (a 60 x 30 mm rectangle, `I_z = 4 I_y`), orientation `v_r =
0.8 n + 0.6 e_a`; block 1 (spoke) Beam2 `[6,5, 5,2]`, section `BeamSection::new(1.2e-3, 1.0e-7, 2.5e-7, 2.4e-7, 0.85, 0.75)`,
orientation `v_s = 0.6 e_b + 0.8 n`. Element set `all`; node sets `low = {0}`, `high = {4}`, `hub = {6}` (clamped, six
components), `p<q>` on each loaded node `q in {0, 1, 2, 3, 5}`. Constraint `Constraint::cyclic_with("low", "high", n, O, 6,
CyclicOptions::new().shells(true).beams(true))`. Neither `v` is parallel to an element: the ring chords have tangents `-sin phi_m
e_a + cos phi_m e_b` with `v_r . t = -0.6 sin phi_m`, `|.| <= 0.48`; the spoke direction `(0.7 (cos 30 e_a + sin 30 e_b) - 0.25
n) / 0.743` has `v_s . d = (0.21 - 0.20)/0.743 = 0.013` (nearly perpendicular). The orientation vectors are oblique to the axis,
so `R^j v != v` (D3 is exercised).

**Full ring.** Built in the test from `P` directly (not by `full_rotor` and not by rotating the sector's nodes): ring node `4 j +
i` at `P((4 j + i) alpha / 4, 1.0, 0)` (`i = 0 .. 3`, 24 nodes, closing on node 0), spoke node `24 + j`, hub `30 + j`; per sector
`j` two Beam2 blocks (ring `2 j`, spoke `2 j + 1`) with the sections of W and orientations `R^j v_r`, `R^j v_s`, `R^j =
rotation_matrix(n, j alpha)`; element set `all`; hubs clamped. Sector node `q` maps to ring node `4 j + q` (`q <= 4`, mod 24),
`24 + j` (`q = 5`), `30 + j` (`q = 6`). `R^0` is the identity exactly (`sin 0 = 0`, `1 - cos 0 = 0`), so sector 0 of the ring
has the bits of W's elements.

**Loads.** The force and moment of node `q` are `nodal_load(q)` of `sa_cyclic_shells.rs` (forces `1e3 sin/cos(...)` N, moments
`1e2 ...` N m). Static `k = 0` (S0): those nodal loads and `Load::centrifugal(n, O, 100)` on W, and on the ring the nodal loads
`R^j f`, `R^j m` on the images and the same centrifugal load. Harmonic static `k = 1, 2, 3` (S-k): the cosine part `nodal_load(q)`,
the sine part `nodal_load(q + 7)` (zero at `k = 3`), on the ring `R^j (cos(j theta) f_a - sin(j theta) f_b)` (as `s2` of
`sa_cyclic_shells.rs`).

**Modal options** (ADR 0581/0584 floor): `ModalOptions::new(m)` with `tolerance = 1e-12` and `ResidualTest::Backward` (as
`sa_cyclic_shells.rs`); sector `m = 4` per harmonic, ring `m = 30`.

## Arithmetic (by hand, disclosed)

1. **Sizes.** W: 7 nodes x 6 = 42 unknowns, hub 6 fixed, high node 6 dependent: 30 independent at `k = 0` and `k = 3`, 60 at `k =
   1, 2`. Ring: 36 nodes x 6 = 216, 36 fixed: 180 free. `30 < 180`.
2. **Lowest eigenvalue.** Sector mass: ring `7850 x 1.8e-3 x 1.044 = 14.8` kg (chord sum `8 sin 7.5 deg = 1.044`), spoke `7850 x
   1.2e-3 x 0.743 = 7.0` kg. Out-of-plane spoke stiffness about the hub `3 E I_y / L^3 = 3 x 2e11 x 1e-7 / 0.41 = 1.5e5` N/m;
   with the ring mass at its tip `lambda ~ 1.5e5 / 16.5 ~ 9e3`; the ring spans between spokes soften the `k > 0` modes: `lambda_1
   ~ 1e3 .. 1e4` rad^2/s^2.
3. **Floor of the default relative residual** (ADR 0581). `lambda_max`: the shear-rotary mode of a spoke element, `~ 12 kappa G A
   / (rho I L^2) = 12 x 0.85 x 7.7e10 x 1.2e-3 / (7850 x 1e-7 x 0.137) ~ 9e12`; `lambda_max / lambda_1 ~ 1e9 .. 1e10`, `eps` times
   it `~ 2e-7 .. 2e-6`: far above `1e-10`. So the backward test is used (floor `c eps`).
4. **Eigenvalue differences.** Sector 0 of the ring has W's element matrices bit for bit; sectors `j >= 1` have the rotated
   geometry rounded at `~ eps |x|`, which perturbs each element matrix by `~ eps ||K_e||` against its near-rigid motion in a
   low mode. Bound: `|d lambda| / lambda <~ eps ||K_e|| / (m_e lambda_1)`, `||K_e|| ~ E A / L = 2e11 x 1.8e-3 / 0.261 = 1.4e9`,
   `m_e = 3.7` kg, `lambda_1 >= 3e3`: `2.2e-16 x 1.3e5 = 3e-11`, a coherent worst case; with random signs and the cancellation
   seen by ADR 0556 (`1.7e-15` on the frustum) the expected value is `~ 1e-14 .. 1e-12`. Convergence: with backward residual
   `eta <= 1e-12` the Rayleigh quotient is off by `~ eta^2 lambda_max` (`1e-24 x 1e13 = 1e-11` absolute, `1e-15` relative).
   Limit `1e-10` (the acceptance): margin 3 over the worst case, 100 over the expected value.
5. **Mode shapes.** The expansion `phi_j = R^j (cos(j theta) a - sin(j theta) b)` of a sector mode `(a, b)` is a vector of the
   ring; the ring residual `K phi - lambda M phi` at its free unknowns is the reduced residual of the sector redistributed (an
   interior row is a row of the sector; a boundary node's row is the sum of the two sectors' contributions, which is the reduced
   row `T^T r` of that node), so its backward error `||r||_inf / ((||K||_inf + lambda ||M||_inf) ||phi||_inf)` is the sector's up
   to the ratio of the norms (`||K_r||` up to `2 ||K||` after the high rows fold onto the low rows, a factor up to `sqrt 3` for
   rotated `inf`-norms twice): `<= 6 eta`, `eta <= 1e-12` the sector's tolerance: `<= 6e-12`. Limit `1e-10`.
6. **Statics.** As ADR 0556 S1/S2 (`1.5e-14`, `1.1e-14` on the frustum); the beam stiffness ratio `E A/L : 3 E I / L^3 ~ 1.4e9 :
   1.5e5 ~ 1e4` gives `eps x 1e4 x (a few) ~ 1e-11` as a worst case. Limit `1e-10`.
7. **Orientation control NC-O.** In sector `j` of a ring with `v_r`, `v_s` unrotated, the angle between the section axis and the
   element differs from W's by up to `60 deg x j`; with `I_z = 4 I_y` the bending stiffness of a ring element out of the plane
   changes by `O(1)` (between `E I_y` and `E I_z`). The S0 field error is `O(0.1 .. 1)`. Limit `>= 1e-3`.

## Checks (limits fixed now)

Measures. Field error of a sector field `(a, b)` of harmonic `k` against a ring field `u` (as `field_error` of
`sa_cyclic_shells.rs`, over every sector node `q` and sector `j`): `e_u = max |R^j (c a_q - s b_q) - u_{image}| / max |u|` over
the translations, `e_theta` the same over the rotations, `e = max(e_u, e_theta)`. List protocol of the modes (as E1 of ADR 0556):
cut-off `lambda_c = (1 - 1e-6) min_k values_k[3]`; the ring list is complete if its largest eigenvalue is `>= lambda_c`; the
sector eigenvalues below `lambda_c` of every harmonic (each `eigenvalues` entry, pairs twice) sorted and paired with the ring's
below it; `d_k = max |lambda_s / lambda_r - 1|` over the pairs from harmonic `k`. Mode error `e_phi` per arithmetic 5, over the
modes of harmonic `k` whose eigenvalue is below `lambda_c` (both modes of a pair).

| Id | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| S0 | c1, c2 | W with the S0 loads, `solve_static_mpc`, against `solve_static` of the ring: `e` | `<= 1e-10` | arithmetic 6: `~1e-14`, worst `1e-11` |
| R1 | c1 | in the S0 solution, for the pair `(0, 4)`: `max |theta_4 - R theta_0| / max |theta|` | `<= 1e-14` | substitution, as ADR 0556 R1 |
| S1, S2, S3 | c2 | W, `solve_static_harmonic` `k = 1, 2, 3`, against the ring under the expanded load: `e` each | `<= 1e-10` | arithmetic 6 |
| M-list | c3 (set-up) | ring list complete; counts below `lambda_c` equal | complete; equal | D2, D3: the same discrete spectrum |
| M0 .. M3 | c3 | `d_k`, `k = 0 .. 3` | `<= 1e-10` each | arithmetic 4: worst `3e-11`, expected `~1e-13` |
| P0 .. P3 | c3 | `e_phi` of the modes of harmonic `k` in the ring | `<= 1e-10` each | arithmetic 5: `<= 6e-12` |
| Q1, Q2 | c3 | the pair splits of `k = 1, 2` (`max pair_splits`) | `<= 1e-10` | the doubled problem is block-diagonal: rounding (ADR 0556 `2.1e-15`) |
| B1 | c4 | the existing tests of `rcfd-fem` that cover the changed lines (release): `sa_cyclic_shells`, `sa_cyclic_shell_extensions`, `sa_cyclic_shell_two_sided`, `sa_rotating_cyclic_shells`, `sa_hyper_structures_f666`, `si_beam_transformation`, `si_cyclic`, `si_cyclic_harmonic`, `si_rotating_cyclic`, `sa_ortho_cyclic`, `sa_ortho_cyclic_default_bits`, `si_shell_transformation`, the lib unit tests | pass | D5 |
| B2 | c4 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | D5 |
| B3 | c4 | a Shell4 sector without beams (the S1 frustum sector of ADR 0556, rebuilt in the new test) under `shells(true)` and under `shells(true).beams(true)`: `solve_static_mpc` displacements and the `k = 0 .. 3` frequencies (4 each) | bits equal | D1: the option only removes a refusal |

## Negative controls

| Id | Control | Expected |
|---|---|---|
| NC1 | W with `CyclicOptions::new().beams(true)` (no shells; low and high also given as surface sets is impossible for a beam, so node sets): `solve_static_mpc`, `modal_analysis_harmonic` `k = 1` | refused, key `cyclic`, the message names ADR 0595 |
| NC2 | W with `shells(true).beams(true).shell_extensions(true).axis_nodes(true)`, and the same with `.interpolation(TieOptions::default())` in place of `axis_nodes`: `solve_static_mpc` | refused, key `cyclic`, naming ADR 0595 |
| NC3 | W with `shells(true)` only: `solve_static_mpc`, `modal_analysis_harmonic` `k = 0` | refused `NotYetSupported { variant: "Cyclic" }` naming `beam::FOLLOW_UP` (unchanged) |
| NC4 | W with the option: `full_rotor` | refused `UnsupportedKind { kind: "Beam2" }` (D4, unchanged) |
| NC-O | the ring with `v_r`, `v_s` unrotated in every sector, S0 | `e >= 1e-3` (arithmetic 7) |

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| X1 | `check_transformation_model` ignores the option (Beam2 refused as before) | S0, R1 (refused) |
| X2 | `check_cyclic_model` ignores the option | S1 .. S3, M, P, Q (refused) |
| X3 | `cyclic_rows` emits the translation rows only for the matched pairs (`bases = [0]`) | S0, R1, S1 .. S3, M, P |
| X4 | the rotation rows (components 3 to 5) of the pairs use `R^T` in place of `R` | R1, S0, S1 .. S3, M, P |
| X5 | the refusal of `beams` without `shells` removed | NC1 |
| X6 | the refusal of `beams` with `interpolation` or `axis_nodes` removed | NC2 |

X3 and X4 change lines that also serve shells; they are run to show that the beam checks see the rotation rows, not to test
shells.

## Decision rule

**Done** if S0, R1, S1 .. S3, M-list, M0 .. M3, P0 .. P3, Q1, Q2, B1, B2, B3 and NC1 .. NC4, NC-O all hold in the registered run
and every mutation is caught by at least one check. **Partial** if a clause c1 .. c4 is met only in part (a miss recorded as a
`*_known_miss`, a check not run, a mutation not caught). **Todo** if the sector with Beam2 cannot be taken without changing a
default result.

## Limits (not covered)

- `full_rotor` and `FullRotor::expand*` of a Beam2 sector (one orientation per block; D4): the ring here is built in the test.
- The rotating paths (`RotatingPart`, `RotatingSector`, whirl and Campbell) on a sector with Beam2: not checked.
- Beam ends on interpolated boundaries or on the axis (refused, NC2); a beam node shared with a shell on a cyclic boundary (taken
  by the checks of `check_supported_with_structures`, not exercised here).
- The warping beam (seven unknowns per node) is not a Beam2 element and is not in the cyclic constraint.
- Line loads on beams in a sector (a global direction, not rotated per sector by the path; the loads here are nodal and
  centrifugal), the hyperelastic material on beams in the cyclic constraint (ADR 0593 A-B: refused before by the cyclic check;
  with the option it reaches the material checks; not run).
- The case schema: no case key for the option.

## Amendment 1 (2026-11-15, after run 1, before the corrected run): the NC1 set-up

Run 1 (`run1.log` in the scratch folder) ran every test once. `nc1_to_nc4_refusals` stopped in its set-up: the NC1 model (`beams(true)`
without `shells`) cannot be built, because `SolidModel::add_constraint` checks the boundary sets of a cyclic constraint by kind
(surface sets without `shells`) and refused the node set `low` with `UnknownSet { kind: Surface, name: "low" }` before any path
was called. NC2 to NC4 in the same function were therefore not run. Diagnosis: a defect of the set-up and of D1, not of a limit:
D1 put the refusal of `beams` without `shells` in the entry checks and in `boundary`, which the model never reaches; the
helper `check_beams_option` scans `model.constraints()`, which does not yet hold a constraint being added.

Corrections, registered now:

- **D1a.** `SolidModel::add_constraint` refuses a cyclic constraint with `beams` and without `shells` (key `cyclic`, naming ADR
  0595) before its set check; the refusals in the entry checks and `boundary` stay (a constraint cannot reach them without
  `shells` any more; they stay as a guard). A constraint without `beams` meets exactly the code it met before.
- **NC1 (amended).** `add_constraint` of `Constraint::cyclic_with("low", "high", n, O, 6, CyclicOptions::new().beams(true))` on
  W's mesh: refused, key `cyclic`, the message names ADR 0595. NC2 to NC4 unchanged.
- The run-1 function is kept unchanged as an ignored record (`#[ignore]`, renamed `nc1_to_nc4_refusals_run1_record`); the
  corrected copy is `nc1_to_nc4_refusals_amended`, run once.
- **X5 (amended).** The refusal of D1a removed: caught by NC1 (amended) (the `UnknownSet` of run 1 in its place).

Every other check of run 1 is a registered result and is not re-run (they are in addendum 1).
