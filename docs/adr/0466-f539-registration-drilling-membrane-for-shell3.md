# ADR 0466: F-539 registration: a drilling-unknown membrane for Shell3 (the ANDES optimal membrane triangle, with Allman's 1984 triangle beside it), opt-in through the Shell block property

Status: **REGISTERED 2026-11-14, written before any code of F-539 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-539 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with a plain numpy prototype in the scratch folder
(`w49i-work/f539-scratch/proto.py` (Allman's membrane, plane stress), `andes.py` (the ANDES membrane as recalled), `shell18.py`
(the full 18-unknown element: a verbatim copy `proto419.py` of the F-419 prototype `f419-scratch/proto.py`, which matched rcfd's
Shell3 to `2.2e-9` on every check of ADR 0454, with its membrane replaced), `est1.py` to `est11.py`, logs `est*.log`; dense
matrices, no rcfd code; disclosed, its numbers quoted below). The bit record B1 of the default paths is a test file written and
run on the base after this commit and before any library code (D5). Results go in the addenda at the end; nothing above the
addenda is changed after a run.

Follow-up of F-419 ([ADR 0454](0454-f419-registration-three-node-flat-shell.md): limit L2 "the CST membrane is stiff in in-plane
bending; a drilling-unknown (Allman) membrane is a follow-up"). Roadmap item F-539 (deps F-419). Number 0466 given by the lead.
Branch `f539-work` from main d0439898, worktree `w49i-work/f539`, scratch `w49i-work/f539-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; no existing test edited (new tests in new files); clean-room.

## What F-539 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. In-plane bending of a cantilever strip within 2 % of beam theory on a 8 x 2 mesh, patch tests kept.

The note: "a drilling-unknown membrane". The brief adds: (b1) the Allman 1984 triangle or the Bergan-Felippa / ANDES optimal
membrane, cited from memory and marked NOT read in `docs/reference-data.md`; (b2) the discretisation-error estimate of the 8 x 2
strip registered, with its diagonal pattern, before the run, not tuned; (b3) the spurious zero-energy mode of Allman's element
(equal drilling rotations) noted, and its suppression registered; (b4) a one-ulp probe of the set-up before any `1e-12`-class
limit, disclosed; (b5) opt-in, a new membrane choice on Shell3, default Shell3 bits unchanged (digests on the base before code);
(b6) shared-file edits minimal (another agent works in `rcfd-fem`).

Clauses: (a0) every limit registered here; (a1) in-plane bending of a cantilever strip on an 8 x 2 mesh within 2 percent of beam
theory; (a2) patch tests kept.

Readings, registered:

- **(a1) "8 x 2 mesh":** 8 x 2 square cells, each cut in two (32 triangles), the diagonal pattern stated per check (CT1 is the
  registered one: every cell cut along `(i, j)-(i + 1, j + 1)`). **"Cantilever strip":** `L = 1`, depth `b = 0.25` (square cells),
  `t = 0.01`, steel `E = 2e11`, `nu = 0` (as F-419's cantilever), in the `x, y` plane, clamped at `x = 0`, loaded at `x = L` by an
  in-plane shear `P = 1 N` along `+y`. **"Beam theory":** at `L / b = 4` the shear deflection is `3 E I / (kappa G A L^2) = 3.75`
  percent of the bending one, so beam theory is read as the Timoshenko beam `delta = P L^3 / (3 E I) + P L / (kappa G A)`,
  `kappa = 5/6` (the shear factor of the rectangle used by Shell4 and Shell3, `shell::SHEAR_FACTOR`), `I = t b^3 / 12`, `A = b t`,
  `G = E / 2`; the deflection measured is `u_y` of the tip node on the axis (`x = L`, `y = b / 2`). The Euler-Bernoulli reading is
  covered by CT4 (an end couple, no shear) and printed for CT1 (O2). Arithmetic 6 shows the clamped two-dimensional strip lies
  within `1.5e-4` of this beam theory, so the reference is not the source of more than that.
- **(a2) "patch tests kept":** the patch tests of F-419 (ADR 0454 clauses a2 to a4: the membrane and the constant-curvature bending
  patch on the irregular flat mesh of Shell3 and on the mesh mixing Shell3 and Shell4, within `1e-8`): (k1) for the default Shell3,
  unchanged (the existing test passes unedited, B2); (k2) with the new membrane, on the irregular Shell3 mesh (PT1 to PT4); (k3)
  with the new membrane on the mixed mesh (PT5, PT6). Arithmetic 8 derives that the membrane patch of (k3) **cannot** hold for a
  drilling membrane of this family next to a Shell4 (or a constant-strain Shell3) element: PT5 is registered at the acceptance's
  `1e-8` all the same, predicted to miss (prototype analogue `2.05e-1`), and the model refuses such a mesh (RF1, D3).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0454 (whole: arithmetic 1 to 18, the checks, limit L2, the mutations); ADR 0461 (whole: the
  style, the B1 pattern, the opt-in pattern).
- `crates/rcfd-fem/src/shell3.rs` (whole), `shell.rs` (`homogeneous_section`, `moduli`, `section_of`, `model_section`,
  `shell_thickness`, `thickness_or_error`, `check_shell_model`, `shell3_stresses`), `mesh.rs` (`BlockProperty`,
  `BlockProperty::shell`, `set_block_property`), `thermal.rs` (the Shell3 thermal call), `assemble.rs` (`assemble_with`,
  `stiffness_with`, the Shell3 dispatch), `constraint.rs` (`prescribed_equations`, `reduce`); the `grep` of `BlockProperty` over
  `crates`: in `src` only `rcfd-fem` (no serialisation, no hash, no golden; `rcfd-post` tests build it with `BlockProperty::shell`).
- Tests: `crates/rcfd-fem/tests/sa_shell3.rs` (the helpers, the patch model, the cantilever, the free plate, the single element),
  `sa_shell3_bits.rs` (the digest pattern).
- `f419-scratch/proto.py` (copied verbatim as `proto419.py`).
- **Not read:** any finite-element code, manual or documentation outside this repository, and no paper. Cited from memory (marked
  NOT read in `docs/reference-data.md`): D. J. Allman, "A compatible triangular element including vertex rotations for plane
  elasticity analysis", Comput. Struct. 19 (1984) 1-8 (the edge-normal displacement quadratic, driven by the difference of the
  vertex rotations; the spurious equal-rotation mode); P. G. Bergan and C. A. Felippa, "A triangular membrane element with
  rotational degrees of freedom", Comput. Methods Appl. Mech. Engrg. 50 (1985) 25-69 (the free formulation, the basic and
  higher-order stiffness, `alpha_b = 3/2`); C. A. Felippa and C. Militello, "Membrane triangles with corner drilling freedoms. II.
  The ANDES element", Finite Elem. Anal. Des. 12 (1992) 189-201; C. A. Felippa, "A study of optimal membrane triangles with
  drilling freedoms", Comput. Methods Appl. Mech. Engrg. 192 (2003) 2125-2168 (the element OPT: `alpha_b = 3/2`, `beta_0 = max((1
  - 4 nu^2) / 2, 0.01)`, `beta_1 .. beta_9 = 1, 2, 1, 0, 1, -1, -1, -1, -2`, the higher-order stiffness `(3/4) beta_0 t A sum Q^T
  E_nat Q` over the mid-sides). The basic stiffness and the edge field are derived below; the higher-order stiffness is taken as
  recalled and **verified** against the property OPT was designed for, exact in-plane pure bending of a rectangle cut into two
  triangles for any aspect ratio (arithmetic 4), which a misremembered `beta`, factor or permutation does not give (the factor `A /
  3` in place of `3 A / 4` gives `0.8611`). The numbers taken from the sources are `alpha_b`, `beta_0` and `beta_1 .. beta_9`, taken
  once and not varied.

## Arithmetic done before the code (disclosed)

Local axes of the facet (ADR 0454 arithmetic 1, counter-clockwise, area `A > 0`), membrane unknowns `(u_i, v_i, theta_i)`,
`theta = theta_z` positive about `e_3`; `omega = (v_X - u_Y) / 2` (the convention of Shell3's drilling penalty).

1. **Allman's edge field (derived).** On edge `e` from node `i = e` to `j = e + 1`, tangent `s`, length `l`, outward normal `n = (s_Y,
   -s_X)`: a rigid rotation `u = theta (-Y, X)` has `u . n = -theta (x . s)`, so `d(u_n) / ds = -theta`. Allman's field keeps the
   linear interpolation and adds a quadratic normal displacement on each edge whose end slopes are `-theta_i`, `-theta_j`: for `q(s)
   = a (1 - s / l) + b s / l + c 4 s (l - s) / l^2`, `q'(0) - q'(l) = 8 c / l`, so `c = l (theta_j - theta_i) / 8`, the mid-edge
   excess along `n`. In the triangle: `u = sum_i N_i u_i + sum_e 4 N_i N_j c_e n_e = ... + sum_e (N_i N_j / 2)(theta_j - theta_i)(d_Y,
   -d_X)`, `d = x_j - x_i`. The field is conforming (two elements sharing an edge give it the same displacement), its strain is
   linear, its energy and `int (theta - omega)^2` are quadratic: the three mid-edge points are exact. Prototype (`est1.log`): rigid
   motions `|K r| <= 1.6e-16`; a linear field with `theta_i = omega` gives the exact strain at every point.
2. **The spurious mode of Allman's element (b3).** `u_i = v_i = 0`, `theta_1 = theta_2 = theta_3 = c`: every `c_e = 0`, the field is
   zero, so the strain energy is zero: a fourth zero of the 9 x 9 membrane (prototype, `est1.log`: four eigenvalues below
   `2.2e-16` of the largest without a penalty), and in an assembled mesh one global mode (all rotations equal, no translation;
   a 4 x 4 free plate has 7 zero eigenvalues instead of 6, `est11.log`). **Suppression, registered:** for the Allman membrane,
   Shell3's drilling penalty `alpha_d G t int (theta - omega)^2 dA` (`shell::DRILLING_FACTOR = 1e-3`, the mid-edge rule, `omega`
   now of Allman's field) stays: on the mode `omega = 0`, `theta = c`, the energy is `alpha_d G t A c^2 > 0` (prototype: 3 zeros of
   the membrane, the fourth at `1.5e-4`; the plate 6). For the ANDES membrane the higher-order stiffness acts on the deviatoric
   rotations `theta_i - omega_0` (`omega_0` the rotation of the linear field) through a positive definite 3 x 3 matrix; the mode has
   `theta_i - omega_0 = c` and is stiff (arithmetic 5); no penalty is added (the control with `beta_0 = 0` has 9 zeros of 18).
3. **Allman's element alone misses (a1).** The prototype of the strip of (a1), 8 x 2, the clamp of arithmetic 6: `u_y / delta =
   0.9156777414` on either single diagonal (`est2.log`; CST `0.5428975996`; convergence `0.9724, 0.9912, 0.9974` on 16 x 4, 32 x 8,
   64 x 16). 8.4 percent short: the acceptance cannot be met by Allman's element on this mesh, so the element of F-539 is the
   optimal membrane; Allman's stays as an option and a control (N2). An Allman-type family with the basic and higher-order parts
   scaled freely gives anything from `0.92` to `1.12` (`est3.py`): its factors are not chosen here (that would be tuning).
4. **The ANDES optimal membrane (OPT).** `K = K_b + K_h`.
   *Basic stiffness (derived):* a constant stress `sigma` does the boundary work `t sum_e int (u . sigma n) ds` on the edge field of
   arithmetic 1 with the edge excess scaled by `alpha_b`; since the field's volume integral of strain is its boundary integral,
   this is `A t Bbar(alpha_b)^T sigma` with `Bbar` the mean of Allman's strain over the element and its rotation columns times
   `alpha_b`: `K_b = A Bbar(alpha_b)^T A_m Bbar(alpha_b)` (`A_m` the membrane matrix of the section), **`alpha_b = 3/2`**. Per edge
   the rotation columns contribute `(l^2 / 12)(theta_j - theta_i) n (x) n` to `A Bbar` (`int 4 N_i N_j ds = 2 l / 3`).
   *Higher-order stiffness (recalled):* `K_h = T^T K_theta T`, `T` the 3 x 9 rows `theta_i - omega_0`; natural strains along the
   sides `21, 32, 13` (nodes `1, 2, 3` = local `0, 1, 2`) at the corners `Q_1 theta~`, `Q_2 theta~`, `Q_3 theta~` with `Q_1 = (2 A / 3)
   [[b1, b2, b3] / l21^2, [b4, b5, b6] / l32^2, [b7, b8, b9] / l13^2]`, `Q_2`, `Q_3` its cyclic images (rows `[b9, b7, b8], [b3, b1,
   b2], [b6, b4, b5]` and `[b5, b6, b4], [b8, b9, b7], [b2, b3, b1]`); at the mid-sides `Q_4 = (Q_1 + Q_2) / 2`, `Q_5 = (Q_2 + Q_3) /
   2`, `Q_6 = (Q_3 + Q_1) / 2`; `E_nat = T_e^T A_m T_e`, `T_e` the inverse of the 3 x 3 map from Cartesian strain to the side
   strains `s^T eps s` (derived, not recalled); **`K_theta = (3/4) beta_0 A sum_{4,5,6} Q^T E_nat Q`**, **`beta_0 = max((1 - 4
   nu^2) / 2, 0.01)`**, `nu = A_m[0][1] / A_m[0][0]` (the Poisson ratio of the isotropic section). Verification (`est4.log`,
   `est5.log`): a rectangle `a x b` in two triangles under the exact pure bending field (`u = -k x y`, `v = k (x^2 + nu y^2) / 2`,
   `theta = omega = k x`, and the same about the other axis) has the energy ratio element / exact `1.000000000` for `a / b = 0.25,
   0.5, 1, 2, 4`, both diagonals, both directions, `nu = 0, 0.2, 0.3, 0.45`; with `A / 3` for `3 A / 4` it is `0.8611`, with
   `beta_0 = 1/2` at `nu = 0.3` it is `1.0989`. The column sums of `Q_1 + Q_2 + Q_3` are `(b1 + b9 + b5, b2 + b7 + b6, b3 + b8 +
   b4) = 0`: the higher-order strain averages to zero over the element, so the centroid strain is the basic strain `Bbar(alpha_b)
   u` and the higher-order part does not couple with a constant curvature (a coupled section takes `Bbar(alpha_b)` in the
   membrane-bending coupling, exactly).
5. **Rank and the single element.** The triangle of ADR 0454 arithmetic 15 in space (`est7.log`), OPT: t = 0.01, six eigenvalues
   below `8.5e-17` of the largest, the seventh `3.741e-6`; t = 0.1: `7.8e-17`, `3.694e-4`; `beta_0 = 0`: 9 zeros. Allman with the
   penalty: 6 zeros, the seventh `3.766e-6` (t = 0.01), `1.499e-4` (t = 0.1); without it 7 zeros. Rigid motions of OPT `|K r| /
   (max |K| max |r|) <= 2.0e-16`; cyclic relabelling `[1, 2, 0]`, `[2, 0, 1]` `<= 8.0e-16`.
6. **The cantilever strip (a1, b2).** Clamp: the three nodes at `x = 0` hold all six unknowns. Derivation of the drilling part:
   on a clamped edge `u_n = 0` along the whole edge; in the edge field of arithmetic 1 that needs `theta_i = theta_j` and, with
   `d(u_n) / ds = -theta` at the nodes, `theta = 0`, so `theta_z = 0` at the clamp is the discrete clamp (also F-419's cantilever
   holds all six). Disclosed: the prototype was also run with `theta_z` free at the clamp, `1.0511895148` (observation O3; it is not
   a clamp of the edge field). Load: the consistent nodal forces of the parabolic shear `tau = (3 P / (2 b))(1 - (2 y' / b)^2)`
   (`y'` from the axis) for the linear interpolation of the edge (the tangential load does no work on the normal edge excess):
   `3/16, 10/16, 3/16` of `P` at `y = 0, b/2, b`. **Estimate (prototype, `est6.log`), ANDES OPT, 8 x 2:** cut `(i, j)-(i + 1, j + 1)`
   (CT1) `u_y / delta = 0.9964751240` (`-3.525e-3`), the other diagonal (CT2) the same (mirror image about the axis under an
   antisymmetric load), alternating diagonals `(i + j) % 2` (CT3) `0.9969394138`; against Euler-Bernoulli alone `1.0338` (O2).
   Convergence of CT1's pattern: `0.9964751240, 0.9981970229, 0.9992343965, 0.9996317549, 0.9997715477` on 8 x 2 to 128 x 32: the
   clamped two-dimensional strip lies about `1.5e-4` below the Timoshenko beam (Richardson on the last three, order about 1.5), so
   the **discretisation error of the 8 x 2 strip is `3.4e-3`** and the reference contributes `1.5e-4`. End couple `M = 1 N m` at
   `x = L` (the consistent forces of the linear `sigma_xx` along `x`) against `M L^2 / (2 E I)` (CT4): `0.9972707469`.
7. **Patch tests with the new membrane (k2).** The patch of ADR 0454 arithmetic 17 (in space by `Q`, t = 0.05, `nu = 0.3`), all
   Shell3 OPT (`est8.log`): membrane interior unknowns `1.18e-13`, bending `1.01e-14`; Allman `2.1e-14`, `4.7e-15`. Why it holds:
   the linear field with `theta = omega` has every edge excess zero and every deviatoric rotation zero, so the element strain is
   exact; at an interior node the rotation rows of `K_b` receive `+- alpha_b t (l^2 / 12) sigma_nn` from each edge, and the two
   elements of an interior edge give opposite signs (the edge is `i -> j` in one and `j -> i` in the other, `n (x) n` the same), so
   they cancel. One-ulp probe (node 5 moved by one ulp): `5.1e-14`, `4.2e-15`.
8. **The mixed mesh (k3): the membrane patch fails by construction.** On an edge shared by an OPT (or Allman) triangle and an
   element with a linear edge (Shell4, or Shell3 with the constant-strain membrane), the moments `+- alpha_b t (l^2 / 12)
   sigma_nn` of the drilling element are not cancelled: a constant stress leaves unbalanced nodal moments of the order of the
   nodal forces times the edge length, so the constant-strain state is not the discrete solution. Prototype analogue (the mixed
   patch of ADR 0454 with cells 0, 4, 8 constant-strain Shell3 and the others OPT; Shell4 is not in the prototype): membrane
   `2.052e-1`, bending `4.6e-15` (unchanged: the bending rows do not change). The same holds for any free-formulation or Allman
   edge next to a linear edge. **Design consequence (D3):** a model in which an edge is shared by shell elements of different
   membrane classes (linear: Shell4 and constant-strain Shell3; Allman; ANDES) is refused naming the element.
9. **Thermal load (consistent).** The membrane thermal strain `eps_th = alpha dT_mean (1, 1, 0)`: the free expansion `u = eps_th x`,
   `theta = 0` has `theta~ = 0` and the strain `eps_th`, so `K u = A Bbar(alpha_b)^T A_m eps_th`: the consistent load is that
   (Allman: `alpha_b = 1`, his mean strain). Prototype (`est10.log`): the plate with the node lines `x = 0, 0.15, 0.35, 0.6`, `y = 0,
   0.18, 0.4` (cells cut `(0)`), t = 0.01, `nu = 0.3`, `alpha = 1.2e-5`, `dT = 50`, held at node 0 (all six): free expansion to
   `5.9e-13` (OPT), `1.9e-12` (Allman); the constant-strain thermal rows under OPT (mutation MU7) `1.3e1`. Disclosed: the uniform
   plate of ADR 0454 arithmetic 18 does not show MU7 (`2.3e-13`: an isotropic thermal stress has the same `sigma_nn` on every
   boundary edge and equal consecutive edges cancel), hence the non-uniform node lines.
10. **One-ulp probes (b4).** Single element (one node moved by one ulp in each coordinate): `max |K(x + ulp) - K(x)| / max |K| =
    5.0e-16` (t = 0.01), `4.8e-16` (t = 0.1); the rigid-motion residual at the moved geometry `3.0e-16`, `2.0e-16` (`est7.log`).
    Pure bending energy (BE1): `2.8e-14`, with one corner moved by one ulp `5.0e-14` (`est9.log`). Patch: arithmetic 7. No check
    here holds a model at one node through a soft mode except TH1 (OPT has no soft drilling mode: the drilling stiffness is of the
    order of the membrane's; TH1 is still registered relative at `1e-8`, as ADR 0454 LD3).
11. **Free plate.** The 4 x 4 plate of ADR 0454 FR1 (1 x 0.8, t = 0.1, `nu = 0.3`, cut `(0)`), stiffness only (`est11.log`): OPT 6
    zeros (the next `2.3e-5` of the largest), Allman with the penalty 6, without 7, CST 6. The bending modes of a flat plate do not
    involve the membrane, so `f_7`, `f_8` are F-419's `395.4141399498`, `525.7994847722 Hz` (prototype values of ADR 0454 P6).

## Design

### D1. The element (`shell3.rs`)

`pub enum Shell3Membrane { ConstantStrain, Allman, AndesOpt }` (non-exhaustive, `Default` = `ConstantStrain`), constants
`ANDES_ALPHA_B = 1.5`, `ANDES_BETA = [1, 2, 1, 0, 1, -1, -1, -1, -2]`, `ANDES_BETA0_MIN = 0.01`; `Shell3Options` gains the field
`membrane` (default `ConstantStrain`) and the builder `membrane(m)`. `local_stiffness_section`: for `ConstantStrain` the present
code, unchanged; for `Allman` the membrane energy of arithmetic 1 by the mid-edge rule and the drilling penalty with Allman's
`omega`; for `AndesOpt` `K_b + K_h` of arithmetic 4 on the unknowns `(u, v, theta_z)` and no drilling penalty (the options'
`drilling` is not used). A coupled section takes `Bbar(alpha)` rows (`alpha = 1` Allman, `alpha_b` ANDES) in the coupling. The
bending and the shear rows are untouched. `thermal_load_vector_with(coords, t, material, dT, membrane)` (arithmetic 9;
`thermal_load_vector` calls it with `ConstantStrain`, the same arithmetic as now). `element_stresses` takes the membrane: the
membrane strain at the centroid is `Bbar(alpha) u` for Allman and ANDES (arithmetic 4), the present rows for `ConstantStrain`.
Mass, pressure and body loads stay those of the linear field for every membrane (L2).

### D2. The opt-in (`mesh.rs`, `shell3.rs`)

`BlockProperty::Shell` gains the field `membrane: Shell3Membrane` (the variant is non-exhaustive: other crates cannot build it
literally, nothing outside `rcfd-fem` matches its fields); `BlockProperty::shell(t)` sets `ConstantStrain`; new
`BlockProperty::shell_with_membrane(t, membrane)` (the same refusals). `set_block_property` refuses a membrane other than
`ConstantStrain` on a Shell4 block (`UnsupportedKind`, operation "a Shell3 membrane"). `shell3::model_element_stiffness` takes the
block's membrane; `shell.rs` keeps its two `Shell { thickness }` patterns with `..` added and passes the membrane to
`element_stresses`; `thermal.rs` calls `thermal_load_vector_with` with the block's membrane. For a model without a non-default
membrane every call is the former one with the former arithmetic (B1).

### D3. Mixed meshes (`shell3.rs`, one call in `shell.rs::check_shell_model`)

`shell3::check_membrane_edges(mesh)`: returns at once if no Shell3 block has the Allman or ANDES membrane; otherwise every edge of a
Shell4 or Shell3 element (unordered node pair) is recorded with the first element and its class (linear, Allman, ANDES); a later
element on that edge with another class is refused, `FemError::Element { element, source: InvalidValue { key: "membrane", .. } }`,
the message naming the other element, the edge's nodes and ADR 0466 arithmetic 8. Elements meeting at a node only are taken.

### D4. Which paths

Every path that takes Shell3 (static and modal, elimination and transformation, the assembly, the mass, the loads, the thermal
load, the stresses) takes the membrane the same way; the paths that refuse Shell3 refuse it as before.

### D5. The bit record of the default (B1)

`crates/rcfd-fem/tests/sa_shell3_membrane_bits.rs`, using only the API of the base d0439898, run once on the base after this
commit and before any library code; its digests (FNV-1a of the bits) written into `EXPECTED` from that run: a Shell3 plate (4 x 3
cells cut alternately, slightly warped, clamped, every shell load: pressure, angle-weighted pressure, gravity, centrifugal, nodal
body force, temperature) stiffness, mass, loads, solution, stresses, four frequencies; a mixed Shell3 + Shell4 plate stiffness,
solution, stresses; a Shell4 plate stiffness and solution; one Shell3 element through `shell3::element_stiffness`,
`element_stiffness_with` (drilling 0, stabilisation 0, the displacement shear) and `thermal_load_vector`.

## Checks (limits fixed now)

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| CT1 | a1 | the strip of (a1), 8 x 2, cut `(i, j)-(i + 1, j + 1)`, ANDES: `|u_y / delta - 1|` | `2e-2` | arithmetic 6: `3.525e-3` (discretisation `3.4e-3`, reference `1.5e-4`) |
| CT2 | a1 | the same, the other diagonal | `2e-2` | `3.525e-3` |
| CT3 | a1 | the same, alternating diagonals `(i + j) % 2` | `2e-2` | `3.061e-3` |
| CT4 | a1 | end couple `M = 1 N m`, cut `(0)`: `|u_y / (M L^2 / (2 E I)) - 1|` | `2e-2` | `2.729e-3` |
| PT1 | a2 (k2) | ANDES membrane patch (ADR 0454 patch, in space), interior unknowns | `1e-8` | arithmetic 7: `1.18e-13` (ulp `5.1e-14`) |
| PT2 | a2 (k2) | its `N` against `A_m eps` relative to `max |N|`; `m`, `Q` relative to the same | `1e-8` | rounding |
| PT3 | a2 (k2) | ANDES bending patch, interior unknowns | `1e-8` | `1.01e-14` |
| PT4 | a2 (k2) | its `m` against `D_b kappa`; `Q`, `N` times the side, relative to `max |m|` | `1e-8` | ADR 0454 PT4 |
| PT5 | a2 (k3) | mixed patch (ADR 0454: cells 0, 4, 8 Shell4, the others Shell3 ANDES), membrane, interior unknowns (through `assemble_with`, the model being refused, RF1) | `1e-8` | arithmetic 8: **predicted to miss** (analogue `2.05e-1`) |
| PT6 | a2 (k3) | the same, bending | `1e-8` | `4.6e-15` (analogue) |
| PT7 | a2 | Allman membrane and bending patch (all Shell3), interior unknowns | `1e-8` each | `2.1e-14`, `4.7e-15` |
| BE1 | D1 | rectangle pairs (corner at `(0.2, 0.1)`), `a / b = 0.25, 1, 4`, both diagonals, both bending directions, `nu = 0, 0.3`, one ANDES element pair through `shell3::element_stiffness_with`: `max |U / U_exact - 1|` | `1e-10` | arithmetic 4, 10: `2.8e-14` (ulp `5.0e-14`) |
| SE1 | b3 | the element of ADR 0454 arithmetic 15, t = 0.01 and 0.1: ANDES count of `|lambda| <= 1e-10 lambda_max` is 6 and `lambda_7 / lambda_max`; Allman (with the penalty) 6 | `6`, `>= 1e-7`; `6` | arithmetic 5: `3.74e-6`, `3.69e-4`; Allman 6 |
| SE2 | D1 | ANDES rigid motions `|K r| / (max |K| max |r|)`; `K` symmetric bit for bit | `1e-12`; exact | `2.0e-16` (ulp `3.0e-16`) |
| ISO1 | D1 | ANDES with the nodes relabelled `[1, 2, 0]`, `[2, 0, 1]`, permuted back, against `K` relative to `max |K|` | `1e-12` | `8.0e-16` (ulp: `K` moves `5.0e-16`) |
| FR1 | b3 | 4 x 4 free plate, t = 0.1, ANDES model: Sturm count below `lambda_7 / 4` (`lambda_7` of its modal analysis, 8 modes) | `6` | arithmetic 11 |
| FR2 | b3 | its `f_7`, `f_8` against `395.4141399498`, `525.7994847722 Hz` | `1e-5` each | arithmetic 11 |
| TH1 | D1 | the plate of arithmetic 9, ANDES model, `dT = 50`: translations against `eps_th x` relative to `max`; membrane forces relative to `E alpha dT t` | `1e-8`; `1e-8` | `5.9e-13` |
| TH2 | D1 | the same with Allman | `1e-8`; `1e-8` | `1.9e-12` |
| ST1 | D1 | one ANDES element in its plane (a single-element model), unknowns only `theta_z = (1, -2, 0.5) 1e-3` at the nodes: `shell::stresses` membrane force against `A_m eps`, `eps = (alpha_b / A) sum_e (l_e^2 / 12)(theta_j - theta_i) n_e (x) n_e` computed in the test | `1e-12` relative to `max |N|` | arithmetic 4 (element-level, no solve) |
| D1 | D2 | single-element models (ANDES, Allman): `stiffness(model)` equals `shell3::element_stiffness_with(.., Shell3Options::default().membrane(m))` bit for bit | exact | D2 |
| RF1 | D3 | the mixed patch model (ANDES next to Shell4) and a two-block Shell3 model (ANDES next to constant-strain) on `solve_static`: `Element { element, InvalidValue { key: "membrane" } }`, the message naming the other element; a model where the blocks meet at one node only solves | exact | D3 |
| RF2 | D2 | `shell_with_membrane(t, AndesOpt)` on a Shell4 block refused (`UnsupportedKind`); `ConstantStrain` on Shell4 taken; `t = 0` refused (`thickness`) | exact | D2 |
| B1 | b5 | the digests of `sa_shell3_membrane_bits` equal the base run | exact | D5 |
| B2 | b5, k1 | the existing tests pass unedited: every `rcfd-fem` test binary named `sa_shell*`, `si_shell*`, `sa_scaffold`, and `rcfd-post`'s `sa_shell3_output`, `si_shell_output*`, release | pass | D2 |
| B3 | b5 | `cargo test -p rcfd-api --release --test baseline_hashes` (rcfd-api depends on rcfd-fem) | pass | no fluid file touched |

Stronger checks against the prototype (`est*.log`):

| id | check | limit | prototype |
| --- | --- | --- | --- |
| P1 | CT1 `0.9964751240`, CT3 `0.9969394138`, CT4 `0.9972707469`; 16 x 4 cut `(0)` `0.9981970229`; N1 `0.5428975996`; N2 `0.9156777414` | `1e-5` relative each | `est2.log`, `est6.log` |

Negative controls and observations:

| id | control | expected | limit |
| --- | --- | --- | --- |
| N1 | the strip of CT1 with the default constant-strain membrane | stiff | `< 0.9` (`0.5429`) |
| N2 | the strip of CT1 with Allman's membrane | misses the acceptance | `< 0.98` (`0.9157`) |
| N3 | single element, Allman with `drilling = 0` (`element_stiffness_with`): zero count | the equal-rotation mode | `7` |
| N4 | the free plate of FR1 with Allman, `drilling = 0` (through `assemble_with`): Sturm count below the same shift | one global spurious mode | `7` |
| O1 | PT5's number, printed | arithmetic 8 | observation |
| O2 | CT1 against Euler-Bernoulli `P L^3 / (3 E I)`, printed | `1.0338` | observation |
| O3 | CT1 with `theta_z` free at the clamp, printed | `1.0512` | observation |

CT, PT, BE, SE, ISO, FR, TH, ST, D, RF, P, N and O are one test file `crates/rcfd-fem/tests/sa_shell3_membrane.rs`, run once.

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU1 | `ANDES_ALPHA_B = 1.0` | CT1, P1 (prototype `0.8464`), BE1 |
| MU2 | `beta_0 = 1` | CT1, P1 (`0.9387`), BE1 |
| MU3 | `K_theta` with `A / 3` in place of `3 A / 4` | BE1, P1 |
| MU4 | `beta_2` and `beta_3` exchanged | BE1, P1 |
| MU5 | the model's membrane ignored (`model_element_stiffness` with the default options) | CT1, D1, N2 |
| MU6 | the stresses of ANDES from the constant-strain rows | ST1 |
| MU7 | the thermal load of ANDES from the constant-strain rows | TH1 |
| MU8 | `check_membrane_edges` not called | RF1 |
| MU9 | `set_block_property` accepts any membrane on Shell4 | RF2 |
| MU10 | the default `ConstantStrain` takes the ANDES branch | B1, B2, N1 |
| MU11 | Allman's edge excess with the opposite sign | PT7, TH2, N2 (P1) |
| MU12 | Allman's penalty dropped | SE1 (Allman), N3 not (it sets drilling 0 already) |

## Decision rule

- **done:** CT1 to CT4, PT1 to PT7, BE1, SE1, SE2, ISO1, FR1, FR2, TH1, TH2, ST1, D1, RF1, RF2, B1 to B3, P1 met, the controls as
  stated, every mutation caught by a registered check, no amendment after a run.
- **partial:** a check missed (recorded as a `*_known_miss`) or not run, a mutation not caught, or a check met only after an
  amendment written after its run. PT5 is predicted to miss (arithmetic 8), so the predicted outcome is **partial**.
- **todo:** the membrane does not build.

## Limits (not covered)

- L1. The higher-order stiffness and its parameters are taken from memory (verified by BE1, not derived); `beta_0` assumes an
  isotropic section (`nu` from `A_m`); a layup keeps the constant-strain membrane (its property has no membrane field).
- L2. Mass, pressure and body loads use the linear field for every membrane (no drilling moments from in-plane body forces); the
  consistent mass of a free-formulation element is not defined by a field.
- L3. A drilling membrane next to a linear-edge element fails the membrane patch test (arithmetic 8); such meshes are refused
  (D3). A Shell4 with a matching drilling edge, or an interface treatment, is a follow-up.
- L4. Stresses at the centroid only, from the basic strain; the higher-order strain is not reported.
- L5. Edge tractions do not exist as a load in rcfd; a consistent edge load on a drilling membrane would carry the moments `alpha_b
  t l^2 q_n / 12`.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The element: ANDES OPT (taken) rather than Allman's alone (arithmetic 3: `0.916` on 8 x 2, the acceptance not reachable);
   Allman's kept as an option and control.
2. The opt-in through a field of the Shell property (taken) rather than a new property variant (more match arms in shared files)
   or a model-wide option.
3. Mixed meshes with a drilling membrane next to a linear edge refused (taken; arithmetic 8) rather than solved with a wrong
   constant-strain state.
4. The clamp holds `theta_z` (arithmetic 6; F-419's cantilever does the same).

## Addendum 1: results (2026-11-14)

The registered run is commit 7801e0ea (`f539-scratch/run1.log`), made once. Cargo stopped after the failing binary, so B1 ran
separately on the same commit (`run1-b1.log`). Disclosed: B1 had also been run once during development, after the library
compiled and before the checks file was written (the same test, the same result). After the run, PT5's miss was recorded as
rule 2 asks (commit 0c676472). The registered test is kept as the ignored record
`pt_patch_tests_with_the_drilling_membranes_known_miss` with PT5's limit intact, and `pt_patch_tests_kept` runs the same
checks with PT5 printed only (`run2-pt-kept.log`, identical numbers). In the same commit `cargo fmt` changed the layout of
F-539 lines only; the two test files passed again after it (`after-fmt.log`). B2 and B3 ran on 0c676472. No amendment was
written.

| id | limit | measured | met |
| --- | --- | --- | --- |
| CT1 | `2e-2` | `3.525e-3` (`u_y / delta = 0.9964751240`) | yes |
| CT2 | `2e-2` | `3.525e-3` (`0.9964751240`) | yes |
| CT3 | `2e-2` | `3.061e-3` (`0.9969394138`) | yes |
| CT4 | `2e-2` | `2.729e-3` (`0.9972707469`) | yes |
| PT1 | `1e-8` | `8.5e-15` | yes |
| PT2 | `1e-8` | `1.5e-15`; m and Q `5.8e-16` | yes |
| PT3 | `1e-8` | `1.1e-15` | yes |
| PT4 | `1e-8` | `1.3e-14`; Q and N `8.9e-13` | yes |
| PT5 | `1e-8` | **`2.0776e-1`** (predicted, arithmetic 8; analogue `2.05e-1`) | **no** (`*_known_miss`) |
| PT6 | `1e-8` | `1.1e-15` | yes |
| PT7 | `1e-8` each | membrane `3.9e-15`, bending `7.5e-16` | yes |
| BE1 | `1e-10` | `3.7e-14` | yes |
| SE1 | `6`, `>= 1e-7`; Allman `6` | 6, `3.741e-6` (t = 0.01); 6, `3.694e-4` (t = 0.1); Allman 6 and 6 (`lambda_7` `3.77e-6`, `1.50e-4`) | yes |
| SE2 | `1e-12`; exact | `2.8e-16`, `3.2e-16`; symmetric bit for bit | yes |
| ISO1 | `1e-12` | `1.0e-15` largest | yes |
| FR1 | `6` | 6 | yes |
| FR2 | `1e-5` each | `2.0e-15`, `5.2e-14` (`f_7 = 395.4141399498 Hz`) | yes |
| TH1 | `1e-8`; `1e-8` | `1.9e-14`; `2.7e-15` | yes |
| TH2 | `1e-8`; `1e-8` | `2.3e-13`; `2.0e-15` | yes |
| ST1 | `1e-12` | `1.8e-16` | yes |
| D1 | exact | ANDES and Allman bit for bit | yes |
| RF1 | exact | ANDES next to Shell4 refused (element 3, naming element 0, nodes 1 and 5); ANDES next to the constant-strain Shell3 refused; blocks meeting at one node solve | yes |
| RF2 | exact | `UnsupportedKind { block: 0, kind: "Shell4", operation: "a Shell3 membrane" }`; constant strain on Shell4 taken; `thickness` refused | yes |
| B1 | exact | 15 of 15 digests equal the base | yes |
| B2 | pass | the whole `rcfd-fem` suite, release: 99 test binaries, 0 failed (`b2-fem.log`) | yes |
| B3 | pass | baseline_hashes 3 passed, 1 ignored (the existing record) | yes |
| P1 | `1e-5` each | largest `7.1e-11` (N1); CT1 `7.6e-12` | yes |
| N1 | `< 0.9` | `0.5428975996` | yes |
| N2 | `< 0.98` | `0.9156777414` | yes |
| N3 | `7` | 7 and 7 | yes |
| N4 | `7` | 7 | yes |
| O1 | printed | `2.0776e-1` | observation |
| O2 | printed | `1.0338429412` | observation |
| O3 | printed | `1.0511895148` | observation |

Mutations (archive copy of 0c676472, `f539-scratch/mutate.py`, `mutlogs/`), each caught:

| id | caught by |
| --- | --- |
| MU1 | CT1 to CT4, BE1, P1 |
| MU2 | CT1 to CT4, BE1, P1 |
| MU3 | CT1 to CT4, BE1, P1 |
| MU4 | CT1 to CT4, BE1, P1 |
| MU5 | CT1 to CT4, D1, TH1, TH2, P1 (with N2) |
| MU6 | ST1 |
| MU7 | TH1, TH2 |
| MU8 | RF1 (both refusals) |
| MU9 | RF2 |
| MU10 | B1, N1, P1, and the existing `sa_shell3` (PT5, CY1, FR, N1, P4) |
| MU11 | CT1 to CT4, BE1, ST1, P1 (with N2); not PT7 or TH2 as registered: a reversed excess is still exact for the patch and free-expansion states, where every edge excess is zero |
| MU12 | SE1 (Allman, 7 zeros), P1 (N2); TH2 panicked on the singular solve |

fmt (`cargo fmt -p rcfd-fem --check`) and clippy (`cargo clippy -p rcfd-fem --all-targets --release -- -D warnings`): clean.
`rcfd-cli`'s `docs_site` test (for `docs/reference-data.md`): 2 passed. Not run: the whole workspace suite (the lead's gate).
Shared solver files changed (`git diff d0439898`): `mesh.rs` (the `membrane` field of `BlockProperty::Shell`, `shell` delegating
to the new `shell_with_membrane`, the Shell arm of `set_block_property`), `shell.rs` (two patterns gain `..`, one line in
`check_shell_model`, one in `shell3_stresses`), `thermal.rs` (the Shell3 thermal call), `shell3.rs` (the membranes).

**Decision: partial**, by the rule, because PT5 missed. PT5 is F-419's mixed Shell3 + Shell4 membrane patch, run with the new
membrane. The registration predicted the miss and diagnosed its cause (arithmetic 8: a linear-edge neighbour does not cancel
the drilling edge moments `alpha_b t l^2 sigma_nn / 12`). The model refuses such meshes (RF1). Every other check was met and
every mutation caught. The acceptance's bending clause is met: the 8 x 2 strip is within `3.5e-3` of beam theory.

Remaining questions (options, recommendation):

1. Mixed meshes (PT5, L3): (a) keep the refusal; (b) a follow-up: a Shell4 drilling membrane with the same edge field (a
   quadrilateral free-formulation or ANDES membrane), so that ANDES Shell3 and the new Shell4 can share edges; (c) an interface
   treatment that drops the excess on mixed edges (not patch-test exact either). Recommend (a) now and (b) as a follow-up.
2. The default membrane of Shell3: (a) keep the constant-strain default and ANDES opt-in; (b) make ANDES the default (changes
   every Shell3 result and the B1 digests; needs the maintainer). Recommend (a).
3. Allman's membrane as a model option (N2: `0.916` on 8 x 2): (a) keep it for comparison; (b) keep it only in `Shell3Options`.
   Recommend (a), documented as stiff.

## For the lead: roadmap entries

Proposed status of F-539: **partial**.

> Structural solver, advanced tier. Follow-up of F-419 (ADR 0454, 2026-11-14). F-539 (ADR 0466, 2026-11-14): opt-in drilling membranes for Shell3 through `BlockProperty::shell_with_membrane`. The ANDES optimal membrane: basic stiffness of a constant stress on Allman's edge field with alpha_b 3/2, higher-order stiffness of the deviatoric corner rotations with beta_0 (1 - 4 nu^2)/2 and the OPT betas, recalled and verified by exact pure bending of rectangle pairs to 3.7e-14. Allman's 1984 triangle: its equal-rotation zero mode suppressed by Shell3's drilling penalty; ANDES needs no penalty. Consistent thermal load and centroid stresses. Cantilever strip 8 x 2 in in-plane bending: 0.99648 of Timoshenko beam theory on either diagonal (0.99694 alternating, end couple 0.99727; CST 0.543, Allman 0.916). Patch tests on Shell3 meshes to 1e-14; six zero modes per element and per free plate; default Shell3 and Shell4 bit-identical; twelve mutations caught. Partial: the mixed Shell3 + Shell4 membrane patch misses (2.1e-1, predicted: a linear edge does not balance the drilling edge moments), so the model refuses such meshes, naming the element.

Follow-up item:

- **Drilling membrane for Shell4 matching the ANDES edge** (deps F-539, F-392):

> Every limit not stated here is registered in the ADR before the run. An opt-in Shell4 membrane with the vertex-rotation edge field of ADR 0466, so that ANDES Shell3 and Shell4 may share edges: the mixed Shell3 + Shell4 membrane patch of ADR 0454 within 1e-8, the 8 x 2 in-plane cantilever strip of ADR 0466 on quadrilaterals within 2 % of beam theory, defaults bit-identical.
