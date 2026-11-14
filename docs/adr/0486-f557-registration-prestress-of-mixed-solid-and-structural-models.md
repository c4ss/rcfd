# ADR 0486: F-557 registration: the pre-stress of a static result on a model with both solid and structural elements (the refusal RF1 of ADR 0473 lifted)

Status: **REGISTERED 2026-11-14, written before any code of F-557 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-557 was written, compiled or run: the files listed under "What was read" were read
and the arithmetic below was done by hand (disclosed; no prototype, no probe run). The bit record B0 of the default paths is a
test file written and run on the base after this commit and before any library code (D5). Results go in the addenda at the
end; nothing above the addenda is changed after a run.

Roadmap item F-557 (deps F-420), a follow-up of ADR 0473 (F-420). Number 0486 given by the lead. Branch `f557-work` from
fb7c5d94, worktree `w49i-work/f557`, scratch `w49i-work/f557-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code, the crate graph unchanged; existing tests not edited, except that the test whose refusal this work retires is kept
as an ignored record and continued in an `_after_f557` copy (the pattern ADR 0473 D7 used); clean-room.

## What F-557 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The refusal RF1 of ADR 0473 is lifted: the geometric stiffness of a static result on a model with solid, beam and shell elements is assembled from each element's stress; a solid block carrying a beam or a shell strip in tension has the tension frequencies of ADR 0473 within 1 percent, and the buckling load of a shell plate stiffened by a solid rib equals that of the same model in solid elements within 3 percent; models without mixed elements keep their bits.

The brief adds: (b1) cover the rotating case, a solid hub carrying shell blades at speed through the F-396 rotating path, if
that path takes mixed models, else register that and propose a follow-up; (b2) find and register how solids, shells and beams
are joined; (b3) models without mixed elements keep their bits and `baseline_hashes` stays unchanged, with a default-bits test
at the base before any library change; (b4) probe a quantity before a `1e-12`-class limit, `1e-11` for sums of about `1e3`
terms; (b5) update the F-557 entries of `docs/ROADMAP.md` and `docs/roadmap.json` in a separate last commit.

Clauses: (a0) every limit registered here; (a1) RF1 lifted: `Prestress::from_static` takes a model with solid and structural
elements, and the geometric stiffness of that pre-stress is assembled from each element's own stress; (a2) a solid block
carrying a beam in tension: the tension frequencies `n = 1, 2` within `1e-2`; (a3) the same with a shell strip; (a4) a shell
plate stiffened by a solid rib: the lowest buckling factor against the same model in solid elements within `3e-2`; (a5) models
without mixed elements keep their bits.

Readings, registered:

- **(a1) "assembled from each element's stress":** each solid element of a mixed model receives at the points of its stiffness
  rule exactly the stress that `Prestress::from_static` gives it in a model of solid elements only (`D (eps - eps_th)` of
  `IntegrationPointField`), and each Beam2, Shell4 and Shell3 element exactly its stress of ADR 0473 D4; the assembled
  geometric stiffness is the sum of the elements' matrices of those stresses. Read as three exact (bit-equal) checks M1 to M3
  against the parts computed alone, and the negative controls N2, N3 (each part's stress matters).
- **(a2, a3) "the tension frequencies of ADR 0473":** the formula `w_n^2 = (n pi/L)^4 E I/(rho A) + (n pi/L)^2 T/(rho A)` of the
  pinned string-beam with `T = pi^2 E I/L^2`, on the beam of B3 and the strip of S1 of ADR 0473 with one end carried by a
  short solid block through which the tension flows (the block is the hinge and the anchor; arithmetic 1).
- **(a4) "the same model in solid elements":** the plate halves meshed in Hex20 (one element through the thickness, the same
  in-plane cells) and joined to the same Hex20 rib by a node-to-surface tie on coincident nodes (conforming), the same loads,
  supports and rib; the lowest positive factor of `buckling::factors` of each, `|lambda_mixed/lambda_solid - 1|`.
- **(b1)** The rotating path of F-396 (`RotatingPart::new`) takes the pre-stress from `Prestress::from_static` of a static solve
  of the unit centrifugal load with the model's constraints (`rotating.rs` lines 285 to 296), and its other steps already take
  beams and shells (ADR 0473 D6) and linear constraints (the transformation path, ADR 0398). The refusal RF1 is the only
  obstacle (read, not run); lifting it opens the path, so the rotating case is checked here (H1 to H3), not proposed as a
  follow-up. The whirl and Coriolis paths still refuse beams and shells (ADR 0473 Limits) and are not part of this work.

## How solids, shells and beams are joined (b2; read, registered)

- **Shared nodes are refused.** A node of a Beam2, Shell4 or Shell3 element has six unknowns (`DofMap::for_mesh`), a solid
  element's matrix has three per node, and the scatter refuses the size mismatch; ADR 0432 S1 records the refusal of a shell on
  the nodes of a Hex8 naming the node.
- **Joints are linear constraints.** `tie::beam_to_solid`, `tie::shell_to_solid` (ADR 0456 D3, the least-squares rigid motion of
  a set of solid nodes with unit weights) and `tie::beam_to_solid_with`, `tie::shell_to_solid_with` (ADR 0460 D1, the
  face-integrated `JointCoupling::Consistent` or `Rigid`) return `Constraint::Linear` rows in solved form, the structural node
  dependent; every analysis then takes the transformation path `x = T q` (ADR 0296, ADR 0398), which the static solve,
  `buckling::factors`, `prestressed_modes` and `RotatingPart` have.
- **The tests of this ADR use `JointCoupling::Consistent`** (`beam_to_solid_with` for the beam, `shell_to_solid_with` for the
  shells; the coupling ADR 0460 J1, J2 measured within `5.6e-4` on a continuing beam and shell) for every mixed model, and
  `tie::add_tie` with `TieMethod::NodeToSurface` on coincident nodes for the solid comparison models (the slave nodes then
  follow the master interpolation at their own position, which is the master node's value: a conforming joint).
- A mixed model whose parts are not joined (disjoint, each supported) is a mixed model too: the RF1 model of ADR 0473 (a Hex8
  and a Shell4 apart) is the case of the `_after_f557` copy (A1).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0473 in full (its D4 and RF1, the checks B3 and S1 with their measured `5.15e-5, 2.58e-4`
  and `9.84e-4, 5.12e-3`, P1 `3.30e-3`, E1 `5.92e-3`); ADR 0456 D3 and Limits, ADR 0460 D1, Limits and Addendum 1; ADR 0432
  (S1, L4).
- `crates/rcfd-fem/src`: `geometric_struct.rs` (the refusal, `prestress_from_displacement`), `geometric.rs` (`Prestress`,
  `assemble_geometric`, both paths of `prestressed_modes`), `recover.rs` (`IntegrationPointField::compute`), `rotating.rs`
  (`RotatingPart::new`, `with_unit_prestress`, `check_model`, `unit_centrifugal_model`), `buckling.rs` (the entry and the
  transformation path), `assemble.rs` (`element_dofs`, `scatter`), `model.rs` (`DofMap::for_mesh`, `check_elements`),
  `tie.rs` (module text, `assemble_parts`, the joint functions), `builders.rs` (`hex_box` and its sets), `hex20.rs`
  (`upgrade_to_hex20_mapped`, `traction_loads`), `mpc.rs` (`MpcSolution`, `solve_static_mpc`).
- `crates/rcfd-fem/tests`: `sa_geometric_struct.rs` (`rf1_mixed_model_prestress_refused`, the helpers), `sa_geometric_struct_bits.rs`
  (the digest), `sa_joint.rs` (the mixed set-ups).

## Arithmetic done before the code (disclosed)

1. **The block at the end of a string-beam (T1, T2).** The beam (strip) runs from `x = 0` to `L`; the block occupies `[-c, 0]`,
   joined at its face `x = 0`. It is held at the line of its joint face through the beam axis (lateral component zero: the
   hinge) and at the line of its far face through the axis (axial component zero), so the tension `T` applied at `x = L` flows
   through the beam into the block and out at the far face, and the block may rotate rigidly about the hinge. The beam end at
   the hinge is then pinned with two additions: (i) the block's own tension gives the rotation `theta` of the block the
   geometric energy `1/2 int sigma_xx (d w/d x)^2 dV = 1/2 theta^2 int sigma_xx dV = 1/2 T c theta^2` (the virial: `int sigma_xx
   dV = T c` for any equilibrium field of the block), an end spring `T c`; (ii) the block's rotary inertia. The Rayleigh
   quotient of the pinned mode `sin(k x)`, `k = n pi/L`, `theta = k`: the spring adds `T c k^2` to `(E I k^4 + T k^2) L/2`, a
   relative increase of `2 c T/(L (E I k^2 + T))`; with `T = pi^2 E I/L^2` this is `c/L` for `n = 1` and `2 c/(5 L)` for `n =
   2` (an upper bound: the mode adapts). `c = 0.0025`, `L = 1`: `+2.5e-3` and `+1.0e-3`. Rotary inertia of the block about the
   hinge, `rho c h_y h_z (c^2/3 + h^2/12)`: beam (`h = 0.01` square block, beam `b = 0.005`): `8.3e-12 rho`, against the beam's
   modal mass `rho A L/2 = 1.25e-5 rho`, times `k^2`: `6.6e-6` (`n = 1`), `2.6e-5` (`n = 2`), lowering. Strip (block `c x b x
   h = 0.0025 x 0.05 x 0.01`, strip `t = 0.005`): `1e-6`. The discretisation errors are ADR 0473's measured B3 `5.15e-5,
   2.58e-4` and S1 `9.84e-4, 5.12e-3` (S2 `8.58e-4, 4.31e-3`; their signs are not recorded there; taken as adding). The joint
   (consistent coupling of a beam node to a face, of a shell edge to a face) adds the flexibility of the face, of the relative
   size of ADR 0460's J1 and J2 (`5e-6`, `5.6e-4` in static deflection). Estimates: T1 `n = 1, 2`: `<= 3e-3`; T2 (Shell4) `<=
   4e-3`, `<= 7e-3`; T2b (Shell3) `<= 4e-3`, `<= 6e-3`. Limit `1e-2` (the acceptance's).
2. **The stiffened plate (T3).** `a = 1`, `t = 0.02`, `nu = 0` (the joint of ADR 0460 is checked at `nu = 0` only, and with
   `nu = 0` and equal tractions on the whole end section the pre-stress is exactly uniform, check T3u), `E = 2e11`. Rib along
   `x` at the plate's centre line: width `w_r = 0.0625`, from `z = +t/2` down to `z = -t/2 - h_r`, `h_r = 0.04`; the plate is
   two halves `y in [0, 0.46875]` and `[0.53125, 1]` whose inner edges continue into the rib's side faces over its top layer
   `z in [-t/2, t/2]` (no lever). Rayleigh estimate with `w = sin(pi x/a) sin(pi y/a)` (rib on the crest): plate `D pi^4/(2
   a^2)`, rib `E I_r pi^4/(4 a^3)` with `I_r = 2.6e-6 m^4` about the plate's mid-plane (`1.125e-6` own plus `A_r d^2 = 3.75e-3 x
   0.02^2`; an upper value: the composite neutral axis lies between), geometric `sigma (t pi^2/8 + A_r pi^2/(4 a))`: the
   overall mode at `sigma ~ pi^2 E 2.86e-4`; the local mode of the half panels (simply supported, `k = 4` on `b' = 0.469`) at
   `sigma ~ pi^2 E 6.07e-4`, twice as high: the overall mode governs. Without the rib's pre-stress the overall estimate rises by
   `0.375` (the rib's share of the geometric work, `2 A_r/(a t)`): N2. Differences between the models: the plate as Shell4 (MITC4
   16 x 16 cells, P1 of ADR 0473 `3.3e-3` of the closed form at `t/a = 0.01`) against Hex20 with one element through the
   thickness (aspect `3.1`; the twisted strip E1 of ADR 0473 matched Shell4 against Hex20 within `5.9e-3` in frequency, `1.2e-2`
   in `omega^2`), the joint (consistent edge coupling against conforming nodes; J2 of ADR 0460 `5.6e-4`), Mindlin against 3-D at
   `t/a = 0.02` (`(1 + nu) t^2/(6 kappa) pi^2 ... ~ 2e-3`). Estimate `<= 1.5e-2`; limit `3e-2` (the acceptance's).
3. **The rotating hub (H1 to H3).** Axis `z` through the origin. Hub: Hex20 block `x in [0.02, 0.05]`, `y in [-0.025, 0.025]`,
   `z in [-0.005, 0.005]` (2 x 12 x 3 cells, the middle layer `z in [-t/2, t/2]`), clamped at `x = 0.02`. Blade: flat plate in
   the `x-y` plane, `x in [0.05, 0.20]` (span `L = 0.15`), chord `0.05`, `t = 0.001`, steel `nu = 0.3`; Shell4 36 (span) x 12
   (chord) joined to the hub's middle-layer face at `x = 0.05`, against the Hex20 blade 36 x 12 x 1 tied to the same faces on
   coincident nodes. The hub's bending compliance relative to the blade's root, `(0.03/(E 0.05 0.01^3/12))/(0.15/(E 0.05
   0.001^3/12)) = 2e-4`: the root is clamped. Rest estimates as ADR 0473 arithmetic 7 for the flat strip: `1F = 238.7` rad/s,
   `1T ~ 1312`, `2F ~ 1496`, `2T ~ 3936`, `3F ~ 4188` (six modes computed, tracks 1 to 4 compared). Speeds `0, 120, 240, 360`
   rad/s; the flap stiffening with the hub offset `R/L = 1/3` is of order `Omega^2` (at 360 rad/s the stiffening term of
   `omega_1F^2` is about three times the rest term). Expected differences: those of E1 (`5.9e-3`) and the joint (`5.6e-4`):
   estimate H1 `<= 1e-2` against the limit `2e-2` (E1's); the stiffening increment of track 1, `omega^2(360) - omega^2(0)`,
   compares the pre-stress and `K_sigma` of both models directly: estimate `<= 1e-2`, limit `3e-2`.
4. **Exactness of M1 to M3.** In a mixed model the solid nodes have three unknowns and the structural nodes six, numbered in
   node order; a part's elements see the same coordinates, materials and displacement values as in the part alone (the
   displacement slice of its unknowns), and the per-element arithmetic of D1 is the arithmetic of the existing paths (same
   functions, same order), so the stresses are bit-equal (M1, M2). The assembled matrix of a model whose parts share no node
   has, at each entry, the contributions of one part only, added in element order: bit-equal to the part's matrix at the
   shifted indices, and zero across (M3). No probe is needed for an equality of this kind; nothing `1e-12`-class is registered.
5. **Uniform pre-stress of T3 (T3u).** With `nu = 0`, the same `E` everywhere and the uniform end traction `sigma_ref` on the
   whole end section (the shell edges by their consistent nodal forces `N h`, `N h/2`), the field `sigma_xx = -sigma_ref`,
   `u_x = -sigma_ref x/E` is an exact solution of both models (the joint rows hold it exactly: the face motion is the edge
   reference motion). The static solve of the transformation path reproduces it to its rounding (`~ 1e-13` relative on the
   direct solver, conditioning of `T^T K T` with the joint rows unknown): limit `1e-8` on `max |sigma_ij/sigma_ref + delta_i1
   delta_j1|` over every point of both models, no probe.

## Design

### D1. The pre-stress of a mixed model (`geometric_struct.rs`, `recover.rs`)

`geometric_struct::prestress_from_displacement` no longer refuses a solid block. When the model has a non-empty solid block
it computes `recover::IntegrationPointField::compute_solids(model, displacement)` (new, `pub(crate)`): the loop of
`IntegrationPointField::compute` with the structural elements given no points; `compute` becomes a call of the shared body
with the flag off, the arithmetic of every element unchanged. Each solid element then takes its points' stresses from that
field (the offsets of `Prestress` count the element's stiffness rule, as `check_fits` expects), each structural element its one
point of ADR 0473 D4 as before. A model of structural elements only does not compute the field: its path is the former one,
bit for bit. A model of solid elements only does not reach this function (`Prestress::from_static` keeps its former path).
The constant `FOLLOW_UP` stays (other refusals may name it); the module and function documentation state the new reach.

### D2. A masked pre-stress (`geometric.rs`)

`Prestress::masked(&self, keep: impl Fn(usize) -> bool) -> Prestress`: the same layout with the stresses of the elements not
kept set to zero (for the negative controls N2, N3: one part's stress alone). Public, additive; nothing else in `geometric.rs`
changes.

### D3. The paths

Unchanged: `assemble_geometric` already dispatches per element (ADR 0473 D6), `buckling::factors`, `prestressed_modes` and
`RotatingPart` already take beams, shells and linear constraints. The only refusal lifted is RF1.

### D4. Records retired

`sa_geometric_struct.rs::rf1_mixed_model_prestress_refused` checks a refusal this work retires: it becomes an ignored record
(`#[ignore = "retired by ADR 0486 (F-557): the mixed pre-stress is taken"]`, body unchanged) and continues as
`a1_rf1_model_prestress_after_f557` in the new test file, which checks on the same model that the pre-stress is taken and that
its Hex8 part has the stress of the Hex8 alone and its Shell4 part that of the Shell4 alone (bit-equal).

### D5. The bit record of the default (B0)

`crates/rcfd-fem/tests/sa_prestress_mixed_bits.rs` uses only the API of the base fb7c5d94 and is run once on the base (this ADR
committed, no library line changed) to print its digests (FNV-1a 64 of the `to_bits`, the digest of ADR 0291), which become
its constants; after the change it runs unchanged and compares. Models without mixed elements on the paths this work
touches: `Prestress::from_static` of a beam (B3 of ADR 0473 shortened), a Shell4 and a Shell3 plate under edge load (structural
only, the function D1 changes), of a Hex20 cantilever, of a Hex8 block with a temperature field (the thermal branch of the
refactored `compute`) and of a Tet10 block (solid only); `IntegrationPointField::from_result` of the Hex8 thermal block
(positions, weights, strain, stress, thermal strain); `buckling::factors` of the Shell4 plate; a static solve of a mixed model
with a joint (unchanged path, for reference). The existing `sa_geometric_struct_bits.rs` (B1 of ADR 0473) and
`rcfd-api`'s `baseline_hashes` run unchanged.

### D6. Names in the topic documentation

`docs/reference-data.md` gains the F-557 section (the closed forms are those of ADR 0473's section; the end-spring and
Rayleigh arithmetic of this ADR is first principles). No case-file key is added.

## Checks (limits fixed now)

Common: `E = 2e11`, `rho = 7850`, eigen-solves with `ResidualTest::Backward`, tolerance `1e-12` (T1 to T3) and `1e-10` (H1 to
H3), `JointCoupling::Consistent` for every joint. Every value is printed. Test file `crates/rcfd-fem/tests/sa_prestress_mixed.rs`.

| Id | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| A1 | a1 | the RF1 model of ADR 0473 (a Hex8 and a Shell4 apart) with a non-zero displacement: `Prestress::from_static` taken, `check_fits` holds, its Hex8 points bit-equal to `Prestress::from_static` of the Hex8 alone and its Shell4 point to that of the Shell4 alone (`_after_f557` copy, D4) | holds, equal | equal |
| T1 | a2 | beam of B3 (`L = 1`, `b = 0.005`, `nu = 0.3`, 16 Beam2) in the `x-y` plane carried at `x = 0` by a Hex20 block `[-0.0025, 0] x [-0.005, 0.005]^2` (1 x 2 x 2 cells, `nu = 0`) through `beam_to_solid_with`; block: `u_z = 0` at every node, `u_y = 0` on its joint-face nodes with `y = 0`, `u_x = 0` on its far-face nodes with `y = 0`; beam: `u_z = theta_x = theta_y = 0` at every node but the joint node, `u_y = 0` at `x = L`; end force `T = pi^2 E I/L^2` along `+x` at `x = L`; static solve, pre-stress from it, pre-stressed modes (4): `|omega_n^2/formula - 1|`, `n = 1, 2` (the lowest modes with the largest kinetic share in `u_y`) | `<= 1e-2` each | `<= 3e-3` |
| T2 | a3 | strip of S1 (`L = 1`, `b = 0.05`, `t = 0.005`, `nu = 0`, Shell4 40 x 2 on `[0, 1] x [0, 0.05]`) carried at `x = 0` by a Hex20 block `[-0.0025, 0] x [0, 0.05] x [-0.005, 0.005]` (1 x 2 x 2 cells, `nu = 0`) through `shell_to_solid_with` of the edge `x = 0`; block: `u_z = 0` on its joint-face nodes with `z = 0`, `u_x = 0` on its far-face nodes with `z = 0`, `u_y = 0` at its far-face node `(−0.0025, 0, 0)`; strip: `u_z = theta_x = 0` on the edge `x = L`; the end force `T` shared `T/4, T/2, T/4` on `x = L`; pre-stressed modes (6): `n = 1, 2` of the modes with share in `u_z > 0.5` | `<= 1e-2` each | `<= 4e-3`, `<= 7e-3` |
| T2b | supp. | T2 with Shell3 40 x 2 x 2 | `<= 1e-2` each | `<= 4e-3`, `<= 6e-3` |
| M1 | a1 | in T1 and T2 after the static solve: the pre-stress of every block element bit-equal to `Prestress::from_static` of the block alone with its displacement slice | equal | equal |
| M2 | a1 | the same for every beam (T1) and shell (T2) element against the structural part alone | equal | equal |
| M3 | a1 | `geometric_stiffness` of the mixed pre-stress of T1 and T2: every stored entry bit-equal to the part's entry at the shifted indices (zero across the parts), every entry of each part found | equal | equal |
| T3 | a4 | the stiffened plate of arithmetic 2: plate halves Shell4 16 (along `x`) x 8 (across) each, rib Hex20 16 x 1 x 3 (`z` layers of `0.02`, the top one the plate's thickness), the halves' inner edges joined to the rib's top-layer side faces; against the halves in Hex20 16 x 8 x 1 tied node-to-surface (slave the halves' inner faces, master the rib's top-layer side faces); both: `u_z = 0` on the outer edges of the plate (shell edge nodes, or every node of the Hex20 outer faces) except the joint/slave nodes, `u_z = 0` on every node of the rib's end faces, `u_x = u_y = 0` at the mid-surface node `(0, 0, 0)`, `u_y = 0` at `(a, 0, 0)`; end loads: the uniform compressive stress `sigma_ref = 1e6` Pa on the whole end sections at `x = 0` and `x = a` (shell edges by `N h`, `N h/2` with `N = sigma_ref t`; Hex20 faces by `hex20::traction_loads`); static solve, pre-stress, `buckling::factors` (2): `|lambda_mixed/lambda_solid - 1|` of the lowest | `<= 3e-2` | `<= 1.5e-2` |
| T3u | supp. | in T3, both models: `max |sigma_ij/sigma_ref + delta_i1 delta_j1|` over every pre-stress point | `<= 1e-8` | rounding |
| T3m | supp. | in T3: the lowest mode is the overall one (the rib's lateral `u_z` at mid-span, `|u_z(a/2, a/2, -t/2 - h_r)|`, at least half the largest `|u_z|` of the plate) in both models | holds | holds |
| H1 | b1 | the hub and blade of arithmetic 3, mixed (Shell4 blade) against all-Hex20, `RotatingPart::campbell` with 6 modes at `0, 120, 240, 360` rad/s: `max |omega_mixed/omega_solid - 1|` over tracks 1 to 4 and the four speeds | `<= 2e-2` | `<= 1e-2` |
| H2 | b1 | the stiffening increment of track 1: `|(omega^2(360) - omega^2(0))_mixed/(omega^2(360) - omega^2(0))_solid - 1|` | `<= 3e-2` | `<= 1e-2` |
| H3 | b1 | in H1, both models: no loss of stiffness, every MAC of tracks 1 to 4 with their previous mode `> 0.9` | each | `~1` |
| B0 | a5 | every digest of `sa_prestress_mixed_bits.rs` equal to the base's | equal | equal |
| B1b | a5 | `sa_geometric_struct_bits.rs` (ADR 0473) passes unchanged; `rcfd-api` `baseline_hashes` passes unchanged | passes | passes |

Negative controls:

| Id | Quantity | Expected |
|---|---|---|
| N1 | T1 and T2 with a zero pre-stress (`Prestress::uniform(model, 0)`) on the same path: `|omega_1^2/formula - 1|` | `> 5e-2` each (`0.5`) |
| N2 | T3 mixed with the rib's pre-stress masked (`Prestress::masked`, shells kept): `|lambda/lambda_solid - 1|` | `> 3e-2` (`~0.3`) |
| N3 | T3 mixed with the shells' pre-stress masked (rib kept): `|lambda/lambda_solid - 1|` | `> 3e-2` (`> 1`) |
| N4 | H1 mixed with `stress_stiffening(false)` at 360 rad/s, track 1: `|omega_mixed/omega_solid - 1|` | `> 5e-2` |

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the solid elements of a mixed model given a zero stress (the field not used) | M1, T3, N2 |
| MU2 | the solid elements' points taken from the wrong offset (each solid element given the stresses of the previous one, the first its own) | M1, T3u |
| MU3 | `compute_solids` gives the structural elements one dummy point (offsets shifted for the elements after them) | A1 (`check_fits`), M1 |
| MU4 | the RF1 refusal restored | A1, T1 to T3, H1 |
| MU5 | the structural elements of a mixed model given a zero stress | M2, T1, T2, T3, H1 |
| MU6 | `compute` (the default) given the solids-only flag on | B0, A1 |

## Decision rule

- **done:** every check above met (A1, T1, T2, T2b, M1 to M3, T3, T3u, T3m, H1 to H3, B0, B1b), every negative control as
  expected, the existing tests of `rcfd-fem` passing (with the record of D4), fmt and clippy clean.
- **partial:** a clause of the acceptance missed or not run, with the miss recorded as a `*_known_miss` test and diagnosed; or a
  supplementary check (T2b, T3u, T3m, H1 to H3) missed.
- **todo:** the mixed pre-stress refused or not reaching the buckling path.

## Limits (not covered)

- The joints are those of ADR 0456 and ADR 0460, at `nu = 0` in T1 to T3 (the joint's own registered range) and `nu = 0.3` in
  H1 to H3 (the face contraction of the consistent coupling at `nu > 0` is part of the measured difference); no shared nodes.
- The pre-stress of each element is that of its own kind (ADR 0473 Limits: constant per structural element, membrane and axial
  only); a joint row carries no geometric stiffness of its own (the rows are linear constraints, the second-order terms of a
  rigid lever under a pre-load are not modelled).
- The whirl and Coriolis analyses, the cyclic sector and the transient path with beams or shells are not part of this work; a
  mixed model with an orthotropic or tabulated solid material or a temperature field is taken by the same code (the field of
  `compute`) but no check here exercises it.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

- **Q1. Where the solid stresses come from.** Options: (a) the shared body of `IntegrationPointField::compute` with the
  structural elements skipped (one arithmetic for both); (b) a copy of the solid loop in `geometric_struct.rs`. Taken (a): no
  duplicated arithmetic, and B0 records that the default `compute` keeps its bits.
- **Q2. A public `Prestress::masked`.** Options: (a) add it (additive, it lets a caller isolate a part's stiffening; used by N2,
  N3); (b) build the masked pre-stress inside the tests from a field. Taken (a).
- **Q3. A case-file key.** None added (ADR 0473 Q1).
