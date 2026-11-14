# ADR 0426: F-513 registration: beams on the transformation path (rigid sets joining a beam to a solid, nodal springs and linear constraints on rotations, the free rigid motions count with rotational unknowns)

Status: **REGISTERED 2026-11-14, written before any code of F-513 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-513 was written, compiled or run: the files listed under "What was read" were read and
the arithmetic below was done by hand and evaluated with a short script of closed forms (`w49i-work/f513-scratch/estimate.py`,
numpy, no rcfd code; disclosed, its numbers quoted below). Results go in the addendum at the end; nothing above the addendum is
changed after a run.

Parents: [ADR 0417](0417-f391-registration-six-unknowns-and-beam.md) (F-391: six unknowns per node, Beam2; its D4 refuses
beams on the transformation path naming `beam::FOLLOW_UP`, its D5 refuses a beam node shared with a solid with
`FemError::BeamSolidNode`, its limit L2 and remaining question 2 option (a) name this item),
[ADR 0296](0296-linear-multi-point-constraints-rigid-sets-elastic-supports.md) (F-393: the transformation `x = T q + g`, rigid
sets with six reference unknowns, elastic supports), [ADR 0384](0384-f486-registration-modal-analysis-on-the-transformation.md)
(`mpc::modal_analysis_mpc`), [ADR 0399](0399-f488-registration-zero-modes-on-the-transformation.md)
(`free_motions::free_rigid_motions`, which refuses rotational unknowns), [ADR 0422](0422-f514-registration-beam-line-and-gravity-loads.md)
(F-514: the `si_beam` N2/N3 copy this item retires again). Roadmap item F-513 (deps F-391, F-393). Number 0426 given by the lead.
Branch `f513-beams-transformation-path`, worktree `w49i-work/f513`, scratch `w49i-work/f513-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact); each registered run made once; defaults bit-identical; no edit of `baseline_hashes.rs`, of the hash
tables or of a golden file; no new dependency, no unsafe code; clean-room. The parallel item F-510 works in
`crates/rcfd-fem/src/rotating`; this item does not touch it (its paths keep refusing beams through `check_supported`).

## What F-513 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. A cantilever beam joined to a solid block by a rigid set
> on a face of the block gives the beam's tip deflection plus the block's compliance within the registered limit; a nodal spring
> on a rotation gives k_theta theta = M within 1e-10; the modal analysis of a beam held by a rigid set has the frequencies of the
> equivalent clamped beam within the registered limit; the free rigid motions count takes rotational unknowns.

Clauses: (a1) beam joined to a block by a rigid set on a face: tip = beam's tip deflection + the block's compliance, within a
limit registered here; (a2) a nodal spring on a rotation: `k_theta theta = M` within 1e-10; (a3) modal analysis of a beam held by
a rigid set: the frequencies of the equivalent clamped beam within a limit registered here; (a4) `free_rigid_motions` counts with
rotational unknowns. The brief adds: (b1) solid-only models give the same bits (`si_scaffold_bits.rs` unedited, existing tests
unedited); (b2) the BeamSolidNode refusal of a node shared directly stays; (b3) linear constraints on rotations (the title).

Readings, registered:

- **(a1)** "the block's compliance" is obtained **independently** of the joined model: a separate solid-only model (the same
  block mesh, material and clamp, the same rigid set on the same face with the same reference point, no beam) loaded at the
  reference point by the force and the moment the beam's root carries (statics: the tip force `F` and the tip moment `M_tip`
  transferred to the root, `F` and `M_tip + (x_tip - x_root) x F`), solved by `mpc::solve_static_mpc` with a `ReferenceLoad`
  (the code of ADR 0296, unchanged). Its reference motion `(t, w)` moves the beam's root rigidly; "the beam's tip deflection" is
  the cantilever closed form of ADR 0417 arithmetic 6. The prediction of the tip: `u_tip = t + w x (x_tip - x_R) + u_cant`,
  `theta_tip = w + theta_cant` (reference point `x_R` at the root). Compared relative to the largest component of each field.
- **(a2)** "a nodal spring on a rotation": `ElasticSupport::NodalSpring` on component 5 (`theta_z`) of a beam node; `M` the
  moment it carries. Checked with the moment applied at the spring's node (the spring alone holds the beam's rigid rotation) and
  with the moment at the tip (the spring and the beam in series).
- **(a3)** "a beam held by a rigid set": the beam's root node is the only beam node of a rigid set whose other nodes are the
  nodes of a face of a solid block with every block unknown prescribed to zero (the beam joined to a fixed solid); "the
  equivalent clamped beam" is the same beam mesh with the root's six unknowns prescribed to zero, solved on the elimination path.
  Both take the same modal options. A second, independent comparison against the Euler-Bernoulli cantilever closed form with a
  registered allowance for shear, rotary inertia and discretisation.
- **(a4)** "takes rotational unknowns": `free_rigid_motions` no longer refuses a model with rotational unknowns; the six scaled
  rigid fields carry their rotations at the rotational unknowns, and the prescribed, linear, rigid-set and support rows on
  rotational unknowns enter the rank. Checked by counts on models whose number of free rigid motions is known by hand, and by the
  rigid fields being in the null space of the beam stiffness.
- **(b3)** linear constraints on rotations: `Constraint::Linear` terms on components 3 to 5 (already accepted by
  `add_constraint` and `transformation`, unreachable for beams until now).

## What was read (no run)

- `AGENT_RULES.md`; the brief; `docs/roadmap.json` entries F-391, F-393, F-513.
- ADR 0417 (whole: D1 to D7, the checks, the addendum, the roadmap text proposing F-513), ADR 0422 (head), ADR 0399 (head and the
  counting rule), ADR 0296 (by its roadmap note).
- `crates/rcfd-fem/src/mpc.rs` (whole: `uses_transformation`, `RowOrigin`, `Transformation`, `transformation` with the rigid-set
  rows, `rigid_set_rows` of the harmonic transformation, `eliminate`, `support_triplets`, `add_supports`, `solve_static_mpc`,
  `modal_analysis_mpc`), `constraint.rs` (whole), `free_motions.rs` (whole), `solve.rs` (`solve_static_report`: the
  beam-aware check then the dispatch to `solve_static_mpc`), `modal.rs` (`modal_analysis_with`, the same), `model.rs`
  (`add_constraint`, `add_support`, `check_elements*`, `check_supported*`), `beam.rs` (`check_beam_model`, `FOLLOW_UP`), the
  callers of `mpc::transformation`, `solve_static_mpc` and `modal_analysis_mpc` (buckling, cyclic, geometric, harmonic,
  transient, nonlinear, rotating: each calls `check_supported` first), `builders.rs` (`hex_box`, its node and surface sets).
- Tests: `si_beam.rs` (whole: its running `n2_n3_other_paths_and_invalid_data_after_landed_amended` has the row "a nodal spring
  (the transformation) refused naming `beam::FOLLOW_UP`", which this item retires), grep of every test for `FOLLOW_UP`,
  `BeamSolidNode`, `free_rigid_motions` and "rotational" (no other test asserts the refusals this item lifts).
- **Not read:** any finite-element code, manual or documentation outside this repository. The kinematics of a rigid body under
  a small rotation and the clamped-free Euler-Bernoulli frequency equation are derived below from first principles.

## Retired row (standing decision: ignored record citing this ADR plus a running copy)

| file | test | row | why |
| --- | --- | --- | --- |
| `si_beam.rs` | `n2_n3_other_paths_and_invalid_data_after_landed_amended` | N2 "nodal spring (transformation)" refused naming `beam::FOLLOW_UP` | the transformation path now takes beams |

It gets `#[ignore = "record of ADR 0422 amendment 1 before ADR 0426 (F-513, beams on the transformation) retired the N2 nodal
spring row; superseded by the n2_n3_other_paths_and_invalid_data_after_transformation_landed copy (maintainer standing decision,
2026-11-13)"]` and nothing else. The copy keeps every other row as it is and replaces the retired row by: the model solves, and
the root reaction forces, the spring force and the tip force balance, `|sum R + S + F| / |F| <= 1e-10` (check R1 below).

## Arithmetic done before the code (disclosed)

1. **The rigid set with a six-unknown node.** A rigid body under a small rotation `w` about the reference point `x_R` and a
   translation `t` of it has the displacement field `u(x) = t + w x (x - x_R)`; its gradient `grad u = W` (the skew matrix of
   `w`) has the rotation part `(grad u - grad u^T) / 2 = W`, so every material line of the body turns by `w`. A node of the set
   with six unknowns (translations `u_a` and small rotations `theta_a` about the global axes, ADR 0417 D1) moves with the body:
   `u_a = t + w x (x_a - x_R)` (the three rows of ADR 0296) **and `theta_a = w`** (three new rows, `theta_{a,c} - w_c = 0`,
   `c = 0, 1, 2`). The virtual work of the node's forces `f_a` and moments `m_a` on an admissible motion is
   `f_a . (dt + dw x d_a) + m_a . dw = dt . f_a + dw . (d_a x f_a + m_a)`, `d_a = x_a - x_R`: the transformation carries them to
   the reference point as a force `f_a` and the moment `d_a x f_a + m_a` (it is `T^T`, no new code). A beam root in the set
   therefore transmits its section force and moment to the solid face as a rigid plate would.
2. **Condensation of the beam (a1).** In the joined model the beam (any number of exact Timoshenko elements, ADR 0417
   arithmetic 2) is statically determinate: free at the tip, its root unknowns being the dependent `u_r = t + w x d_r`,
   `theta_r = w`. Minimising the energy over the beam's interior and tip unknowns at a fixed root motion gives the root motion's
   rigid displacement of the beam plus the cantilever solution (the rigid motions are in the null space of the element
   stiffness, ADR 0417 K2), and leaves for the solid and the reference unknowns the energy of the block minus the work of the
   root force `F` and moment `M_R = M_tip + (x_tip - x_R) x F` on `(t, w)` (for `x_R = x_r`, `d_r = 0`). That is exactly the
   solid-only problem of reading (a1) with the `ReferenceLoad (F, M_R)`: the two block solutions, and the reference motions, are
   the same vectors up to rounding, and the tip of the joined model is `t + w x (x_tip - x_R) + u_cant`,
   `w + theta_cant`. The check is therefore rounding-limited, not discretisation-limited (the block's mesh is the same in both).
   With the reference point elsewhere (`x_R'`), the rows of arithmetic 1 describe the same rigid face motion; the tip is the same
   up to rounding (check J3).
3. **The joined model (a1), data.** Block: `hex_box` origin `(0, -0.1, -0.1)`, lengths `(0.2, 0.2, 0.2)`, divisions
   `[3, 3, 3]` (64 nodes; the face centre `(0.2, 0, 0)` is not a block node), isotropic `E_b = 5e9 Pa`, `nu = 0.3`,
   `rho = 1200`; the face `xmin` clamped (components 0 to 2). Beam: root node at `(0.2, 0, 0)`, 4 Beam2 elements along `+x` to
   the tip `(1.2, 0, 0)` (`L = 1 m`), the section of ADR 0417 arithmetic 6 (`A = 0.012`, `I_y = 3e-6`, `I_z = 5e-6`,
   `J = 4e-6`, `kappa_y = 0.85`, `kappa_z = 0.75`), `E = 2.1e11`, `nu = 0.3`, `rho = 7850`, orientation `(0, 1, 0)` (so the local
   axes are the global ones: `e_1 = x`, `e_3 = e_1 x v / |..| = z`, `e_2 = y`). Rigid set "joint": the 16 nodes of `xmax` and
   the beam root, reference point `x_R = (0.2, 0, 0)`. Tip loads: force `F = (1200, -2500, 3700) N`, moment `(450, 0, 0) N m`.
   Cantilever closed form (ADR 0417 arithmetic 6, script): `u_cant = (4.762e-7, -7.967e-4, 1.963e-3) m`,
   `theta_cant = (1.393e-3, -2.937e-3, -1.190e-3)`. Root moment `M_R = (450, -3700, -2500) N m`. A rough estimate of the block's
   share by treating the block as a Timoshenko stub of length 0.2 (only to show the check sees the block; Hex8 is stiffer in
   bending than that, and the clamped face restrains warping): tip translation `(1.2e-6, -9.2e-4, 1.36e-3)`, 41 percent of the
   largest tip component; rotation share 29 percent (negative control N1).
   Rounding: the reduced matrix spans block entries `~ E_b h ~ 3e8` to the beam's bending stiffness `~ 3 E I_y / L^3 = 1.9e6`;
   condition below about `1e5` in the scale of the unknowns; the two direct solves agree to about `1e-11` relative. Limit
   **`1e-8`** (margin about 1000).
4. **The nodal spring on a rotation (a2).** Beam along `+x` from the origin, `L = 1 m`, 2 elements, the section and steel of 3.
   Root node: translations and `theta_x`, `theta_y` prescribed to zero, a nodal spring `k_theta = E I_z / L = 1.05e6 N m/rad` on
   `theta_z`. K1: moment `M = 300 N m` about `z` at the root: the beam turns rigidly (no strain), `theta_root = M / k_theta =
   2.857e-4`, the spring force on the body `-k_theta theta = -M`. K2: the same moment at the tip instead: the spring carries `M`
   (statics), `theta_root = M / k_theta`, `theta_tip = M / k_theta + M L / (E I_z) = 5.714e-4`, `u_y,tip = M L / k_theta +
   M L^2 / (2 E I_z) = 4.286e-4 m` (constant moment, no shear). Each element is exact; rounding only.
5. **Linear constraints on rotations (b3).** L1: the one-element cantilever of ADR 0417 (the seeded element, tip force and
   torque of arithmetic 6) with its six root constraints written as `Constraint::Linear` with a single term of coefficient 2
   and value 0, against the same model clamped by `Constraint::displacement` (the elimination path): every unknown equal within
   `1e-12` relative to the largest of each field (the F-393 precedent, measured `3e-16`). L2: the 2-element beam of 4 clamped
   at the root, `Linear theta_z(tip) = 0` (a guided tip), tip force `F_y = -2500 N`: the moment is `F_y (L / 2 - x)`, so
   `theta_z(L) = 0` and `u_y,tip = F_y (L^3 / (12 E I_z) + L / (kappa_y G A)) = -2.0145e-4 m` (bending integrated twice plus the
   constant shear strain); the constraint moment at the tip `rho = K x - f = -F_y L / 2 = 1250 N m`. L3: the same beam without
   load, `Linear theta_z(tip) = theta_0 = 1e-3`: a constant moment `E I_z theta_0 / L = 1050 N m`, `u_y,tip = theta_0 L / 2 =
   5e-4 m`, the constraint moment `1050 N m`.
6. **The beam held by a rigid set (a3).** Beam along `+x` from the origin, `L = 1 m`, 20 elements,
   `beam::rectangle_section(b = 0.01, h = 0.02, J = 4.58e-9, kappa = 5/6)` (`A = 2e-4`, `I_z = h b^3 / 12 = 1.667e-9`,
   `I_y = b h^3 / 12 = 6.667e-9`, `L / r_z = 346`, `L / r_y = 173`; `J` an input, about `0.229 b^3 h`), `E = 2e11`, `nu = 0.3`,
   `rho = 7850`, orientation `(0, 1, 0)`. The block: one Hex8 `hex_box` origin `(-0.1, -0.05, -0.05)`, lengths `(0.1, 0.1, 0.1)`,
   steel, every node prescribed to zero (components 0 to 2); rigid set "joint": its `xmax` face (4 nodes) and the beam root
   `(0, 0, 0)` (not a block node), reference point `(0, 0, 0)`. Elimination (ADR 0296): the prescribed rows make the face nodes
   zero; the face rows then read `t + w x d_a = 0` on four non-collinear points, which fixes `t = w = 0` (6 rows; the other 6 are
   dropped as redundant); the root rows give `u_r = 0`, `theta_r = 0`. The reduced matrices are the clamped beam's `K_FF`,
   `M_FF` (the block has no independent unknown), so the frequencies agree to the eigensolver's rounding. Modal options for
   both: `ModalOptions::new(6).tolerance(1e-12).residual_test(ResidualTest::Backward)` (as ADR 0417 amendment 1 for slender
   beams). Limit **`1e-8`** relative for each of the six lowest frequencies.
   Euler-Bernoulli cantilever (M2): `w(x)` with `w'''' = beta^4 w`, `w(0) = w'(0) = 0`, `w''(L) = w'''(L) = 0` gives
   `cos(lambda) cosh(lambda) = -1`, `lambda = beta L`, roots (bisection in the script and in the test) `1.87510, 4.69409, 7.85476,
   10.99554`; `f = lambda^2 / (2 pi L^2) sqrt(E I / (rho A))`. Lowest five over both planes: `8.1538 (y, n=1)`, `16.3076 (z, 1)`,
   `51.0990 (y, 2)`, `102.1981 (z, 2)`, `143.0788 (y, 3)` Hz; the next `280.38 (y, 4)`, `286.16 (z, 3)`; torsion `580 Hz`, axial
   `1262 Hz`. Allowance: the Timoshenko pinned-pinned quadratic of ADR 0417 arithmetic 5 expanded for small `(k r)^2` gives the
   relative frequency drop `(1/2) (k r)^2 (1 + E / (kappa G))` (it reproduces ADR 0417's `c_1 = 0.99199`: `8.1e-3` against
   `8.0e-3`); with `k = lambda_n / L` as an estimate for the cantilever: `6.0e-5, 2.4e-4, 3.8e-4, 1.5e-3, 1.1e-3` for the five.
   Discretisation with 20 elements (consistent mass, raises the frequency): below `9e-4 (7.85 / 9.42)^4 = 4.4e-4` (scaled from
   ADR 0417's 20-element pinned-pinned `n = 3`). Limit **`5e-3`** for each of the five (margin above 3).
7. **Free rigid motions with rotations (a4).** The scaled rigid field `j` (ADR 0399) is a translation `e_j` (`j < 3`, no
   rotation) or the rotation `e_{j-3} / l` about the centre `c` (`u = e_{j-3} x (x - c) / l`, rotation `e_{j-3} / l` at every
   point, arithmetic 1). So at a rotational unknown `(node, 3 + a)` the field's value is `0` for `j < 3` and `delta_{a, j-3} / l`
   for `j >= 3`. A rigid-set row `theta_{a,c} - w_c` with the reference moving with the body (`w_ref = e_{j-3} / l`, the existing
   code) is `delta / l - delta / l = 0` on every field: a rigid set still holds no rigid motion. Counts by hand: Z1 a free beam: 6.
   Z2 root translations prescribed and a spring on `theta_z` at the root: the motions with `u(root) = 0` are the three rotations
   about the root, the spring holds the one about `z`: 2 (with the rotational unknowns read as zero, as the code of ADR 0399 would
   if its refusal were only removed, the spring row vanishes: 3). Z3 root translations prescribed, `Linear theta_x(root) = 0`
   and `theta_y(root)` prescribed: 1 (zero-reading: 3). Z4 the joined model of 3 without the clamp: 6. Z5 the joined model of 3:
   0; the held beam of 6: 0. Z6: each of the six basis fields of Z1 times the beam stiffness, relative to `max |K| max |X c|`:
   rounding (ADR 0417 K2 measured `7.3e-17`); limit `1e-12`.

## Design

### D1. Rigid-set rows at six-unknown nodes (`mpc::transformation`)

For each node of a rigid set with six unknowns, after its three rows of ADR 0296, three rows `theta_c - w_c = 0` (designated
`theta_c`, `RowOrigin::RigidSet { component: 3 + c }`), in the order of the set. Nodes with three unknowns get exactly the rows
they got before, in the same order. `rigid_set_rows` (the harmonic transformation) is not changed: its paths refuse beams.

### D2. Entry checks of `solve_static_mpc` and `modal_analysis_mpc`

`check_supported` becomes `check_supported_with_beams` (ADR 0417 D4: a Beam2 block passes; `check_beam_model` keeps refusing
a node shared with a solid with `BeamSolidNode`, a block without its property and an element without a frame), followed by a
new refusal: a model with a Beam2 block and a `Constraint::Cyclic` gives `NotYetSupported { item: "constraint", variant:
"Cyclic", location: "constraint i, a model with Beam2", package: beam::FOLLOW_UP }` (the cyclic rows rotate translations only).
The preconditioned-CG refusal is unchanged. Every other user of the transformation (buckling, cyclic, geometric, harmonic,
transient, nonlinear, rotating) keeps its own `check_supported`, so it keeps refusing beams naming `beam::FOLLOW_UP`.

### D3. `free_motions`

The refusal of rotational unknowns is removed. `field_at` and `rigid_field` give the rotation of the scaled field at components
3 to 5 (arithmetic 7); the rigid-set rows of `free_rigid_motions` get the three rotation rows of D1 at a six-unknown node (with
`w_ref`). Prescribed, linear and support rows on rotational unknowns use the same `at(eq)` and need no change. The doc comment
names this ADR. The cyclic refusal stays.

### D4. What needs no change

Nodal springs on rotations (`support_triplets` and `add_supports` work per equation; the diagonal of a rotation equation is in
the beam's stiffness pattern), linear constraints and prescribed values on rotations (`dofs.equation`), reference loads,
reactions at rotational equations (moment reactions), `SolidResult` with six values at beam nodes.

### D5. Bits

A model without six-unknown nodes: D1 adds no row; D2's check is the same comparison (`check_elements(true)` differs from
`check_elements(false)` only on a Beam2 block; `check_beam_model` returns at once without one); D3 changes only values at
components above 2. So every solid-only result is the same bits (B1 to B3).

## Checks (limits fixed now)

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| J1 | a1 | joined model (arithmetic 3): tip translation against `t + w x (x_tip - x_R) + u_cant` from the solid-only solve, `max |diff| / max |u_tip|`; tip rotation against `w + theta_cant`, `max |diff| / max |theta_tip|` | `1e-8` each | arithmetic 2, 3: `~1e-11` |
| J2 | (stronger, a1) | the block's nodal displacements of the joined model against the solid-only solve, relative to their largest; the joined model's reference motion `(t, w)` against the solid-only one, relative to the largest of each | `1e-8` | arithmetic 2 |
| J3 | (a1) | the joined model with the reference point at the face corner `(0.2, 0.1, 0.1)` (a block node): the tip's six values against J1's joined model, relative to the largest of each field | `1e-8` | arithmetic 2 |
| J4 | (a1) | the reactions at `xmin` of the joined model balance the tip loads: `|sum R + F| / max |F|`; moments about the origin `|sum x_a x R_a + x_tip x F + M_tip| / (|x_tip| max |F|)` | `1e-10` | rounding |
| K1 | a2 | spring at the root, moment at the root (arithmetic 4): `|k_theta theta_root - M| / |M|`; the spring force `|S_theta + M| / |M|` (`MpcSolution::support_forces`) | `1e-10` each | exact, rounding |
| K2 | (stronger, a2) | moment at the tip: `|k_theta theta_root - M| / |M|`; `theta_tip`, `u_y,tip` against arithmetic 4, relative | `1e-10` each | exact element |
| L1 | b3 | the root clamp written as six `Linear` rows against `Constraint::displacement` (elimination path): every unknown, relative to the largest of each field | `1e-12` | F-393 precedent `3e-16` |
| L2 | b3 | guided tip by `Linear theta_z(tip) = 0`: `u_y,tip` and the constraint moment against arithmetic 5, relative | `1e-10` each | exact element |
| L3 | b3 | `Linear theta_z(tip) = theta_0`: `u_y,tip = theta_0 L / 2`, constraint moment `E I_z theta_0 / L`, relative | `1e-10` each | exact element |
| M1 | a3 | beam held by a rigid set on a fixed block (arithmetic 6): the six lowest frequencies against the clamped beam on the elimination path, `|f_i - f_i^c| / f_i^c` | `1e-8` each | the same reduced matrices |
| M2 | (stronger, a3) | the five lowest frequencies of the held beam against the Euler-Bernoulli cantilever values (both planes, sorted) | `5e-3` each | arithmetic 6: `<= 1.5e-3` |
| M3 | (a3) | the held beam: `rigid_free == false`; its `free_rigid_motions` count 0 | exact | arithmetic 6, 7 |
| Z1 | a4 | free beam (4 elements along `x`, no constraint): count 6 | exact | arithmetic 7 |
| Z2 | a4 | root translations prescribed, spring on `theta_z` at the root: count 2 | exact | arithmetic 7 (zero-reading: 3) |
| Z3 | a4 | root translations prescribed, `Linear theta_x(root) = 0`, `theta_y(root)` prescribed: count 1 | exact | arithmetic 7 (zero-reading: 3) |
| Z4 | a4 | the joined model without the clamp: count 6 | exact | arithmetic 7 |
| Z5 | a4 | the joined model: count 0 | exact | arithmetic 7 |
| Z6 | a4 | Z1's six free fields `X c` (from `FreeRigidMotions::fields`): `max |K X c| / (max |K| max |X c|)`, `K` the beam stiffness | `1e-12` | rounding (ADR 0417 K2 `7.3e-17`) |
| S2 | b2 | the box of `si_beam`'s S1 with the beam's second node on the box (a node shared with the block) and a rigid set on `xmax` added (the transformation path): `solve_static` and `modal_analysis` refused with `BeamSolidNode` naming the shared node | exact | D2 |
| R1 | (retired row) | the `si_beam` N2/N3 copy: the one-element model with the tip spring solves; `|sum R_root + S_tip + F| / |F|` (translations) | `1e-10` | rounding |
| B1 | b1 | `si_scaffold_bits.rs` passes unedited | pass | D5 |
| B2 | b1 | every existing `rcfd-fem` test passes, unedited except the retired row (ignored) whose copy passes | pass | D5 |
| B3 | b1 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | D5 |

Negative controls:

| id | control | expected | limit |
| --- | --- | --- | --- |
| N1 | the joined model's tip translation against the cantilever closed form alone (no block) | the block's share, estimated 41 percent | `> 0.1` relative |
| N3 | a beam model with a `Constraint::Cyclic` (a Hex8 box with its `ymin`/`ymax` surface sets and a separate beam) on `solve_static` and `modal_analysis` | `NotYetSupported { variant: "Cyclic", package: beam::FOLLOW_UP }` | exact |
| N4 | the joined model on paths that do not take beams: `nonlinear::initial_states`; `solve_static` with the preconditioned CG | `NotYetSupported` naming `Beam2` and `beam::FOLLOW_UP`; `InvalidValue { key: "solver" }` (unchanged) | exact |

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU1 | the rotation rows of D1 left out | J1 (root rotations free: singular or wrong), M1 |
| MU2 | rotation rows with `+ w` (`theta + w = 0`) | J1, J2 |
| MU3 | rotation row of component `c` tied to `w_{(c + 1) mod 3}` | J1 |
| MU4 | `free_motions`: rotation components of the fields read as 0 | Z2, Z3, Z6 |
| MU5 | `free_motions`: rigid-set rotation row with `+ w_ref` | Z4 |
| MU6 | `free_motions`: rotation value `l` instead of `1 / l` | Z6 |
| MU7 | `check_beam_model` not called on the transformation path | S2 |
| MU8 | the Cyclic refusal of D2 removed | N3 |

## Decision rule

- **done:** every check of the acceptance (J1, K1, M1, Z1 to Z5) and J2 to J4, K2, L1 to L3, M2, M3, Z6, S2, R1, B1 to B3 met,
  N1, N3, N4 as stated, every mutation caught by a registered check.
- **partial:** a clause missed (recorded as a `*_known_miss`) or not run, a mutation not caught, or a check met only after an
  amendment written after its run.
- **todo:** nothing of the path builds.

## Limits (not covered)

- L1. Beams on the cyclic, harmonic, transient, rotating, buckling, geometric-stiffness and nonlinear paths stay refused (each
  its own package); the harmonic transformation's rigid-set rows are not extended.
- L2. A rigid set ties every unknown of a six-unknown node (translations and rotations): a pinned joint (translations only) is a
  linear constraint written by the user, not a rigid-set option.
- L3. A beam node shared directly with a solid stays refused (`BeamSolidNode`); the joint is the rigid set.
- L4. Foundations (`ElasticSupport::Foundation`) act on solid faces only, as before.
- L5. The preconditioned CG stays refused on the transformation path (ADR 0296).

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. A six-unknown node of a rigid set takes the rotation of the reference point (D1), rather than its translations only:
   the rigid set is a rigid body (arithmetic 1), and a pin is available as linear rows. Recommended and taken.
2. A beam model with a cyclic constraint is refused on the transformation path (D2) rather than solved with translations-only
   cyclic rows: recommended (the rows would be wrong for rotations at a boundary node) and taken.

## Amendment 1 (2026-11-14, written before any run of a check)

Found on reading the test before its first run: `solve_static` and `modal_analysis` run `check_supported_with_beams` (and so
`check_beam_model`) before they dispatch to the transformation, so S2 as registered does not reach the entry check of D2, and
MU7 (the beam check left out of that entry check) could not be caught by it. S2 also calls `mpc::solve_static_mpc` and
`mpc::modal_analysis_mpc` directly on the same model, expecting the same `BeamSolidNode` naming the shared node (exact). No limit
changes; MU7 is expected to be caught by these two rows.
