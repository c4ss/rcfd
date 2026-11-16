# ADR 0635: F-709 registration: the torque-free centrifugal load of a layup with a first mass moment

Status: **REGISTERED 2026-11-16, written before any code of F-709 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-709 was written and nothing of rcfd was compiled or run; the
arithmetic below was done by hand and evaluated with a plain numpy prototype in the scratch folder
(`w49i-work/f709-scratch/probe_f709.py`, `probe_f709.log`; no rcfd code; disclosed, its numbers quoted below). Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-709 (dep F-686), the follow-up proposed by
[ADR 0633](0633-f686-registration-the-free-sector-prestress-on-tetrahedral-layered-shell-and-orthotropic-sectors.md) (its D1
finding and remaining question 1, option (a)). Number 0635 given by the lead. Branch `f709-work` from 21322aa8 (the F-686 tip),
worktree `w49i-work/f709`, scratch `w49i-work/f709-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited;
defaults bit-identical (the new term is opt-in); no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any
golden file; no new dependency, no unsafe code; clean-room (the derivation below is from first principles). The roadmap files are
not edited. Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation).

## What F-709 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. An opt-in adds the force term m1 Omega^2 r_perp(e_3) to the centrifugal load of a Shell4 layup; with it the load of a layup with m1 != 0 on a facet in a meridian plane has a torque about the axis within 1e-12 of zero relative to m1 A rho Omega^2, its force resultant matches the through-thickness integral of the exact body force within 1e-12, and a free layup sector of such facets with free_sector_prestress gives the pre-stress of a constrained-copy route within 1e-10; the defaults bit-identical.

Clauses: **a0** every limit registered here; **a1** an opt-in adds `m1 Omega^2 r_perp(e_3)` to the centrifugal load of a Shell4
layup (D1, D2; the checks FR, NB); **a2** with it, on a facet in a meridian plane with `m1 != 0`, the torque about the axis is
within `1e-12` of zero relative to `m1 A rho Omega^2` (TQ-M); **a3** its force resultant matches the through-thickness integral
of the exact body force within `1e-12` (FR-M); **a4** a free layup sector of such facets with `free_sector_prestress` gives the
pre-stress of a constrained-copy route within `1e-10` (FS); **a5** the defaults bit-identical (DF, NB, B1, B2).

Readings (taken on the standing instruction): `r_perp(e_3)` is the part of the facet normal perpendicular to the axis, `P e_3`,
`P = I - a a^T` (the linear part of `r_perp(x) = P (x - O)`; D1). "rho" in `m1 A rho Omega^2` is the distance of the facet's
area centroid from the axis, `rho_c = |P (x_c - O)|`, the scale of ADR 0633 OL. "Within 1e-12" of the force resultant is relative
to the length of the exact resultant. "A free layup sector of such facets": a cyclic sector cannot consist of meridian facets
alone (a facet on the cyclic boundary would be its own image), so the sector is a fan-like one, D3: a hub ring of chordal facets
(mirror-symmetric about their mid-meridian plane, torque-free with and without the opt-in, ADR 0633 D1) carrying a flat blade
whose every facet lies in the meridian plane at mid-sector, the facets that give the default load its torque.

## What was read

`AGENT_RULES.md`; the brief; ADR 0633 (all: D1 and its finding, the OL check, the models, the copy route, the test
`sa_free_sector_prestress_tet_layup_ortho_f686.rs`), ADR 0502 D1 to D4 (the layup body load, the kind of rule in `loads.rs`),
ADR 0611 and 0606 through ADR 0633. Sources at 21322aa8: `loads.rs` (`nodal_load_vector`, the `Load::Centrifugal` and
`Load::Gravity` arms, `perpendicular`, `ShellBody`, `shell_body_vector`, `add_shell_body_load`), `shell.rs`
(`layup_body_load_vector_at`, `facet`, `transformation`, `point`, `GAUSS`), `laminate.rs` (`Laminate::mass = (m0, m1, m2)`,
`single_isotropic`), `model.rs` (`SolidModel`: its fields, the two opt-in flags of ADR 0584 and ADR 0614 and their setters),
`rotating.rs` (`unit_centrifugal_model`: it copies the two flags; `RotatingPart::new`, `free_sector_unit_static`: both build the
pre-stress model through `unit_centrifugal_model`). `SolidModel::new` is called in the library sources only in
`unit_centrifugal_model`.

## Design

### D1. The exact through-thickness load (first principles)

The shell kinematics of ADR 0496/0502: a point of the layer at height `z` above the mid-surface point `x` of a flat facet with
unit normal `e_3` is at `y = x + z e_3`, its displacement `u + z (theta x e_3)`. The centrifugal body force per unit volume at a
rotation `Omega` about the unit axis `a` through `O` is `rho(z) Omega^2 P (y - O)`. On a flat facet `dV = dA dz` exactly. The
virtual work:

`delta W = int_A int_z rho(z) Omega^2 P (x + z e_3 - O) . (delta u + z delta theta x e_3) dz dA`

`= int_A Omega^2 [ m0 P(x - O) + m1 P e_3 ] . delta u + Omega^2 [ m1 P(x - O) + m2 P e_3 ] . (delta theta x e_3) dA`

with `(m0, m1, m2) = int rho (1, z, z^2) dz` (the laminate's `mass`). Four terms:

1. `m0 Omega^2 P(x - O) . delta u`: the library's force (ADR 0502 D1).
2. `m1 Omega^2 P e_3 . delta u`: **the force term this item adds** (`m1 Omega^2 r_perp(e_3)` of the acceptance). It is
   constant over a flat facet; per node `f_i += m1 Omega^2 (int N_i dA) P e_3`.
3. `m1 Omega^2 P(x - O) . (delta theta x e_3)`: the library's moment `m1 int N_i (e_3 x b) dA` (ADR 0502 D1).
4. `m2 Omega^2 P e_3 . (delta theta x e_3) = m2 Omega^2 delta theta . (e_3 x P e_3) = -m2 Omega^2 (a . e_3) delta theta . (e_3 x
   a)`: a moment the library drops for every shell (the homogeneous `Shell` too, `body_load_vector`). **Not added** (the
   acceptance names the force term only; Limits). It is torque-free: its work on the rotation `theta = a` of `r_2` is `-m2
   Omega^2 (a . e_3) a . (e_3 x a) = 0`, and `r_1` has no rotation.

**The force resultant** with term 2: `int_A Omega^2 (m0 P(x - O) + m1 P e_3) dA = Omega^2 A (m0 P(x_c - O) + m1 P e_3) =
Omega^2 A sum_k rho_k t_k P(x_c + zbar_k e_3 - O)`, the through-thickness integral of the exact body force, ply by ply (the
integrand is affine in `z` within a ply). Without term 2 the resultant misses it by `Omega^2 A m1 P e_3`.

**Torque-free** (the work on the cyclic rigid motions of ADR 0611 D1, `r_1 = (a, 0)`, `r_2 = (a x (x - O), a)`, at any point,
so at any quadrature rule that is exact for the interpolated rigid field, which holds for linear fields on the facet). `r_1`:
`(m0 P(x - O) + m1 P e_3) . a = 0` (`P a = 0`). `r_2`, with `r = x - O`: term 1 `m0 P r . (a x r) = m0 P r . (a x P r) = 0`;
terms 2 and 3: `m1 [ P e_3 . (a x r) + (e_3 x P r) . a ]`; `P e_3 . (a x r) = e_3 . (a x r)` (`a x r` is perpendicular to `a`)
`= e_3 . (a x P r)`, and `(e_3 x P r) . a = e_3 . (P r x a) = -e_3 . (a x P r)`: the sum is zero pointwise. Without term 2 the
work is `m1 Omega^2 int e_3 . (P r x a) dA` (ADR 0633 D1 finding), on a facet in a meridian plane `m1 A rho_c Omega^2` in
magnitude. So with the opt-in the load is orthogonal to `r_1` and `r_2` on any facet, and the free sector of ADR 0606 has a
reaction-free gauge whatever its facets.

### D2. The library change (opt-in, default bits unchanged)

- **`model.rs`**: a third opt-in flag of the model, as those of ADR 0584 and ADR 0614: field `layup_first_moment_force: bool`
  (default `false` in `SolidModel::new`), getter `layup_first_moment_force()`, setter `set_layup_first_moment_force(on)`. A model
  flag (not a field of `Load::Centrifugal`, which would change every literal and match of the public enum, and not a
  `RotatingOptions` field, which would not reach the static copy route) so that `nodal_load_vector` and every model derived from
  the model carry it; taken on the standing instruction (take the recommendation).
- **`rotating.rs`**: `unit_centrifugal_model` copies the flag (one line, as the two others), so that `RotatingPart::new` and
  `free_sector_unit_static` take the same load as the static solve of the model.
- **`shell.rs`**: a new public function `layup_body_load_vector_affine_at(coords, m0, m1, b, along)`, `along(v)` the derivative
  of the acceleration (density 1) along the direction `v` (centrifugal: `Omega^2 P v`): the vector of the **unchanged**
  `layup_body_load_vector_at(coords, m0, m1, b)` plus `T^T g`, `g` the local vector with `m1 (int N_i det) (e_k . d)` on the
  translations of node `i`, `d = along(e_3)`, `int N_i det` by the same 2 x 2 rule at the points of the projected facet (exact
  for the bilinear `det` of a flat quad), carried to the nodes by the same `T^T` (on a warped facet the same route as the other
  forces). `layup_body_load_vector_at` is not edited.
- **`loads.rs`**: `ShellBody::PerMass` takes a third field, an optional `along` rule. The `Load::Centrifugal` arm passes `Some`
  (`Omega^2 P v`, `P` with the axis normalised as in `perpendicular`) only when `model.layup_first_moment_force()`, otherwise
  `None`; `Load::Gravity` always `None` (a uniform field: its term is zero). `shell_body_vector`: in the branch of a layup of more
  than one ply, `Some` calls the new function, `None` the existing call unchanged. Every other branch (a `Shell` block, a layup of
  one isotropic ply, Shell3) ignores the field: their `m1` is zero (homogeneous section, mid-plane at the half-thickness, ADR 0496
  D2), so their exact term is zero and their bits stay those of before with and without the opt-in (NB).

Without the opt-in every changed line is on a branch not taken or passes `None` to a `match` arm whose body is the line of
before: the same operations on the same values. B1 (`baseline_hashes`) is run because shared solver files change.

### D3. The models (fixed now)

Frame of ADR 0611/0633: `a = (1, 2, 2) / 3`, `O = (0.3, -0.2, 0.5)`, `e_a = (2, -1, 0) / sqrt 5`, `e_b = a x e_a`. The layup L2
of ADR 0633 (two plies of the ADR 0496 ply material, `0.025` each, bottom 0 degrees `rho = 1400`, top 90 degrees `rho = 2800`;
`m0 = 105 kg/m^2`, `m1 = 0.4375 kg/m`, `m2 = 0.021875 kg`, probe). Block material steel (the plies carry their own).

- **FM** (the meridian facet, ADR 0633 OL): one Shell4, corners `O + r e_a + z a`, `(r, z) = (0.5, 0), (0.7, 0), (0.7, 0.2),
  (0.5, 0.25)`; `A = 0.045`, `rho_c = 0.5962963`, `e_3 = +- e_b` (probe).
- **FO** (an oblique facet, supplementary): `n0 = normalise(0.6 e_a + 0.5 e_b + 0.6 a)`, `u1 = normalise(a x n0)`, `u2 = n0 x u1`,
  `c0 = O + 0.6 e_a + 0.1 a`, corners `c0 + s u1 + t u2`, `(s, t) = (0, 0), (0.2, 0.02), (0.17, 0.21), (-0.03, 0.15)`: planar,
  no mirror symmetry about a meridian plane, `e_3 . a = 0.609` (an axial part), `A = 0.0352`, `rho_c = 0.4991122` (probe).
- **HB** (the free fan-like sector, a4): `N = 6` sectors (`alpha = pi / 3`). Hub: the cylinder of radius `0.5` and axial extent
  `[0, 0.3]`, nodes at `phi = 0, alpha/2, alpha` (columns 0, 1, 2) and `z = 0, 0.1, 0.2, 0.3`, 2 x 3 chordal Shell4 facets. Blade:
  the plate in the meridian plane `phi = alpha/2`, radius `0.5` to `1.0` in 3 divisions and `z` in the same 3 divisions, 3 x 3
  Shell4 facets, its root nodes the hub's column-1 nodes. Points `O + r (cos phi e_a + sin phi e_b) + z a`. One block, the L2
  layup on every element; node sets `low` (column 0), `high` (column 2); `Constraint::cyclic_with("low", "high", a, O, 6,
  CyclicOptions::new().shells(true))`; free. Interior nodes: the hub's column 1 and the blade's nodes. `r_max = 1.0` (the blade
  tip).

**The constrained-copy route** (ADR 0611 D2 with the copy-route nodes chosen apart from the library's gauge, which lies on the
blade tip): a copy with `Constraint::linear` `a . u_A = 0` at `A` = the hub column-1 node at `z = 0` and `t_B . u_B = 0` at `B` =
the hub column-1 node at `z = 0.3` (radius 0.5, the blade root), plus `Load::centrifugal(a, O, 1)`, `solve_static` default
options, `Prestress::from_static`. The copy is a clone of the model, so it carries the model's flag.

## Arithmetic (disclosed; probe numbers)

1. **TQ** (with the opt-in). By D1 the work on `r_2` is zero pointwise; what remains is the rounding of a 24-term sum. On FM the
   force terms are each perpendicular to `a x r` at the node to rounding (radial forces, tangential `r_2`): ADR 0633 OL measured
   the default's torque against its closed form at `1.5e-14` relative to `m1 A rho_c`. On FO the 12 force products are each of
   size `~ m0 A rho^2 Omega^2 / 4` and cancel: `~ 6 eps (m0 / m1) rho ~ 6 x 2.2e-16 x 240 x 0.5 ~ 2e-13` relative to `m1 A rho_c
   Omega^2`. Limit `1e-12` (the acceptance) for both.
2. **FR** (with the opt-in): the resultant of 8 node forces of size `|F| / 4` and the reference from the corners: `~ 1e-15`
   relative; limit `1e-12` (the acceptance). `|F_exact| = 7043.92 N` (FM), `4636.44 N` (FO) at `Omega = 50`.
3. **NF** (without the opt-in): the miss `m1 |P e_3| / |m0 P(x_c - O) + m1 P e_3|`: FM `6.99e-3`, FO `6.58e-3` (probe). Limit:
   at least `1e-3`.
4. **NT** (without the opt-in): the torque equals the ADR 0633 closed form `m1 A Omega^2 e_3 . (P(x_c - O) x a)`: FM `|.| = 1`
   relative to `m1 A rho_c Omega^2`, FO `-0.4694` (probe, with the probe's `e_3`; the test takes `e_3` from the corners as the
   library does, ADR 0633 OL). Limits: the difference `<= 1e-10` relative to `m1 A rho_c Omega^2` (as OL), and the ratio of size
   at least `0.1`.
5. **HB torque without the opt-in**: the hub facets are mirror-symmetric (zero, ADR 0633 D1); each blade facet `m1 A_j rho_cj`
   (rectangles in the meridian plane, `rho` linear on them), the sum `m1 int rho dA = m1 x 0.3 x (1 - 0.25) / 2 = 0.04921875`
   (`Omega = 1`). NT-HB: `|r_2^T f| / 0.04921875 - 1` within `1e-10`.
6. **FS** (HB with the opt-in): two reaction-free gauges (D1), as L2 of ADR 0633 (measured `1.7e-15`); the T-junction and the
   cantilever blade raise the condition number, `~ 1e-13` expected; limit `1e-10` (the acceptance). RX, RXc as ADR 0633
   arithmetic 3 (`<= 1e-12` expected, limit `1e-10`). OR `<= 1e-13` (as ADR 0633).
7. **NC1** (HB without the opt-in, both routes without it): the gauges carry the torque `0.0492 N m` as a tangential force of
   `0.049 N` at the tip (library) and `0.098 N` at `B` (copy): the difference of the two solutions is the response to that
   self-equilibrated pair, a point force in the hub membrane of size `~ 0.1 / (t h) = 0.1 / (0.05 x 0.1) = 20 Pa` near `B`
   against the centrifugal membrane stress `~ m0 int r dr / t = 105 x 0.375 / 0.05 ~ 800 Pa`: `~ 2e-2`, at least `1e-3`. Limit:
   FS without the opt-in at least `1e-5` (the property that the opt-in is what makes the gauge reaction-free on HB).

## Checks (limits fixed now)

Options: `base = RotatingOptions::new(3).coriolis(true)`, the free-sector opt-in `base.free_sector_prestress(true)`. "On" is
`set_layup_first_moment_force(true)` on the model, "off" the default.

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| TQ-M | a2 | FM on, `Omega = 50`, `nodal_load_vector` of `Load::centrifugal(a, O, 50)`: `abs(r_2^T f) / (m1 A rho_c Omega^2)` | `<= 1e-12` | arithmetic 1 |
| FR-M | a3, a1 | FM on: the sum of the node forces against `Omega^2 A sum_k rho_k t_k P(x_c + zbar_k e_3 - O)` (plies, `A`, `x_c`, `e_3` from the corners), `abs` of the difference over `abs` of the reference | `<= 1e-12` | arithmetic 2 |
| TQ-O | supplementary | as TQ-M on FO | `<= 1e-12` | arithmetic 1 |
| FR-O | supplementary | as FR-M on FO | `<= 1e-12` | arithmetic 2 |
| OR1 | D1 | FM, FO on: `abs(r_1^T f) / (abs(r_1) abs(f))` | `<= 1e-13` | `P a = 0` |
| NF | negative control | FM, FO off: the resultant's miss as in FR | `>= 1e-3` each | arithmetic 3 |
| NT | negative control | FM, FO off: `abs(r_2^T f - T_c) / (m1 A rho_c Omega^2)`, `T_c` the closed form; and `abs(T_c) / (m1 A rho_c Omega^2)` | `<= 1e-10`; `>= 0.1` | arithmetic 4 |
| FS | a4 | HB on: `RotatingSector::new(HB, opt-in).part().unit_prestress()` against the copy route: `max abs(S_new - S_copy)` over every element, point, component over `max abs(S_copy)` | `<= 1e-10` | arithmetic 6 |
| RX | supplementary | HB on: `free_sector_unit_static`; `K u - f` (`K` = `assemble::stiffness`, `f` = `nodal_load_vector` of the centrifugal copy, on) at the interior nodes, `max abs` over `max abs(f)` | `<= 1e-10` | arithmetic 6 |
| RXc | supplementary | the same residual of the copy route's solution | `<= 1e-10` | arithmetic 6 |
| OR | D1 | HB on: `abs(r_i^T f) / (abs(r_i) abs(f))`, `i = 1, 2` | `<= 1e-13` | arithmetic 6 |
| GN | D1 | HB: the gauge node interior and `abs(rho_g / r_max - 1)` | interior; `<= 1e-12` | ADR 0606 D3 |
| NT-HB | negative control | HB off: `abs(abs(r_2^T f) / 0.04921875 - 1)` | `<= 1e-10` | arithmetic 5 |
| NC1 | negative control | HB off, both routes off: FS as above | `>= 1e-5` | arithmetic 7 |
| NC0 | a5 | HB on and off, `RotatingSector::new(HB, base)` (no free-sector opt-in) | refused `SingularStiffness` | free sector |
| NB | a5, a1 | on against off, bits of `nodal_load_vector` of the centrifugal load (`Omega = 50`): a `Shell { 0.05 }` steel model of the FO facet, the FO facet with a layup of one isotropic steel ply of `0.05`, a Shell3 `Shell { 0.05 }` model of the FO facet split in two triangles, a Hex8 `hex_box` steel solid; and the gravity load `(1, -2, -9.81)` on FO with L2 | bits equal (each) | D2 |
| DF | a5 | `SolidModel::new(mesh).layup_first_moment_force()` | `false` | D2 |
| B1 | a5 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | D2 |
| B2 | a5 | existing tests unedited: `sa_laminate_body`, `sa_laminate_body_default_bits`, `sa_free_sector_prestress_tet_layup_ortho_f686` (its OL is the default load's torque, so it fails if the default changes), `sa_free_sector_prestress_solids_shells_f683`, `sa_free_sector_prestress_f680`, `cargo test -p rcfd-fem --release --lib` filtered `loads`, `shell`, `rotating`, `model` | pass | D2 |

Three test functions in `crates/rcfd-fem/tests/sa_layup_centrifugal_first_moment_f709.rs`: the facets (TQ, FR, OR1, NF, NT), the
sector (FS, RX, RXc, OR, GN, NT-HB, NC1, NC0), the defaults (NB, DF).

## Negative controls

NF and NT (without the opt-in the resultant misses the exact one by the predicted `m1 P e_3` and the torque is that of ADR
0633), NT-HB and NC1 (without the opt-in the HB sector's load has the blade torque and the free-sector stress depends on the
gauge, so FS tests the term), NC0 (the free sector is refused without the free-sector opt-in), NB (the opt-in does not touch
loads whose `m1` is zero or that are not centrifugal), and the mutations.

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| M1 | the new term's sign flipped (`-m1`) | TQ-M, TQ-O, FR-M, FR-O, OR (HB), FS |
| M2 | `m0` in place of `m1` in the new term | TQ-M, FR-M, FS |
| M3 | `unit_centrifugal_model` does not copy the flag | FS, RX (the sector's pre-stress without the term) |
| M4 | `P v` replaced by `v` (no projection) in the centrifugal `along` rule | FR-O, OR1 (FO) (FM and HB have `e_3 . a = 0`: not expected there) |
| M5 | the default of the flag `true` | DF, and `sa_free_sector_prestress_tet_layup_ortho_f686` OL (run in this mutation too) |

Each applied by a script to a `git archive` copy of the test commit, each with its own `CARGO_TARGET_DIR`, the new test file run
once per mutation with `--no-fail-fast` (M5 also the f686 file).

## Decision rule

**done**: every check TQ-M to B2 met in the registered run, every mutation caught by at least one of its expected checks.
**partial**: any registered check missed (recorded as a `*_known_miss`), a check met only after a post-run amendment, a check not
run, or a mutation not caught. **todo**: TQ-M, FR-M and FS not run.

## Limits (what is not covered)

- The moment term 4 of D1 (`m2 Omega^2 e_3 x P e_3`, torque-free) is not added, for layups or for homogeneous shells.
- Shell3 layups stay refused (ADR 0496 D7); the opt-in has no effect on Shell3, on `Shell` blocks and on one-ply layups (their
  `m1` is zero).
- The opt-in is a library flag of the model; no case-file key, CLI or schema entry (those would touch goldens).
- Gravity's term is zero (uniform field); the nodal body-force field is per volume and has no first-moment term (ADR 0502 D1).
- Warped facets: the term follows the existing projected-facet route; the exactness of D1 is derived for flat facets only and
  only flat facets are run.
- Whether the opt-in should become the default (it would change the results of every layup with `m1 != 0` under a centrifugal
  load, ADR 0633 option (b)) is the maintainer's decision; not taken.
