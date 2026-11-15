# ADR 0556: F-628 registration: shells in the cyclic constraint (the rotation unknowns of Shell4 and Shell3 nodes rotated as the translations)

Status: **REGISTERED 2026-11-15, written before any code of F-628 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-628 was written, compiled or run: the files under "What was read" were read, and the
arithmetic below was done by hand (no script, no rcfd run; disclosed). Results go in the addenda at the end; nothing above the
addenda is changed after a run.

Roadmap item F-628 (deps F-541, F-522), the follow-up named by [ADR 0545](0545-f541-registration-shell3-on-the-remaining-paths.md)
(its known miss KM1: the cyclic constraint refuses Shell4 and Shell3 together) and by
[ADR 0435](0435-f522-registration-shells-on-the-transformation-and-body-loads.md) (a shell model with a cyclic constraint refused
naming a follow-up of ADR 0432). Number 0556 given by the lead. Branch `f628-work` from d4ae697d, worktree `w49i-work/f628`,
scratch `w49i-work/f628-scratch`. The purpose given in the brief: rotating fan blades modelled as shells.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no edit of existing tests; no new dependency, no
unsafe code; clean-room. `docs/ROADMAP.md` and `docs/roadmap.json` are not edited (the entries go in "For the lead"). Every choice
below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-628 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. The cyclic rows of solve_static_mpc, modal_analysis_mpc and the cyclic harmonic paths rotate the rotation unknowns of Shell4 and Shell3 nodes as the translations; a full ring of Shell4 and the same cut into Shell3 against one sector with the cyclic constraint: static displacements within 1e-10 and the frequencies of each harmonic within 1e-8; models without shells keep their bits.

Clauses: **a0** every limit registered here; **a1** the cyclic rows of `mpc::solve_static_mpc`, `mpc::modal_analysis_mpc` and the
cyclic harmonic paths (`cyclic::modal_analysis_harmonic`, `cyclic::solve_static_harmonic`, both on
`mpc::harmonic_transformation`) rotate the rotation unknowns of Shell4 and Shell3 nodes by the same `R` as the translations;
**a2** a full ring of Shell4 against one sector with the cyclic constraint: static displacements within 1e-10; **a3** the same for
the ring cut into Shell3; **a4** frequencies of each harmonic within 1e-8 (Shell4 and Shell3); **a5** models without shells keep
their bits.

## What was read (no run)

`AGENT_RULES.md`; the brief; the F-628, F-541 and F-522 entries of `docs/roadmap.json`; ADR 0545 (D0, D3, KM1, its consistency
test over 14 entry paths); ADR 0435 (D1: `check_transformation_model`, the cyclic refusal); ADR 0386 (the rows `a_H = R (c a_L - s
b_L)`, `b_H = R (s a_L + c b_L)`, its checks E1 to E5 and S2, their measured values 4e-14 to 3.3e-13, the ResidualTest::Backward
and 1e-12 tolerance of its modal runs); ADR 0395 (harmonic static loads); ADR 0456 D4 (interpolated boundaries, axis nodes); ADR
0462 (`check_cyclic_axes`); the F-629 commits of ADR 0552 (its opt-in route for multigrid); the sources at d4ae697d: `cyclic.rs`
(`boundary`, `check_cyclic_model`, `modal_analysis_harmonic`, `solve_static_harmonic`, `full_rotor` and its refusal of structural
blocks, `FullRotor::expand*` on three components), `mpc.rs` (`transformation`, `cyclic_rows`, `harmonic_transformation`,
`check_transformation_model`), `model.rs` (`Constraint::Cyclic`, `CyclicOptions`, the validation of `add_constraint`: low and high
must be surface sets), `mesh.rs` (`surface_set_nodes`; `shell::faces()` and `shell3::faces()` are empty: a shell has no face, so
a surface set cannot name the nodes of a shell edge), `shell.rs` and `shell3.rs` (the facet frame: `e3` from the facet, `e1` along
the first edge, `e2 = e3 x e1`; no global direction enters), `laminate.rs` (ply angles from the element's first axis), `loads.rs`
(`NodalMoment` on six-unknown nodes, `ShellPressure`, `Centrifugal` on Shell4 and Shell3); the tests that assert the current
refusals: `tests/si_shell_transformation.rs` (N2: a Hex8 box with a disjoint Shell4 strip and a cyclic constraint on surface sets
of the box, refused naming `shell::FOLLOW_UP` on `solve_static` and `modal_analysis`) and `tests/sa_shell3.rs` (N2: the same with
Shell3 on `solve_static_mpc`, naming `shell3::FOLLOW_UP`), twice in that file.

## Design

### D1. The opt-in route: `CyclicOptions::shells`

The refusals of a shell model with a cyclic constraint are asserted by existing tests (`si_shell_transformation.rs` N2,
`sa_shell3.rs` N2 twice) that may not be edited; and a pure shell sector cannot be written with the default options at all,
because a shell has no faces and the boundaries of `Constraint::Cyclic` are surface sets. So the capability is opt-in, as ADR 0552
made the six-unknown smoothed aggregation a new preconditioner variant instead of changing the default one: a new field `shells:
bool` of `CyclicOptions` (default `false`; builder `CyclicOptions::shells(on)`), which

1. names the boundaries `low` and `high` as **node sets** (validated by `add_constraint` as node sets; `boundary` takes their
   nodes ascending, deduplicated) instead of surface sets;
2. lets `check_transformation_model` (static solve and modal analysis on the transformation) and `check_cyclic_model` (the
   harmonic paths) take Shell4 and Shell3 blocks: the former keeps its checks and drops only the shell refusal; the latter calls
   `check_supported_with_structures` (shell nodes shared with solids stay refused there, ADR 0435 D1) instead of
   `check_supported`; Beam2 with a cyclic constraint stays refused naming `beam::FOLLOW_UP` on both (out of scope: a beam's
   orientation vector is a global direction, it is not rotated with the sector);
3. adds, in `cyclic_rows`, for every matched pair whose two nodes carry six unknowns, the rows of the rotation unknowns
   `theta_H = R (c theta_L - s phi_L)` and, with a sine part, `phi_H = R (s theta_L + c phi_L)` (D2), after the translation rows of
   the pair, with the same designated-dependent rule (the high unknown);
4. refuses a matched pair whose two nodes carry different numbers of unknowns (a shell node against a solid node), naming both
   nodes (key `cyclic`), and refuses `shells` together with `interpolation` or `axis_nodes` (ADR 0456 D4) naming this ADR (a
   follow-up: the rotation rows of interpolated and axis nodes are not derived here).

The point-material branch of `check_cyclic_model` (ADR 0462) is unchanged and still refuses shells; `full_rotor` still refuses
structural blocks (its expansion is on three components) — both listed in Limits.

With `shells = false` no line executed for a model changes: the boundary is read from surface sets as before, the refusals fire as
before, and the rotation rows are not emitted (the branch is guarded by the option). So a5 holds by construction for every
existing model; it is checked by B1 to B4.

### D2. Why the rotations take the same `R` (derivation)

The small-rotation unknowns `theta` of a Shell4 or Shell3 node are the three global components of the infinitesimal rotation
vector of the director (ADR 0432): the displacement of a material point at offset `z e_3` from the node is `u + theta x (z e_3)`.
Let the sector `j + 1` be the image of sector `j` under the rigid rotation `x -> o + R (x - o)`, `R` proper (`det R = 1`). A field
of sector `j` carried rigidly to sector `j + 1` has `u' = R u` and, for the offset point, `u' + theta' x (z R e_3) = R (u + theta x
(z e_3)) = R u + (R theta) x (z R e_3)` (for proper `R`, `R (a x b) = (R a) x (R b)`), so `theta' = R theta`: the rotation vector
transforms as the translation (an axial vector under a proper rotation is an ordinary vector). The element matrices are objective:
the facet frame of both elements is built from the node positions only (`e_3` from the facet, `e_1` along the first edge), so
the element of sector `j + 1` has `K' = Q K Q^T` with `Q = blockdiag(R, R)` per node, and the same for the mass and for the loads
that turn with the sector (pressure along `e_3`, the centrifugal load about the cyclic axis, nodal forces and moments given
rotated). The block-circulant reduction of ADR 0386 then holds unchanged with the node transformation `Q` in place of `R`: the
rows of D1.3 are those of ADR 0386 for the rotation components. Nothing else of the transformation changes.

### D3. The test geometry (fixed now)

A conical frustum about the oblique axis `n = (1, 2, 2) / 3` through `o = (0.3, -0.2, 0.5)` (so that `R` is a full 3 x 3 matrix,
no component untouched). Orthonormal `e_a = (2, -1, 0)/sqrt(5)`, `e_b = n x e_a`; a node at generator parameter `s in [0, 1]` and
angle `phi` is `o + r(s) (cos phi e_a + sin phi e_b) + z(s) n`, `r(s) = 1.0 - 0.4 s` m, `z(s) = 0.8 s` m. `N = 6` sectors (so the
harmonics are `k = 0` single, `k = 1, 2` pairs, `k = 3 = N/2` single); per sector `n_c = 2` elements around and `n_a = 3` along
the generator; node `(i, a)` at `phi = 2 pi i / (N n_c)`, `s = a / n_a`. The full ring has `N n_c = 12` columns (48 nodes, 36
Shell4); the sector has columns `i = 0 .. 2` (12 nodes, 6 Shell4), node set `low` = column 0, `high` = column 2, every node of
both computed by the formula (the high column is the low column rotated to within rounding, well inside `MATCH_TOLERANCE`).
Shell4 `(i, a), (i+1, a), (i+1, a+1), (i, a+1)`: each quad is an isosceles trapezoid (two points on each of two parallel circles,
symmetric about the bisecting meridian plane), hence planar. Shell3: each quad cut along the diagonal from `(i, a)` into `(i, a),
(i+1, a), (i+1, a+1)` and `(i, a), (i+1, a+1), (i, a+1)`, the same in ring and sector. Thickness `t = 0.05` m, steel `E = 2e11`
Pa, `nu = 0.3`, `rho = 7850` kg/m^3, default element options. The column `a = 0` (the large circle) is clamped: six
`Constraint::Displacement` (components 0 to 5) on node set `clamp`.

Arithmetic (by hand): sector unknowns 72; clamped 3 nodes, 18; the high column 4 nodes, 24 cyclic rows, of which the 6 of the
clamped high node are redundant (both sides prescribed zero) and dropped: 72 - 18 - 18 = 36 independent at `k = 0` and `k = 3`,
72 at `k = 1, 2`. Full ring 288 unknowns, 72 clamped, 216 free.

### D4. Loads

**k = 0 static (S1, `solve_static_mpc`).** Model loads in the sector: `Centrifugal` about `(n, o)` at `omega = 100` rad/s;
`ShellPressure` `p = 2e4` Pa on every element; on each non-high, non-clamped sector node `p` (6 nodes) its own node set with
`NodalForce` `f_p` and `NodalMoment` `m_p`, `f_p = 1e3 (sin(1.3 q + 0.2), cos(0.7 q + 0.5), sin(2.1 q + 1.1))` N and `m_p = 1e2
(cos(1.9 q + 0.3), sin(0.4 q + 0.9), cos(1.1 q + 1.7))` N m, `q` the sector node index. The full ring carries the same
centrifugal load and pressure and, at the copy of node `p` in sector `j`, `R^j f_p` and `R^j m_p` (`R^j` = `rotation_matrix(n, 2
pi j / N)`), solved by `solve_static` (no transformation: Displacement constraints only).

**Harmonic static (S2, `solve_static_harmonic`, `k = 1, 2, 3`).** The cosine and sine parts over the sector `DofMap`: at each
non-high, non-clamped node `p`, translations and rotations `f_a(p) = (f_p, m_p)`, and for `k = 1, 2` `f_b(p) = (f_p, m_p)` with
`q` replaced by `q + 7`; zero elsewhere (the high nodes are loaded from the next sector's low nodes in the full ring; ADR 0395).
The full ring: at the copy of `p` in sector `j`, `Q^j (cos(j theta) f_a - sin(j theta) f_b)`, `theta = 2 pi k / N`, `Q^j =
blockdiag(R^j, R^j)`, as per-node `NodalForce` and `NodalMoment`, solved by `solve_static`.

**Modes (E1, E2).** `modal_analysis_harmonic(k, ModalOptions::new(4))` for `k = 0 .. 3` (`k = 0` on `modal_analysis_mpc`), the
full ring `modal_analysis(ModalOptions::new(24))`, every modal run with `ResidualTest::Backward` and tolerance `1e-12` (as ADR
0386 E1).

## Checks (limits fixed now)

Measures. Static: for every node `p` of every sector `j` of the ring (sector node `p`, ring node of `(j, i, a)`), the sector field
mapped, `Q^j (cos(j theta) a_p - sin(j theta) b_p)` (`k = 0`: `Q^j a_p`), against the ring field; `e_u = max |du|_inf / max
|u_ring|_inf` over the translations, `e_theta` the same over the rotations; the measure is `max(e_u, e_theta)`. Modes: the cut-off
`lambda_c = (1 - 1e-6) min_k lambda_{k,4}` (the fourth sector value of each harmonic; below it every harmonic's spectrum is
complete); the sector list = every value below `lambda_c` of every harmonic, a pair entered twice; the ring list = the ring
eigenvalues below `lambda_c`; counts equal, the largest of the 24 ring eigenvalues `>= lambda_c` (else the ring list could be
incomplete: a set-up failure), and after sorting both lists `max |f_s / f_r - 1|` (the sorted pairing is within `delta` whenever
any pairing is); reported per harmonic (the maximum over the entries of each `k`).

| Id | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| S1a | a1, a2 | Shell4, `k = 0`, `solve_static_mpc` against the full ring, `max(e_u, e_theta)` | `<= 1e-10` | the same discrete problem (D2); element rounding in rotated coordinates `~ eps` relative, amplified by the condition of the clamped cone, `t/h ~ 0.1 .. 0.2`, bending/membrane `~ (h/t)^2 ~ 40`, drilling penalty `1e-3`: `kappa ~ 1e6` at most, so `~ 1e-10` as a bound and `~ 1e-13` as ADR 0386 S2 measured on solids; acceptance limit |
| S1b | a1, a3 | Shell3, as S1a | `<= 1e-10` | as S1a |
| S2a | a1, a2 | Shell4, `solve_static_harmonic` `k = 1, 2, 3` against the full ring under the expanded load; the maximum over `k` (each reported) | `<= 1e-10` | as S1a |
| S2b | a1, a3 | Shell3, as S2a | `<= 1e-10` | as S1a |
| E1 | a1, a2, a4 | Shell4: counts equal; per harmonic `k = 0 .. 3` `max |f_s/f_r - 1|`; also every pair split of `k = 1, 2` | equal; `<= 1e-8` each; split `<= 1e-8` | ADR 0386 E1: rounding `~ eps lambda_max / lambda`, `lambda_max / lambda_1 ~ 1e6`, below 1e-10; eigenvalue error `~ residual^2` |
| E2 | a1, a3, a4 | Shell3, as E1 | as E1 | as E1 |
| E3 | a1 | the `k = 0` branch is `modal_analysis_mpc`: the `k = 0` sector frequencies of E1 and E2 equal `modal_analysis_mpc` of the sector bit for bit | equal bits | the same call (ADR 0386) |
| R1 | a1 | Shell4 and Shell3 `k = 0` sector solution: the rotation rows hold, `max_pairs |theta_H - R theta_L|_inf / max |theta|` | `<= 1e-14` | substitution of the transformation, exact up to one product rounding |
| B1 | a5 | every existing rcfd-fem test passes unchanged (release), in particular `si_shell_transformation` N2, `sa_shell3` N2 (the refusals with the default options), `sa_tie` (ADR 0456 cyclic options), the cyclic tests of ADR 0386 / 0395 / 0462 | pass | no line on their paths changes (D1) |
| B2 | a5 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | as B1 |
| B3 | a5 | a Hex8 ring sector without shells (the solid twin of D3: `hex`-like 2 x 3 x 1 sector of the same frustum, thickness direction one element, built from Hex8 with surface sets `low`/`high` of the cut faces): `solve_static_mpc` and `modal_analysis_harmonic` `k = 0 .. 3` with the default options against `shells(true)` and node sets of the same nodes: displacements and frequencies bit for bit | equal bits | D1: with no six-unknown node the rows are the same rows in the same order |
| B4 | a5 | the same B3 default-options results hashed (the `to_bits` of the displacements and of the frequencies, FNV-1a 64) equal the hash recorded by running the B3 default-options part on the unchanged library at d4ae697d (a set-up run made once, disclosed in the addendum, before any library change) | equal | as B1 |

## Negative controls

| Id | Control | Expected |
|---|---|---|
| NC1 | the D3 shell sector with the default options: `add_constraint(Constraint::cyclic("low", "high", ...))` | refused, `UnknownSet` of kind Surface (no face on a shell) |
| NC2 | a Hex8 box with a disjoint Shell4 strip, surface sets of the box, default options (the existing N2 set-up) on `modal_analysis_harmonic` and `solve_static_harmonic` | refused naming Shell4 (`check_supported`, as before) |
| NC3 | `shells(true)` on a Beam2 model with node sets: `solve_static_mpc`, `modal_analysis_harmonic` | refused `NotYetSupported { variant: "Cyclic" }` naming `beam::FOLLOW_UP` |
| NC4 | `shells(true)`, a pair of a six-unknown shell node (low) and a three-unknown Hex8 node (high) | refused, key `cyclic`, naming both nodes and their unknown counts |
| NC5 | `shells(true)` with `axis_nodes(true)`, and with `interpolation` | refused naming ADR 0556 |
| NC6 | the D3 Shell4 sector with `shells(true)` but the high column moved by `1e-6 L` | refused by the boundary match (unmatched low node), as ADR 0386 R1 |

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| M1 | the rotation rows not emitted (translations only, as before) | S1a, S1b, E1, E2, R1 |
| M2 | the rotation rows with `theta_H = theta_L` (identity in place of `R`) | S1a, E1, R1 |
| M3 | the rotation rows with `R^T` | S1a, E1, R1 |
| M4 | the sign of `s` in the rotation rows of the sine part flipped (`phi_H = R (-s theta_L + c phi_L)`) | S2a (k = 1, 2), E1 (k = 1, 2) |
| M5 | `check_cyclic_model` keeps `check_supported` under the option | S2a, E1 (refused) |
| M6 | `check_transformation_model` keeps the Shell3 refusal under the option | S1b, E2 |
| M7 | the unknown-count check of the pair removed | NC4 (the error is then `MissingComponent`, not key `cyclic`) |
| M8 | the boundary read from surface sets regardless of the option | every positive check (refused) |
| M9 | the rotation rows emitted for the pair only when the option is off (inverted guard) | S1a, B3 |

No mutation is declared equivalent.

## Decision rule

**done**: every check S1a to B4 met and NC1 to NC6 behave as stated, every mutation caught. **partial**: any registered check
missed (recorded as a `*_known_miss`), or any mutation uncaught, or a check met only after a post-run amendment. **todo**: the
positive checks S1a to E2 not run.

## Limits (what is not covered)

- The rotating paths on a cyclic sector (`rotating::*` Campbell, whirl, pre-stressed and spin-softened modes, split Campbell of
  ADR 0481, the direct harmonic solve of ADR 0543) keep their own entry checks; they call `harmonic_transformation` and would get
  the rotation rows, but their checks still refuse shells and are not opened here (a follow-up; needed for the spinning fan blade).
- `full_rotor` and `FullRotor::expand`, `expand_field`, `expand_load` still refuse structural blocks / work on three components.
- Interpolated boundaries and axis nodes with shells (refused, NC5).
- Beam2 with a cyclic constraint (refused, NC3).
- A shell node shared with a solid (refused as before by `check_supported_with_structures`).
- The point-material branch (orthotropic or tabulated solids, ADR 0462) with shells; laminated shells (`Layup`) are taken (their
  ply angles are from the element's first axis, objective) but are not checked here.
- Warped facets are not in the D3 geometry (planar trapezoids); objectivity of the warped facet (its projection uses the node
  positions only) is argued, not checked.

## Addenda

(none yet)
