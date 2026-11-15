# ADR 0561: F-647 registration: shells on the rotating cyclic paths and the full rotor (the Coriolis matrix of the shell director, full_rotor and its expansions on six components)

Status: **REGISTERED 2026-11-15, written before any code of F-647 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-647 was written. The files under "What was read" were read, one
entry-path probe was run on the unchanged library (disclosed below: it printed only `Ok` or the error of each call, no number of
any registered check), and the arithmetic below was done by hand (no script; disclosed). Results go in the addenda at the end;
nothing above the addenda is changed after a run.

Roadmap item F-647 (dep F-628), the follow-up proposed by [ADR 0556](0556-f628-registration-shells-in-the-cyclic-constraint.md)
(its remaining questions 1 and 2). Number 0561 given by the lead. Branch `f647-work` from 0e1e4ea0 (which contains F-628),
worktree `w49i-work/f647`, scratch `w49i-work/f647-scratch`. The purpose given in the brief: the 120 mm computer-fan blade study
(spinning shell blades on one sector).

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no edit of existing tests; no new dependency, no
unsafe code; clean-room. `docs/ROADMAP.md` and `docs/roadmap.json` are not edited (the entries go in "For the lead"). Every choice
below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-647 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. With CyclicOptions::shells, the rotating paths on a cyclic sector (Campbell, whirl, pre-stressed and spin-softened modes) take Shell4 and Shell3, and full_rotor with FullRotor::expand, expand_field and expand_load copy shell blocks and expand six components; a spinning ring of Shell4 and the same cut into Shell3 against one sector: frequencies of each harmonic at each speed within 1e-8, expanded modes M-orthonormal within 1e-8; models without shells keep their bits.

Clauses: **a0** every limit registered here; **a1** with `CyclicOptions::shells` the rotating paths on a cyclic sector take Shell4
and Shell3: the Campbell table (`RotatingSector::campbell`), the whirl paths (`RotatingSector::whirl_modes` and the paths on the
same Coriolis matrix: `whirl_campbell`, `whirl_state`, `split_modes`), the pre-stressed modes (stress stiffening without spin
softening) and the spin-softened modes (spin softening without stress stiffening), and the modes with both
(`RotatingSector::modes`); **a2** `full_rotor` copies shell blocks; **a3** `FullRotor::expand`, `expand_field` and `expand_load`
expand six components (the rotations by the same `R^j` as the translations); **a4** a spinning ring of Shell4 against one sector:
the frequencies of each harmonic at each speed within 1e-8; **a5** the same cut into Shell3; **a6** the expanded modes
M-orthonormal within 1e-8; **a7** models without shells keep their bits.

## What was read (no run but the probe)

`AGENT_RULES.md`; the brief; the F-647 and F-628 entries of `docs/roadmap.json`; ADR 0556 (all of it: D1 the opt-in
`CyclicOptions::shells`, D2 the rotation rows, D3 the frustum, its checks and Limits, which name this item); ADR 0403 (D1 the
rotating part, `K_eff = K + Omega^2 (K_sigma,1 - M_perp)`), ADR 0407 (the rotating sector per harmonic index, the Campbell
families), ADR 0411 (D1 the Coriolis matrix `C = int rho N_a N_b [a x]`, `G = 2 Omega C`, skew bit for bit; D5 the sector:
`C` commutes with the sector shift because `R [a x] R^T = [a x]`), ADR 0472 / 0481 (split modes on the modal subspace), ADR 0473
(D3 the spin-softening mass of the shell director, D6: "the whirl and Coriolis paths keep the solid routines and refuse a beam or a
shell through them"), ADR 0432 (the Shell4 facet, its six unknowns, the director kinematics `u + zeta theta x e_3`), ADR 0454
(Shell3), ADR 0522 (corotated shells; nothing of it is on these paths), ADR 0496 / 0502 (the layup mass moments `m0, m1, m2` and
the layup spin-softening mass). Sources at 0e1e4ea0: `rotating.rs` (`RotatingPart`, `RotatingSector`, `check_model`,
`spin_softening_mass`, `track_families`), `rotating/whirl.rs` (`coriolis_matrix`: every element through `element_mass` of
`element_for(kind)`, which for Shell4 and Shell3 is the placeholder that refuses; `RotatingPart::coriolis`, the only caller of
`coriolis_matrix` inside the crate besides `gyro::gyroscopic_matrix`, used by every whirl, whirl-state and split path),
`rotating/whirl/state.rs`, `gyro.rs` (`split_modes` through `part().coriolis()`), `cyclic.rs` (`full_rotor` refusing every
non-solid block with `UnsupportedKind`; `FullRotor::expand`, `expand_field`, `expand_load` on components 0 to 2 only;
`shells_opted_in`), `mpc.rs` (`check_transformation_model`, `cyclic_rows` with the rotation rows of ADR 0556), `geometric.rs`
(`Prestress::from_static` through `geometric_struct::prestress_from_static` for shells), `geometric_struct.rs`
(`element_spin_softening_mass`), `shell.rs` (`facet`, `local_mass`, `local_mass_layup`, `local_spin_softening_mass`,
`local_spin_softening_mass_layup`, `model_spin_softening_mass`, `transformation`, `to_global`), `shell3.rs` (`local_mass`,
`local_spin_softening_mass`), `laminate.rs` (`single_isotropic`, `Laminate::mass`), `model.rs` (`DofMap::components`),
`tests/sa_cyclic_shells.rs` (the D3 geometry and helpers, copied, not edited).

**Audit of the tests that assert refusals this work would lift.** Searched every test of the workspace that names
`full_rotor`, `coriolis_matrix`, `whirl`, `split_modes`, `gyroscopic_matrix` or `RotatingSector` together with `Shell4`,
`Shell3`, `Beam2` or `UnsupportedKind`: none asserts that a whirl, Coriolis or split path refuses a shell, and none asserts that
`full_rotor` refuses a shell block (`sa_tie.rs` calls `full_rotor` on an extended boundary, `sa_ortho_rotating.rs` R2 and R3
assert the refusal of the orthotropic material on Shell4, on paths not changed here; `sa_cyclic_shells.rs` N-controls and
`si_shell_transformation.rs` / `sa_shell3.rs` N2 are on the static and modal paths, not changed here). Even so every lifted
refusal stays behind an opt-in (D1): a model without the option gets exactly the refusal it got before (negative controls NC2 to
NC4).

**Entry-path probe (disclosed).** A temporary test file (`tests/zz_probe_f647.rs`, deleted after the run, never committed)
built a small Shell4 and Shell3 sector (6 sectors about the z axis, clamped, `CyclicOptions::shells(true)`) and printed only
`Ok` or the error of each call on the unchanged library: `RotatingSector::new`, `modes(1, 0)`, `modes(1, 100)`, `campbell`,
`geometric::prestressed_modes`: `Ok` (the F-628 entry checks already let these through); `whirl_modes(1, 100)` and
`split_modes(1, 100)`: `Element { NotYetSupported { variant: "Shell4" / "Shell3", location: "element routines" } }` (the
Coriolis matrix); `full_rotor`: `UnsupportedKind { kind: "Shell4" / "Shell3", operation: "full_rotor" }`. So the library work
is the Coriolis matrix of the shells and the full rotor; the modes, Campbell, pre-stressed and spin-softened paths get their
first checks here and no library line.

## Design

### D1. The opt-in routes

1. **The sector.** With `CyclicOptions::shells` (ADR 0556 D1) on the model's cyclic constraint, `RotatingPart::coriolis` (and
   so every whirl, whirl-state, whirl-Campbell and split path of `RotatingPart`, `RotatingSector` and `gyro`) assembles the
   Coriolis matrix with the shell routine of D2 for Shell4 and Shell3 elements. Beam2 keeps the solid routine (the placeholder
   refuses, as before; a Beam2 model with a cyclic constraint is refused by its entry checks anyway, ADR 0556).
2. **The whole part.** A part without a cyclic constraint (the reference ring of the checks; a single blade) takes the same
   routine only with a new option `RotatingOptions::shells: bool` (default `false`, builder `RotatingOptions::shells(on)`).
   Without it a shell part's whirl paths refuse exactly as before (NC3). The public `coriolis_matrix(model, axis, threads)` and
   `gyro::gyroscopic_matrix` keep their behaviour (solid routines; a shell refused, NC4); a new public
   `coriolis_matrix_with_shells(model, axis, threads)` is the shell-taking assembly that `RotatingPart::coriolis` calls when
   either option is on.
3. **The full rotor.** `full_rotor` takes Shell4 and Shell3 blocks when `cyclic::shells_opted_in(model)`; Beam2 and every
   structural block without the option keep `UnsupportedKind` (NC1, NC2). Shell blocks are copied as solid blocks are (kind,
   connectivity through the node map, property).
4. **The expansions.** `FullRotor::expand`, `expand_field` and `expand_load` treat every sector node with six unknowns
   (`DofMap::components(p) == 6`) by the same formula on the translations (components 0 to 2) and on the rotations (components 3
   to 5), the rotations by the same `R^j` (ADR 0556 D2: the rotation vector turns with the sector as the translation). A node with
   three unknowns runs exactly the code it ran before (the same loop, the same operations), so every existing expansion keeps its
   bits; a six-unknown node cannot reach these functions without the option (`full_rotor` refuses).

So with neither option no line executed for a model changes, and a model without shells executes the same element routine
(solid elements never take the shell branch). a7 holds by construction and is checked by B1 to B4.

### D2. The Coriolis matrix of the shell director (first principles)

ADR 0411 D1: the term of the kinetic energy `T = 1/2 int rho |w_t + Omega a x (X - p + w)|^2` bilinear in `w_t` and `w` is
`Omega int rho w_t . (a x w) dV`; with `w = N q` it is `Omega q_t^T C q`, `C = int rho N^T [a x] N dV`, and Lagrange gives the
gyroscopic matrix `G = 2 Omega C` (skew since `[a x]` is). For a shell the displacement of the material point at distance `z`
from the reference surface along the facet normal is (ADR 0432; ADR 0496 for the layup) `w = u + z theta x e_3 = u + z W theta`,
`W` in facet axes the matrix with `W theta = (theta_2, -theta_1, 0)` (that is `[[0, 1, 0], [-1, 0, 0], [0, 0, 0]]`, the
`beta_x = theta_y`, `beta_y = -theta_x` of `local_mass_layup`). With `u = sum_a N_a u_a`, `theta = sum_a N_a theta_a` and the mass
moments `m0 = int rho dz`, `m1 = int rho z dz`, `m2 = int rho z^2 dz` (homogeneous: `rho t`, `0`, `rho t^3/12`), the node block
of `C` in facet axes is, with `s_ab = int N_a N_b dA` by the element's own mass rule (Shell4: the 2 x 2 Gauss rule of
`local_mass`; Shell3: `A (1 + delta_ab) / 12` of its `local_mass`):

- translations-translations `m0 s_ab [a x]`;
- translations-rotations `m1 s_ab [a x] W`, rotations-translations `m1 s_ab W^T [a x]`;
- rotations-rotations `m2 s_ab W^T [a x] W`, and by hand (arithmetic 1) `W^T [a x] W = [[0, -a_3, 0], [a_3, 0, 0], [0, 0, 0]]`
  (`a_3` the normal component of the axis): the drilling rotation takes no term, as in the spin softening (ADR 0473 D3).

`a` is the axis in facet components (`a_i = e_i . a`), the matrix is carried to global axes by the element's own
`transformation` (`T^T C_l T`, which also carries the offset link of a warped facet), and then made skew in floating point
(`c_ij <- (c_ij - c_ji) / 2`, `c_ji <- -c_ij`, `c_ii <- 0`) so that the assembly is skew bit for bit (as ADR 0411 D1). A Shell4
element of a Layup block takes, for one isotropic ply, the homogeneous call of the ply's thickness and density, and otherwise the
laminate's `m0, m1, m2` (as `model_spin_softening_mass`). The block `[a x] W` with `([a x] W)^T = W^T [a x]^T = -W^T [a x]` makes
the matrix skew before rounding too.

Objectivity (needed for the sector): the facet frame is built from the node positions (ADR 0556 D2), and `a`'s facet components
are the same in every sector because `R a = a`; so the element `C` of sector `j + 1` is `Q C Q^T`, `Q = blockdiag(R, R)` per
node, and ADR 0411 D5's argument (a matrix that commutes with the sector shift enters index `k` as `T_k^T blockdiag(C_s, C_s)
T_k`) holds with ADR 0556's six-unknown `T_k`. Nothing of the whirl solvers changes.

Uniform fields (the closed forms of CO1): for one planar element of area `A` with `v_1, v_2` uniform translations and `theta_1,
theta_2` uniform rotations (every node the same vector), `sum_ab s_ab = A`, so

- `v_1^T C v_2 = m0 A v_1 . (a x v_2)`,
- `theta_1^T C theta_2 = m2 A (theta_1 x e_3) . (a x (theta_2 x e_3))`,
- `v_1^T C theta_2 = m1 A v_1 . (a x (theta_2 x e_3))` (zero for a homogeneous shell),

all in global components (`W theta` is `theta x e_3` in any frame). These follow from the kinematics alone and are checked
against the assembled matrix.

### D3. The test geometry (fixed now)

The frustum of ADR 0556 D3, unchanged: axis `n = (1, 2, 2)/3` through `o = (0.3, -0.2, 0.5)`, `r(s) = 1.0 - 0.4 s` m, `z(s) = 0.8
s` m, `N = 6` sectors, `n_c = 2` columns of elements per sector, `n_a = 3` along the generator, Shell4 or the quads cut into
Shell3 along the diagonal from the first node, thickness `0.05` m, steel `E = 2e11` Pa, `nu = 0.3`, `rho = 7850` kg/m^3, the row
`a = 0` clamped on all six components, node sets `low` (column 0) and `high` (column 2), `CyclicOptions::shells(true)`. The
reference ring is **`full_rotor` of the sector** (a2): its mesh, a node set `r<p>` added on every ring node, the material on `all`
(the union of the copies), the clamp on `clamp` (the union), no cyclic constraint. F1 checks that this mesh is the ring of the
formula (ADR 0556's independent `shell_mesh`) node by node.

Rotating set-up. Axis and origin: the cyclic ones. Sector: `RotatingSector::new(m, opts(4))`; ring: `RotatingPart::new(ring,
n, o, opts(24).shells(true))`; `opts(m)` = `RotatingOptions::new(m)` with the modal options of ADR 0556 (`tolerance 1e-12`,
`ResidualTest::Backward`). Three effect sets: **A** both effects (the defaults), **B** pre-stressed modes (`spin_softening(false)`),
**C** spin-softened modes (`stress_stiffening(false)`); whirl **W** = A with `coriolis(true)`. Speeds (fixed by rule, not by
search): `Omega in {0, 0.2 omega_1, 0.4 omega_1}`, `omega_1 = sqrt(min_k lambda_{k,1})` the lowest sector eigenvalue at rest over
`k = 0 .. 3` of the kind (`modal_analysis_harmonic`, `opts(4)` modal options). Below `0.4 omega_1` set C keeps `K_eff` positive
definite: `phi^T M_perp phi <= phi^T M phi` (`P = I - a a^T <= I`, and the shell's rotational factor is `<= 1` on the rotations
it touches, ADR 0473 D3), so `lambda_1(Omega) >= lambda_1 - Omega^2 >= 0.84 lambda_1`; a loss of stiffness at any registered
speed is a set-up failure (reported, not tuned).

### D4. The measures

**Frequencies (MA, MB, MC, W).** At each speed and effect set: per harmonic `k = 0 .. 3` the sector's list (for the modes every
eigenvalue of `RotatingSector::modes(k, Omega).eigenvalues`, a pair's two entries; for the whirl the `omega` of every mode of
`RotatingSector::whirl_modes(k, Omega)`), the cut-off `c = (1 - 1e-6) min_k (largest entry of k)`; the sector list = every entry
below `c` of every harmonic; the ring list = the ring's entries below `c` (`RotatingPart::modes(Omega).eigenvalues` or the
`omega` of `RotatingPart::whirl_modes(Omega)`); counts equal and the ring's largest entry `>= c` (else a set-up failure), and
after sorting both lists `max |f_s / f_r - 1|` (frequencies: `sqrt` of the eigenvalues; whirl: `omega`), reported per harmonic
as the maximum over its entries.

**Orthonormality (OR).** At each speed of set A, every sector mode of every harmonic (`k = 0`: 4, `k = 1, 2`: 8, `k = 3`: 4; 24
vectors) expanded by `FullRotor::expand(sector dofs, ring dofs, k, mode)`; `G = Phi^T M_ring Phi` (`M_ring` the ring part's
`mass()`); `max |G - I|`.

**Residual (X2).** Each expanded mode `phi` with the sector eigenvalue `lambda` against the ring's own operator at that speed
(`RotatingPart::operator(Omega)` of the ring, set A): `max_free |(K_eff - lambda M) phi| / (max |K_eff phi| + |lambda| max |M
phi|)`, rows of the clamped unknowns excluded.

**Static expansion (XF, XL).** `k = 1, 2, 3`: the sector load over its `DofMap`, at every non-clamped sector node `q` (the high
nodes too, so that `expand_load` sums the two sectors at a shared node) `f_a(q) = (f_q, m_q)` of ADR 0556 D4 and, for `k = 1, 2`,
`f_b(q)` with `q` replaced by `q + 7`; `solve_static_harmonic`; the ring load `F = expand_load(sd, rd, k, f_a, f_b)` applied as
`NodalForce` and `NodalMoment` on each ring node `r<p>`; `solve_static` of the ring; `e = max(e_u, e_theta)` between
`expand_field(sd, rd, k, cos, sin)` of the sector solution and the ring solution (`e_u` the largest translation difference over
the largest ring translation, `e_theta` the same for the rotations). XL: `F` against the hand sum over sectors `j` and every sector
node `p` of `Q^j (cos(j theta) f_a(p) - sin(j theta) f_b(p))` at ring node `node(j, p)`, `max |F - F_hand| / max |F_hand|`.

## Checks (limits fixed now)

| Id | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| F1 | a2 | `full_rotor` of the Shell4 sector and of the Shell3 sector: node count 48, element count 36 (Shell4) / 72 (Shell3), block kind and `BlockProperty::shell(0.05)` copied, `clamp` of 12 nodes; every ring element's nodes at the formula ring's element nodes (matched by position), `max |x - x_formula| / 1 m` | counts equal; property equal; `<= 1e-12` | `rotate_point` against the formula: a few roundings of `O(1)` coordinates, `~ 1e-15` |
| CO1a | a1 | one planar tilted Shell4 (trapezoid, area by formula) of steel, `t = 0.05`, axis `n`: `v_1^T C v_2`, `theta_1^T C theta_2` against D2 (relative); `|v_1^T C theta_2| / (sqrt(m0 m2) A |v_1| |theta_2|)` | `<= 1e-13` each | sums of `~ 50` products of `O(1)` numbers |
| CO1b | a1 | the same for one Shell3 (a triangle of the same plane) | `<= 1e-13` each | as CO1a |
| CO1c | a1 | one Shell4 of a two-ply Layup (ply 1 steel `0.02` m, ply 2 `E = 7e10`, `nu = 0.33`, `rho = 2000`, `0.03` m, both at angle 0): the three forms against D2 with `m0, m1, m2` from the closed form `sum rho_k t_k (1, zbar_k, zbar_k^2 + t_k^2/12)`, `zbar` from `-t/2` along the facet's `e_3` (by hand: `m1 = -1.755` kg/m) | `<= 1e-13` relative each | as CO1a |
| CO1d | a1 | the assembled `C` of the CO1 models and of the D3 sector (Shell4, Shell3) skew bit for bit (`c_ij = -c_ji`, `c_ii = 0`) | exact | D2 skew-symmetrisation and ADR 0411 D1 |
| MA4, MA3 | a1, a4, a5 | set A (both effects): Shell4 (MA4), Shell3 (MA3), each speed, counts equal, per harmonic `max |f_s/f_r - 1|` | equal; `<= 1e-8` | ADR 0556 E1/E2 measured `<= 2.0e-15` at rest; the rotating operator is the same objective sum (D2), and the pre-stress of the sector equals the ring's to ADR 0556 S1 (`1.5e-14`); acceptance limit |
| MB4, MB3 | a1, a4, a5 | set B (pre-stressed modes), as MA | as MA | as MA |
| MC4, MC3 | a1, a4, a5 | set C (spin-softened modes), as MA | as MA | as MA |
| W4, W3 | a1, a4, a5 | set W (whirl, Coriolis on, dense solver): `RotatingSector::whirl_modes` against the ring's `whirl_modes` (ring with `RotatingOptions::shells(true)`), each speed, counts equal, per harmonic `max |omega_s/omega_r - 1|` | equal; `<= 1e-8` | the dense symmetric linearisation of ADR 0411 D2: rounding `~ eps omega_max / omega_1 ~ 1e-13`; acceptance limit |
| CA4, CA3 | a1 | `RotatingSector::campbell(&[0, 1, 2, 3], speeds)` (set A): at each speed and harmonic the multiset of the tracks' eigenvalues equals the `values` of `RotatingSector::modes(k, Omega)` | equal bits | the table is built from the same calls (`track_families`) |
| OR4, OR3 | a3, a6 | set A, each speed: `max |Phi^T M_ring Phi - I|` over the 24 expanded modes | `<= 1e-8` | the eigensolver's M-orthonormality (`~ 1e-14`) carried by the exact identity `sum c_j^2 = N/2` (D4); acceptance limit |
| X24, X23 | a3 | set A, each speed: the residual of D4 of every expanded mode in the ring's `K_eff` | `<= 1e-8` | backward residual tolerance `1e-12` of the sector solve; the ring operator equals the expanded sector operator to the S1 level (`1e-14`) |
| XF4, XF3 | a3 | `k = 1, 2, 3`: `expand_field` of `solve_static_harmonic` against the ring solved under `expand_load`, the maximum over `k` (each reported) | `<= 1e-10` | ADR 0556 S2 measured `<= 1.1e-14` on the same problem |
| XL4, XL3 | a3 | `expand_load` against the hand sum (D4), the maximum over `k` | `<= 1e-14` | the same products in a different order: one or two roundings |
| SM4, SM3 | a1 | smoke: `RotatingSector::split_modes(k, 0.4 omega_1)` (`coriolis(true)`), `whirl_state(k, 0.4 omega_1)` for `k = 0 .. 3` and `whirl_campbell(&[0, 1, 2, 3], speeds)` return `Ok` with finite frequencies | `Ok`, finite | they call the same `coriolis()` (D1); no accuracy is claimed for them here |
| B1 | a7 | every existing rcfd-fem test passes unchanged (release), in particular `sa_cyclic_shells`, `si_rotating_cyclic`, `si_rotating_whirl*`, `sa_gyro_sector*`, the `*_default_bits` records, `sa_tie`, `sa_ortho_rotating` | pass | no line on their paths changes (D1) |
| B2 | a7 | `cargo test -p rcfd-api --release --test baseline_hashes`; `cargo test -p rcfd-cli --release --test docs_site` | pass | as B1 |
| B3 | a7 | the Hex8 twin sector of ADR 0556 B3: default options (surface sets) against node sets with `CyclicOptions::shells(true)` and `RotatingOptions::shells(true)`: `RotatingSector::modes(k, Omega)` eigenvalues and `whirl_modes(k, Omega)` omegas for `k = 0 .. 3` at `Omega in {0, 200, 400}` rad/s, the `full_rotor` node coordinates, `expand` of the `k = 1` mode 0 at rest, `expand_field` and `expand_load` of a fixed `k = 1` vector pair; and the full rotor's `RotatingPart` whirl at 400 rad/s with and without `RotatingOptions::shells(true)` | equal bits | D1: no six-unknown node, the same routines in the same order |
| B4 | a7 | the B3 default-options values hashed (FNV-1a 64 of the bits) equal the hash recorded by running the B3 default part on the unchanged library (this commit plus only the new test file; a set-up run, disclosed in the addendum, before any library change) | equal | as B1 |

## Negative controls

| Id | Control | Expected |
|---|---|---|
| NC1 | `full_rotor` of a Beam2 model with node sets `low`/`high` and `CyclicOptions::shells(true)` | `UnsupportedKind { kind: "Beam2", operation: "full_rotor" }` |
| NC2 | `full_rotor` of a Hex8 box with a disjoint Shell4 strip and the default options on surface sets of the box (ADR 0556 NC2's set-up) | `UnsupportedKind { kind: "Shell4", .. }` as before |
| NC3 | the D3 ring (no cyclic constraint) as a `RotatingPart` with the default `RotatingOptions`: `whirl_modes(100)` | refused as before: `Element { NotYetSupported { variant: "Shell4" } }` |
| NC4 | the public `coriolis_matrix` of the D3 Shell4 sector (with `CyclicOptions::shells`) | refused as before (the solid routines; `variant: "Shell4"`) |

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the rotation block of the shell Coriolis matrix zero (translations only) | CO1a, CO1b |
| MU2 | the rotation block with the opposite sign (`-W^T [a x] W`) | CO1a, CO1b |
| MU3 | the translation block of the shell Coriolis matrix with `P = I - a a^T` (the spin-softening factor) in place of `[a x]` | CO1a, CO1b, CO1d |
| MU4 | the layup coupling `m1` with the opposite sign | CO1c |
| MU5 | `expand` without the rotations (components 3 to 5 left zero) | OR4, OR3, X24, X23 |
| MU6 | `expand` with the rotations by `R^{-j}` (`R^j` transposed) | X24, X23 |
| MU7 | `expand_field` without the rotations | XF4, XF3 |
| MU8 | `expand_load` without the rotations | XF4, XF3, XL4, XL3 |
| MU9 | `full_rotor` takes shell blocks without the option | NC2 |
| MU10 | `full_rotor` drops the block property of shell blocks | F1 (and every ring check: the ring is refused) |
| MU11 | `RotatingPart::coriolis` takes the shell routine only when neither option is on (inverted guard) | W4, W3, SM, NC3 |

No mutation is declared equivalent.

## Decision rule

**done**: every check F1 to B4 met and NC1 to NC4 behave as stated, every mutation caught. **partial**: any registered check
missed (recorded as a `*_known_miss`), or any mutation uncaught, or a check met only after a post-run amendment. **todo**: the
positive checks MA to XF not run.

## Arithmetic (by hand, disclosed)

1. `W^T [a x] W` with `W = [[0, 1, 0], [-1, 0, 0], [0, 0, 0]]`: columns of `[a x] W` are `[a x] (0, -1, 0) = (a_3, 0, -a_1)`,
   `[a x] (1, 0, 0) = (0, a_3, -a_2)`, `0`; `W^T v = (-v_2, v_1, 0)`; so the columns of `W^T [a x] W` are `(0, a_3, 0)`, `(-a_3,
   0, 0)`, `0` (components 1-based).
2. Layup of CO1c: `t = 0.05`, `z_0 = -0.025`, `zbar_1 = -0.015`, `zbar_2 = 0.010`; `m0 = 7850 x 0.02 + 2000 x 0.03 = 217`, `m1 =
   7850 x 0.02 x (-0.015) + 2000 x 0.03 x 0.010 = -2.355 + 0.6 = -1.755`, `m2 = 157 (2.25e-4 + 3.333e-5) + 60 (1e-4 + 7.5e-5) =
   0.040558 + 0.0105 = 0.051058` kg.
3. Counts as ADR 0556 D3: sector 72 unknowns, 36 / 72 independent; ring 288, 216 free; the 24 ring modes and 24 expanded sector
   modes are well below 216.

## Limits (what is not covered)

- The Coriolis matrix of Beam2 (the solid routine still refuses it; a beam with a cyclic constraint is refused anyway).
- The accuracy of the split modes (ADR 0481), the whirl state and the sparse whirl solvers with shells: they take the same
  matrix (D1) and are only smoke-checked (SM); the dense whirl is the one compared with the ring.
- The shell Coriolis matrix is checked by its uniform-field forms (CO1) and by the sector-ring consistency, not against an
  independent analytical whirl solution of a spinning shell (no closed form of a clamped cone is in the repository).
- `geometric::prestressed_modes` on the sector (index 0) is reached through F-628's transformation path (the probe) and is not
  checked here; the pre-stressed modes of the rotating sector (set B) are.
- Interpolated boundaries and axis nodes with shells (F-648), warped facets, laminated shells on the sector (the layup takes the
  Coriolis routine, CO1c, but no layup sector is run), the corotated shells of ADR 0522.

## Amendment 1 (2026-11-15, written before any registered run)

The set-up of NC2 is corrected before its run. ADR 0556 NC2's model (a Hex8 box of `hex_box([0, 1, 0], [1, 1, 1])` with
surface sets `ymin` / `ymax` and a cyclic constraint of 4 sectors about z) was built only to reach the entry checks of the
harmonic paths, which refuse before the boundary is matched; its two faces are not images of each other under the quarter turn
(`(x, 1, z) -> (-1, x, z)` is not on `y = 2`), and `full_rotor` matches the boundary before it checks the block kinds, so on that
model it would report the unmatched boundary (key `cyclic`) and NC2 would not test the kind check at all. NC2 instead uses the
Hex8 twin sector of B3 (default options, surface sets `low` / `high`, which match) with one disjoint Shell4 element (thickness
0.01 m, four new nodes away from both boundaries) in a second block; expected, as registered, `UnsupportedKind { kind: "Shell4",
operation: "full_rotor" }`. MU9 is still to be caught by NC2. Nothing else changes.
