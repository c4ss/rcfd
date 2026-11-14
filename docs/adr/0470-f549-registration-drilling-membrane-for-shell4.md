# ADR 0470: F-549 registration: a drilling membrane for Shell4 matching the ANDES edge (four overlapping ANDES OPT triangles), opt-in through the Shell block property, and the mixed-edge refusal lifted for the ANDES pairing

Status: **REGISTERED 2026-11-14, written before any code of F-549 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-549 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with a plain numpy prototype in the scratch folder
(`w49i-work/f549-scratch`: `andes.py`, `proto.py` (verbatim copies of the F-539 prototypes of ADR 0466, which matched rcfd's
ANDES Shell3 to `7.6e-12` on its CT1), `proto392.py` (a verbatim copy of `f392-scratch/estimate.py`, the F-392 prototype of
Shell4, which matched rcfd's Shell4 to `6.1e-10` on every check of ADR 0432), `quad.py` (the membrane of this ADR), `shell24.py`
(the full 24-unknown element), `est1.py` to `est4.py`, logs `est*.log`; dense matrices, no rcfd code; disclosed, its numbers
quoted below). The bit record B1 of the default paths is a test file written and run on the base after this commit and before
any library code (D6). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Follow-up of F-539 ([ADR 0466](0466-f539-registration-drilling-membrane-for-shell3.md): PT5, the mixed Shell3 + Shell4 membrane
patch, missed by `2.08e-1`, predicted by its arithmetic 8; such meshes refused, RF1; remaining question 1 (b)). Roadmap item
F-549 (deps F-539, F-392). Number 0470 given by the lead. Branch `f549-work` from local main fc026814, worktree
`w49i-work/f549`, scratch `w49i-work/f549-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; no existing test edited (new tests in new files); clean-room.

## What F-549 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. An opt-in Shell4 membrane with the vertex-rotation edge field of ADR 0466, so that ANDES Shell3 and Shell4 may share edges: the mixed Shell3 + Shell4 membrane patch of ADR 0454 within 1e-8, the 8 x 2 in-plane cantilever strip of ADR 0466 on quadrilaterals within 2 % of beam theory, defaults bit-identical.

The brief adds: (b1) a published quadrilateral with drilling unknowns sharing the Allman / ANDES edge field, cited from memory
and marked NOT read where not read, verified by arithmetic or a numpy prototype before this registration to have exactly the edge
field of ANDES Shell3; (b2) one-ulp probes before any `1e-12`-class limit, `1e-11`-class limits for sums of more than `1e3` terms;
(b3) when the opt-in membranes on both sides match, the refusal RF1 of ADR 0466 lifted for that pairing only and kept otherwise;
(b4) opt-in, default Shell4 and Shell3 bits unchanged (digests on the base before code); (b5) shared-file edits minimal.

Clauses: (a0) every limit registered here; (a1) an opt-in Shell4 membrane with the vertex-rotation edge field of ADR 0466; (a2)
ANDES Shell3 and the new Shell4 may share edges (the model takes such a mesh); (a3) the mixed Shell3 + Shell4 membrane patch of
ADR 0454 within `1e-8`; (a4) the 8 x 2 in-plane cantilever strip of ADR 0466 on quadrilaterals within 2 percent of beam theory;
(a5) defaults bit-identical.

Readings, registered:

- **(a3) "the mixed Shell3 + Shell4 membrane patch of ADR 0454":** the patch of ADR 0454 arithmetic 17 (the 16 nodes of
  `PATCH`, carried into space by the rotation `Q` about `(1, -2, 2)` by `0.7` rad, t = 0.05, `nu = 0.3`), cells 0, 4, 8 Shell4,
  the others two Shell3 each (cut by `n % 2`), as built by `patch_model(.., mixed = true, ..)` of ADR 0466's test: every Shell3
  with the ANDES membrane and every Shell4 with the new membrane; the measure of ADR 0466 PT5, `max |u - u_exact|` over the six
  unknowns of the four interior nodes relative to the largest exact component, the boundary nodes held at the exact linear field
  (`theta = omega`). It runs through `solve_static_with`, the model being taken (a2).
- **(a4) "the 8 x 2 in-plane cantilever strip of ADR 0466 on quadrilaterals":** ADR 0466 reading (a1) with each of the 8 x 2
  square cells one Shell4 element with the new membrane: `L = 1`, `b = 0.25`, `t = 0.01`, `E = 2e11`, `nu = 0`, clamped at `x =
  0` (all six unknowns of the three nodes), the consistent forces of the parabolic shear `P = 1 N` at `x = L`, `u_y` of the tip
  node on the axis against the Timoshenko beam `P L^3 / (3 E I) + P L / (kappa G A)`, `kappa = 5/6`.
- **(a5) "defaults bit-identical":** every model whose Shell4 blocks do not ask for the new membrane gives the bits it gave on
  the base, including models with the opt-in ANDES and Allman Shell3 membranes of ADR 0466 (B1, B2).
- **(b3) "lift RF1 for that pairing only":** an edge shared by an ANDES Shell3 and an ANDES Shell4 is taken; an edge of an ANDES
  Shell4 shared with a default Shell4, a constant-strain Shell3 or an Allman Shell3 is refused, and an ANDES Shell3 next to a
  default Shell4 is refused as before.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0466 (whole: arithmetic 1 to 11, the design, the checks, addendum 1); ADR 0432 (the element,
  the facet, the transformation, the drilling penalty); the roadmap entries F-549, F-539, F-392.
- `crates/rcfd-fem/src/shell3.rs` (whole), `shell.rs` (lines 70 to 1093: the options, the facet, the stiffness, the loads, the
  thermal load, the model checks, the stresses), `mesh.rs` (`BlockProperty::Shell`, its constructors, `set_block_property`),
  `thermal.rs` (the shell thermal call, lines 110 to 125), `assemble.rs` (the shell dispatch, lines 215 to 230); the `grep` of
  the callers of `shell::model_element_stiffness`, `shell::thermal_load_vector` and of `BlockProperty::Shell {` over `crates`
  (in `src` only `rcfd-fem`).
- Tests: `crates/rcfd-fem/tests/sa_shell3_membrane.rs` (the helpers, the strip, the patch, RF1, RF2),
  `sa_shell3_membrane_bits.rs` (the digest pattern).
- `f539-scratch/andes.py`, `proto.py`, `est6.py`; `f392-scratch/estimate.py` (copied verbatim as noted).
- **Not read:** any finite-element code, manual or documentation outside this repository, and no paper. Cited from memory
  (marked NOT read in `docs/reference-data.md`): C. A. Felippa and C. Militello, "Membrane triangles with corner drilling
  freedoms. II. The ANDES element", Finite Elem. Anal. Des. 12 (1992) 189-201, and K. Alvin, H. M. de la Fuente, B. Haugen and
  C. A. Felippa, "Membrane triangles with corner drilling freedoms. I. The EFF element", same volume 163-187 (the remark that
  quadrilaterals are built from assembled drilling triangles); B. Haugen, "Buckling and stability problems for thin shell
  structures using high performance finite elements", PhD thesis, University of Colorado, 1994 (a quadrilateral shell made of
  ANDES triangles); the overlapping-triangle quadrilateral (a quadrilateral as the mean of its two diagonal splits), a construction
  of the early membrane literature that I cannot attribute with confidence. What is used is **derived** below (arithmetic 1 to 3)
  and verified by the prototype; nothing numerical is taken from these sources (the ANDES triangle and its parameters are those of
  ADR 0466, unchanged). The quadrilateral of Ibrahimbegovic, Taylor and Wilson (1990) and MacNeal-Harder's are not used: their
  edge fields are not the ANDES edge with `alpha_b = 3/2` (arithmetic 1), which is what (a1) asks.

## Arithmetic done before the code (disclosed)

Local axes of the facet of Shell4 (ADR 0432 arithmetic 1: the projected nodes counter-clockwise about `e_3`, the facet convex),
membrane unknowns `(u_i, v_i, theta_i)` at the four projected nodes, `theta = theta_z` positive about `e_3`.

1. **The element (derived): four overlapping ANDES OPT triangles.** The quadrilateral `0 1 2 3` has two diagonal splits, `S_1 =
   {(0, 1, 2), (0, 2, 3)}` and `S_2 = {(0, 1, 3), (1, 2, 3)}`, every triangle counter-clockwise. The membrane stiffness is
   `K_q = (1/2) sum_{T in S_1 u S_2} K_T`, `K_T` the 9 x 9 ANDES OPT membrane of ADR 0466 arithmetic 4 (basic stiffness with
   `alpha_b = 3/2`, higher-order stiffness with `beta_0` and the OPT betas) on the local coordinates of `T`, scattered to the
   quadrilateral's unknowns `(0, 1, 5)` of its nodes; no drilling penalty. **The edge field.** Each boundary edge of the
   quadrilateral is an edge of exactly one triangle of each split, traversed in the same direction (all counter-clockwise), with
   the same length and outward normal; each diagonal is an interior edge of one split, traversed in opposite directions by its two
   triangles. Under a constant stress `sigma` the nodal forces of `K_T` on any field that `K_T` integrates exactly are the
   boundary work of `sigma` on the ANDES edge field (ADR 0466 arithmetic 4 and 7): per edge `i -> j` the forces of the linear
   field and the moments `+ alpha_b t (l^2 / 12) sigma_nn` at `j`, `-` at `i`. Summed over a split the diagonal terms cancel;
   with weight `1/2` per split and two splits, the quadrilateral's forces are the boundary work of `sigma` on **its four edges
   with the ANDES edge field of weight 1**, the same per edge as an ANDES Shell3 triangle with that edge. Hence on an edge shared
   with an ANDES Shell3 (traversed the other way, the same `n (x) n`) the moments cancel as between two ANDES triangles (ADR 0466
   arithmetic 7), and a mesh of ANDES Shell3 and these quadrilaterals passes the membrane patch test exactly. The linear field
   with `theta = omega` gives every triangle its exact strain and zero deviatoric rotations, so the higher-order stiffness does
   not act in the patch state. An Ibrahimbegovic-Taylor-Wilson or MacNeal quadrilateral would need its edge moment to be `alpha_b
   l^2 / 12` with `alpha_b = 3/2` on every edge to share edges with ANDES Shell3; Allman-type quadrilateral fields have `alpha =
   1`; that is why this construction is chosen (open question 1).
2. **Prototype of the patch (b1), `est1.log`** (2D membrane, the `PATCH` nodes, `nu = 0.3`, `t = 0.05`, the measure of reading
   (a3) without the rotation): all ANDES triangles `3.5e-15`; all nine cells these quadrilaterals `1.4e-15`; **mixed (cells 0, 4,
   8 quadrilaterals, the others ANDES triangles) `3.3e-15`**; mixed with Shell4's default bilinear membrane and penalty in the
   quadrilaterals `2.53e-1` (ADR 0466 PT5 measured `2.08e-1` in space with its scale); mixed with constant-strain triangles next to
   these quadrilaterals `2.55e-1`. One-ulp probe (node 5 moved by one ulp): mixed `2.3e-15`, all quadrilaterals `4.5e-15`.
3. **The single element.** The distorted quadrilateral `(0, 0), (1.1, 0.1), (0.9, 0.8), (0.15, 1.0)`, 12 x 12 membrane
   (`est1.log`): three zero eigenvalues (the in-plane rigid motions; the equal-rotation mode is stiff through the higher-order
   stiffness of each triangle), `lambda_4 / lambda_max = 7.85e-3` (t = 0.01 and 0.1); rigid motions `|K r| / (max |K| max |r|) <=
   1.6e-16`; relabelling `[1, 2, 3, 0]`, `[2, 3, 0, 1]` `<= 2.4e-16` (the two splits exchange); one-ulp move of a node changes `K`
   by `3.2e-16` of `max |K|`. The full 24 x 24 element in space (`shell24.py`, `est3.log`, `est4.log`): the warped facet
   `(0.3, -0.2, 0.5), (1.4, 0, 0.52), (1.2, 0.75, 0.47), (0.4, 0.85, 0.515)` (warp ratio `0.0201`, below Shell4's `0.05`): six
   zeros below `1e-10 lambda_max`, `lambda_7 / lambda_max = 1.738e-6` (t = 0.01), `1.738e-4` (t = 0.1); default Shell4 `2.66e-6`,
   `6.12e-6`; rigid motions in space with the rigid offsets `<= 2.1e-16`; cyclic relabelling `<= 4.5e-16`; one ulp `3.0e-16`.
4. **Pure bending (`est2.log`).** A rectangle `a x b` with its corner at `(0.2, 0.1)`, `a / b = 0.25, 1, 4`, `nu = 0, 0.3`, both
   directions, the exact field of ADR 0466 arithmetic 4: one quadrilateral `|U / U_exact - 1| <= 2.85e-14` (each split is an exact
   rectangle pair, ADR 0466 BE1, so their mean is exact); one ulp at the corner `4.4e-16`.
5. **The cantilever strip (a4), `est1.log`.** 8 x 2 quadrilaterals: `u_y / delta = 0.9957912994` (`-4.209e-3`); 16 x 4
   `0.9981052714`, 32 x 8 `0.9992225465`, 64 x 16 `0.9996302421` (the same approach to the two-dimensional strip about `1.5e-4`
   below the Timoshenko beam as ADR 0466 arithmetic 6), so the discretisation error of 8 x 2 is about `4.1e-3`; end couple `M =
   1 N m` against `M L^2 / (2 E I)` `0.9967626398`; Shell4's default bilinear membrane `0.8838414209`; `theta_z` free at the clamp
   `1.0470229102` (observation); the ANDES triangle strip of ADR 0466 CT1 reproduced `0.9964751240`. Distorted 8 x 2 (interior
   node columns moved along `x` by `+- h / 4` at the top and bottom edges, alternating; `est4.log`): `0.9726063308`, the bilinear
   `0.8502431650` (observation only). MU4 (the penalty kept with the new membrane): `0.9957584513` (`3.3e-5` from CQ1).
6. **Free plate (`est3.log`).** The 4 x 4 plate of ADR 0454 FR1 (1 x 0.8, t = 0.1, `nu = 0.3`) in Shell4 with the new membrane,
   stiffness only: six zeros below `1e-10 lambda_max`, the seventh `2.7e-5` of the largest (the membrane alone `2.2e-4`,
   `est2.log`).
7. **Thermal load (consistent).** The free expansion `u = eps_th x`, `theta = 0` has zero deviatoric rotations in every triangle
   and the strain `eps_th`, so `K_q u = (1/2) sum_T A_T Bbar_T(alpha_b)^T A_m eps_th`: the consistent membrane thermal load is
   that, with `eps_th = alpha dT_T (1, 1, 0)`, `dT_T` the mean of the triangle's three nodal temperature changes (exact for a
   uniform `dT`). Prototype (`est2.log`): the quadrilateral plate with the node lines `x = 0, 0.15, 0.35, 0.6`, `y = 0, 0.18,
   0.4` (six cells), t = 0.01, `nu = 0.3`, `alpha = 1.2e-5`, `dT = 50`, held at node 0: free expansion to `2.3e-14`; the same with
   the rows without the edge excess (`alpha_b = 0`, the linear-field rows; mutation MU8) `2.2e0`.
8. **Stresses.** The membrane strain reported at the centre is the area mean of the basic strain, `eps = sum_T (1/2) A_T Bbar_T
   u / sum_T (1/2) A_T` (`sum_T (1/2) A_T` is the area of the projected facet). Its rotation part: in each split the diagonal terms
   cancel (arithmetic 1), so `A_q eps_theta = alpha_b sum_{quad edges} (l^2 / 12)(theta_j - theta_i) n (x) n` (tensor strain,
   `gamma_xy` twice its off-diagonal), a closed form independent of the split used by ST1.
9. **One-ulp probes (b2).** Arithmetic 2, 3, 4. No check here holds a soft mode; the limits of `1e-12` class are on sums of at
   most 24 x 24 x 24 terms per entry (SE2, ISO1, ST1), below the `1e3`-term mark of (b2) per accumulated value only for ST1; SE2
   and ISO1 compare matrices whose entries are sums of about `1e4` products, so they are registered at `1e-11` (b2) against the
   probe `3e-16`.

## Design

### D1. The element (`shell.rs`, one helper in `shell3.rs`)

`pub enum Shell4Membrane { Bilinear, AndesOpt }` in `shell.rs` (non-exhaustive, `Default` = `Bilinear`); `ShellOptions` gains the
field `membrane` (default `Bilinear`) and the builder `membrane(m)`. `local_stiffness_section`: for `Bilinear` the present
arithmetic in the present order (the membrane rows and the drilling penalty by 2 x 2 Gauss); for `AndesOpt` no bilinear membrane
and no drilling penalty in the Gauss loop, and after it `K_q` of arithmetic 1 added into the unknowns `(6 i, 6 i + 1, 6 i + 5)`.
The bending and the shear rows are untouched. A coupled section (reachable only through the element functions with a layup
section: a `Layup` property has no membrane field) keeps the bilinear rows in the coupling (L3). In `shell3.rs` one new
`pub(crate) fn andes_membrane(local: &[[f64; 2]; 3], am: &[[f64; 3]; 3]) -> ([f64; 81], [[f64; 9]; 3], f64)` returning the 9 x 9
ANDES OPT membrane on `(u, v, theta)` of a counter-clockwise triangle, its basic rows `Bbar(alpha_b)` and its area, built from the
existing `mean_rows` and `add_andes_higher_order` (unchanged) on an 18 x 18 scratch. `thermal_load_vector_with(coords, t,
material, dT, membrane)` (arithmetic 7; `thermal_load_vector` calls it with `Bilinear`, the same arithmetic as now). The Shell4
branch of `stresses` takes the membrane strain of arithmetic 8 for `AndesOpt`. Mass, pressure and body loads stay those of the
bilinear field (L2).

### D2. The opt-in (`mesh.rs`, `shell.rs`)

`BlockProperty::Shell` gains the field `shell4_membrane: crate::shell::Shell4Membrane` (the variant is non-exhaustive);
`BlockProperty::shell(t)` and `shell_with_membrane(t, m)` set `Bilinear`; new `BlockProperty::shell4_with_membrane(t, m)` (the
Shell3 membrane `ConstantStrain`, the same refusals). `set_block_property`: the present arms for a property whose
`shell4_membrane` is `Bilinear` (the same results and messages, ADR 0466 RF2 unchanged); a `shell4_membrane` other than
`Bilinear` on any kind but Shell4 refused (`UnsupportedKind`, operation "a Shell4 membrane"). `shell::model_membrane(mesh, e)`
(`pub(crate)`), `shell::model_element_stiffness` passes it in the options; `thermal.rs` calls `thermal_load_vector_with` with it.
A separate enum rather than `Shell3Membrane::AndesOpt` on a Shell4 block because ADR 0466's RF2 test asserts that refusal and
existing tests are not edited (open question 2).

### D3. Mixed meshes (`shell3.rs::check_membrane_edges`)

The early return now also looks at Shell4 blocks: it returns at once if no Shell3 block has a drilling membrane and no Shell4
block has a membrane other than `Bilinear` (every default model: the former path). The class of a Shell4 element is "a linear edge
(Shell4)" for `Bilinear` (the former string) and "the ANDES membrane" for `AndesOpt`, the string of the ANDES Shell3: the two
match and the edge is taken (b3); every other pairing with a drilling class is refused as before, the message unchanged in form.

### D4. Which paths

Every path that takes Shell4 (static and modal, elimination and transformation, the assembly, the mass, the loads, the thermal
load, the stresses) takes the membrane the same way; the paths that refuse Shell4 refuse it as before.

### D5. Names in the topic documentation

`docs/reference-data.md` gains an F-549 section (the sources above, NOT read; what is derived).

### D6. The bit record of the default (B1)

`crates/rcfd-fem/tests/sa_shell4_membrane_bits.rs`, using only the API of the base fc026814, run once on the base after this
commit and before any library code; its digests (FNV-1a of the bits) written into `EXPECTED` from that run: a Shell4 plate (4 x 3
cells, slightly warped, clamped, every shell load: pressure, angle-weighted pressure, gravity, centrifugal, nodal body force,
temperature) stiffness, mass, loads, solution, stresses, four frequencies; an ANDES Shell3 plate (ADR 0466's opt-in) with the same
loads: stiffness, loads, solution, stresses; the refusal of an ANDES Shell3 next to a default Shell4 (the bytes of its message);
one warped Shell4 element through `shell::element_stiffness`, `element_stiffness_with` (drilling 0; the displacement shear),
`element_mass`, `pressure_load_vector`, `body_load_vector` and `thermal_load_vector`.

## Checks (limits fixed now)

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| MP1 | a2, a3 | the mixed patch of reading (a3) (ANDES Shell3, new Shell4), membrane, through `solve_static_with`: interior unknowns | `1e-8` | arithmetic 2: `3.3e-15` (ulp `2.3e-15`) |
| MP2 | a3 | its `N` against `A_m eps` (every element, its local axes) relative to `max |N|`; `m`, `Q` relative to the same | `1e-8` | rounding (arithmetic 8) |
| MP3 | a3 | the same model, the bending patch of ADR 0454 (constant curvature), interior unknowns | `1e-8` | ADR 0466 PT6 `1.1e-15` (the bending rows unchanged) |
| MP4 | a3 | its `m` against `D_b kappa`; `Q`, `N` times the side, relative to `max |m|` | `1e-8` | ADR 0466 PT4 `1.3e-14`, `8.9e-13` |
| QP1 | a1 | all nine cells Shell4 with the new membrane (the same nodes, in space), membrane patch, interior unknowns | `1e-8` | `1.4e-15` (ulp `4.5e-15`) |
| QP2 | a1 | the same, bending patch, interior unknowns | `1e-8` | ADR 0432 `1.0e-13` |
| CQ1 | a4 | the strip of reading (a4), 8 x 2 Shell4 with the new membrane: `|u_y / delta - 1|` | `2e-2` | arithmetic 5: `4.209e-3` (discretisation `4.1e-3`, reference `1.5e-4`) |
| CQ2 | a4 | the same, end couple `M = 1 N m`: `|u_y / (M L^2 / (2 E I)) - 1|` | `2e-2` | `3.237e-3` |
| BE1 | a1 | rectangles (corner `(0.2, 0.1)`), `a / b = 0.25, 1, 4`, `nu = 0, 0.3`, both directions, one element through `shell::element_stiffness_with(.., ShellOptions::default().membrane(AndesOpt))`: `max |U / U_exact - 1|` | `1e-10` | arithmetic 4: `2.85e-14` (ulp `4.4e-16`) |
| SE1 | a1 | the warped element of arithmetic 3, t = 0.01 and 0.1: count of `|lambda| <= 1e-10 lambda_max` and `lambda_7 / lambda_max` | `6`, `>= 1e-7` | `6`, `1.74e-6`, `1.74e-4` |
| SE2 | a1 | its rigid motions in space `|K r| / (max |K| max |r|)`; `K` symmetric bit for bit | `1e-11`; exact | `2.1e-16` (ulp `3.0e-16`) |
| ISO1 | a1 | relabelled `[1, 2, 3, 0]`, `[2, 3, 0, 1]`, permuted back, against `K` relative to `max |K|` | `1e-11` | `4.5e-16` |
| FR1 | a1 | the free plate of arithmetic 6 (`stiffness(model)`, dense Jacobi): count of `|lambda| <= 1e-10 lambda_max` | `6` | `6` (the seventh `2.7e-5`) |
| TH1 | D1 | the plate of arithmetic 7 (Shell4 with the new membrane, `Load::temperature`, `dT = 50`): translations against `eps_th x` relative to `max`; membrane forces relative to `E alpha dT t` | `1e-8`; `1e-8` | `2.3e-14` |
| ST1 | D1 | one element in its plane (a single-element model), unknowns only `theta_z = (1, -2, 0.5, 0.8) 1e-3`: `shell::stresses` membrane force against `A_m eps` of the closed form of arithmetic 8 | `1e-12` relative to `max |N|` | arithmetic 8 (12 terms per component) |
| D1 | D2 | a single-element model: `stiffness(model)` equals `shell::element_stiffness_with(.., ShellOptions::default().membrane(AndesOpt))` bit for bit | exact | D2 |
| RF1 | a2, b3 | (a) MP1's model solves (`solve_static`, no refusal); refused (`Element { InvalidValue { key: "membrane" } }`, the message naming the other element): (b) a Shell4 block with the new membrane sharing an edge with a default Shell4 block, (c) with a constant-strain Shell3 block, (d) with an Allman Shell3 block; (e) ADR 0466's mixed patch (ANDES Shell3, default Shell4) still refused | exact | D3 |
| RF2 | D2 | `shell4_with_membrane(t, AndesOpt)` on a Shell3 block refused (`UnsupportedKind`, "a Shell4 membrane"); on a Shell4 block taken; `t = 0` refused (`thickness`) | exact | D2 |
| B1 | a5 | the digests of `sa_shell4_membrane_bits` equal the base run | exact | D6 |
| B2 | a5 | the existing tests pass unedited: the `rcfd-fem` test binaries `sa_shell*`, `si_shell*`, `sa_scaffold*` and the library unit tests, `rcfd-post`'s `sa_shell3_output`, `si_shell_output*`, release | pass | D2 |
| B3 | a5 | `cargo test -p rcfd-api --release --test baseline_hashes` (rcfd-api depends on rcfd-fem) | pass | no fluid file touched |

Stronger checks against the prototype (`est*.log`):

| id | check | limit | prototype |
| --- | --- | --- | --- |
| P1 | CQ1 `0.9957912994`, CQ2 `0.9967626398`; 16 x 4 `0.9981052714`; N1 `0.8838414209` | `1e-5` relative each | `est1.log` |

Negative controls and observations:

| id | control | expected | limit |
| --- | --- | --- | --- |
| N1 | the strip of CQ1 with Shell4's default bilinear membrane | stiff | `< 0.9` (`0.8838`) |
| N2 | MP1's patch with the default Shell4 (ADR 0466 PT5 again, through `assemble_with`, the model refused) | the miss the new membrane removes | `> 1e-2` (ADR 0466 PT5 `2.08e-1`) |
| O1 | CQ1 against Euler-Bernoulli `P L^3 / (3 E I)`, printed | about `1.033` | observation |
| O2 | CQ1 with `theta_z` free at the clamp, printed | `1.0470` | observation |
| O3 | the distorted 8 x 2 strip of arithmetic 5, new membrane and default, printed | `0.9726`, `0.8502` | observation |

MP, QP, CQ, BE, SE, ISO, FR, TH, ST, D, RF, P, N and O are one test file `crates/rcfd-fem/tests/sa_shell4_membrane.rs`, run once.

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU1 | the weight of each triangle `1` in place of `1/2` | MP1, CQ1, P1, BE1 |
| MU2 | one split only (`(0, 1, 2), (0, 2, 3)` with weight `1`) | ISO1, P1 (`0.99648` against `0.99579`) |
| MU3 | the quadrilateral's triangles with Allman's mean rows (`alpha = 1`) in the basic stiffness | MP1, CQ1, P1, BE1, ST1 |
| MU4 | the drilling penalty kept with the new membrane | P1 (arithmetic 5: `3.3e-5`) |
| MU5 | the bilinear membrane kept beside the new one | CQ1, BE1, P1 |
| MU6 | the model's Shell4 membrane ignored (`model_element_stiffness` with the default options) | D1, CQ1, MP1, P1 |
| MU7 | the stresses of the new membrane from the bilinear rows | ST1 |
| MU8 | the thermal load of the new membrane from the bilinear rows | TH1 |
| MU9 | `check_membrane_edges`: a Shell4 with the new membrane classed as a linear edge | MP1 (refused), RF1 (a), (b) |
| MU10 | `check_membrane_edges`: the early return ignores Shell4 blocks | RF1 (b) |
| MU11 | `set_block_property` accepts the Shell4 membrane on a Shell3 block | RF2 |
| MU12 | the default `Bilinear` takes the new branch | B1, N1, P1, the existing Shell4 tests |

## Decision rule

- **done:** MP1 to MP4, QP1, QP2, CQ1, CQ2, BE1, SE1, SE2, ISO1, FR1, TH1, ST1, D1, RF1, RF2, B1 to B3, P1 met, the controls as
  stated, every mutation caught by a registered check, no amendment after a run.
- **partial:** a check missed (recorded as a `*_known_miss`) or not run, a mutation not caught, or a check met only after an
  amendment written after its run.
- **todo:** the membrane does not build.

## Limits (not covered)

- L1. The ANDES parameters are those of ADR 0466 (recalled, verified there by BE1); `beta_0` assumes an isotropic section; a layup
  keeps the bilinear membrane (its property has no membrane field).
- L2. Mass, pressure and body loads use the bilinear field for every membrane (no drilling moments from in-plane loads), as ADR 0466
  L2.
- L3. Through the element functions only, a coupled (layup) section with the new membrane keeps the bilinear rows in the
  membrane-bending coupling; not reachable from a model.
- L4. Stresses at the centre only, from the mean basic strain; the higher-order strain is not reported.
- L5. A warped Shell4 carries the new membrane on its projected facet with the rigid offsets of ADR 0432; the patch tests here are
  flat. A drilling edge shared by two warped facets is not checked beyond SE2.
- L6. Allman's Shell3 has no quadrilateral partner here (an `alpha = 1` quadrilateral of the same construction would be its
  match); its mixed edges stay refused.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The element: four overlapping ANDES OPT triangles (taken; its edge field is ANDES's by construction, arithmetic 1) rather than
   an Ibrahimbegovic-Taylor-Wilson or MacNeal quadrilateral (a different edge moment) or a four-triangle assembly about a centre
   node condensed out (also ANDES-edged, but an internal node to condense and a choice of centre).
2. The opt-in through a separate `Shell4Membrane` field of the Shell property (taken) rather than `Shell3Membrane::AndesOpt` on a
   Shell4 block (would change the result of ADR 0466's existing RF2 test).
3. Mixed edges between the ANDES Shell3 and the new Shell4 taken (b3); every other mixed drilling pairing refused as before.

## Addendum 1: results (2026-11-14)

The registered run is commit e0eb1032 (`f549-scratch/run1.log`, the two files `sa_shell4_membrane` and
`sa_shell4_membrane_bits`), made once. B1 had not been run on any F-549 code before it; its only earlier run is the base
recording (`b1-base.log`, commit f3816792). After the run, `cargo fmt` changed the layout of F-549 lines only, and clippy asked
for one `vec!` in the test to become an array (`useless_vec`, BE1's unknown vector, no number changed); both files passed again
after each change (`after-fmt.log`, `after-clippy.log`, identical numbers), committed as e76707b2. B2 and B3 ran on e0eb1032
and e76707b2. No amendment was written.

| id | limit | measured | met |
| --- | --- | --- | --- |
| MP1 | `1e-8` | `2.12e-14` (through `solve_static_with`, the model taken) | yes |
| MP2 | `1e-8` | `1.1e-15`; m and Q `8.0e-16` | yes |
| MP3 | `1e-8` | `1.8e-15` | yes |
| MP4 | `1e-8` | `1.0e-14`; Q and N `9.5e-13` | yes |
| QP1 | `1e-8` | `2.15e-14` | yes |
| QP2 | `1e-8` | `2.9e-15` | yes |
| CQ1 | `2e-2` | `4.209e-3` (`u_y / delta = 0.9957912994`) | yes |
| CQ2 | `2e-2` | `3.237e-3` (`0.9967626398`) | yes |
| BE1 | `1e-10` | `2.6e-14` | yes |
| SE1 | `6`, `>= 1e-7` | 6, `1.738e-6` (t = 0.01); 6, `1.738e-4` (t = 0.1) | yes |
| SE2 | `1e-11`; exact | `2.8e-16`, `1.5e-16`; symmetric bit for bit | yes |
| ISO1 | `1e-11` | `3.7e-16` largest | yes |
| FR1 | `6` | 6 (`lambda_7 / lambda_max = 2.73e-5`) | yes |
| TH1 | `1e-8`; `1e-8` | `4.5e-14`; `2.0e-15` | yes |
| ST1 | `1e-12` | `2.2e-16` | yes |
| D1 | exact | bit for bit | yes |
| RF1 | exact | (a) the mixed ANDES patch taken; (b) next to a default Shell4, (c) a constant-strain Shell3, (d) an Allman Shell3 refused naming element 0; (e) ADR 0466's mixed patch still refused | yes |
| RF2 | exact | `UnsupportedKind { block: 0, kind: "Shell3", operation: "a Shell4 membrane" }`; taken on Shell4; `thickness` refused | yes |
| B1 | exact | 17 of 17 digests equal the base | yes |
| B2 | pass | the whole `rcfd-fem` suite, release: 105 test binaries and the doc-tests, 0 failed (`b2-fem-all.log`; the registered list `b2-fem.log`); `rcfd-post` `sa_shell3_output`, `si_shell_output`, `si_shell_output_bytes`, `si_shell_output_vtk` passed (`b2-post.log`) | yes |
| B3 | pass | baseline_hashes 3 passed, 1 ignored (the existing record) | yes |
| P1 | `1e-5` each | largest `4.6e-11` (N1); CQ1 `1.5e-11` | yes |
| N1 | `< 0.9` | `0.8838414209` | yes |
| N2 | `> 1e-2` | `2.0776e-1` (ADR 0466 PT5's number) | yes |
| O1 | printed | `1.0331334731` | observation |
| O2 | printed | `1.0470229102` | observation |
| O3 | printed | ANDES Shell4 `0.9726063308`, bilinear `0.8502431650` | observation |

Mutations (archive copy of e0eb1032, `f549-scratch/mutate.py`, `mutlogs/`), each caught:

| id | caught by |
| --- | --- |
| MU1 | CQ1, CQ2, MP1, MP2, BE1, TH1, P1 |
| MU2 | ISO1, P1 |
| MU3 | CQ1, CQ2, MP1, MP2, BE1, ST1, P1 |
| MU4 | P1, BE1 (the penalty is not zero on the pure bending field at the Gauss points) |
| MU5 | CQ1, CQ2, MP1, MP2, BE1, TH1, P1 |
| MU6 | D1, CQ1, CQ2, MP1, MP2, TH1, P1 |
| MU7 | ST1 |
| MU8 | TH1 |
| MU9 | RF1 (a) (the MP1 solve refused), (b), (c) |
| MU10 | RF1 (b), (c) |
| MU11 | RF2 |
| MU12 | B1 (both bit records), N1, N2, P1, and the existing `si_shell` (CY1, FR, N1, P4) |

fmt (`cargo fmt -p rcfd-fem --check`) and clippy (`cargo clippy -p rcfd-fem --all-targets --release -- -D warnings`): clean
after the change disclosed above. `rcfd-cli`'s `docs_site` test (for `docs/reference-data.md`): 2 passed (`docs-site.log`). Not run: the whole
workspace suite (the lead's gate).
Shared solver files changed (`git diff fc026814`): `mesh.rs` (the `shell4_membrane` field, `shell_with_membrane` filling it,
the new `shell4_with_membrane`, one guarded arm in `set_block_property`), `shell.rs` (the enum, the options field, the ANDES
branch of the stiffness, two helpers, `thermal_load_vector_with`, `model_membrane`, one match in `stresses`), `shell3.rs` (the
helper `andes_membrane`, the Shell4 classes in `check_membrane_edges`), `thermal.rs` (the Shell4 thermal call).

**Decision: done**, by the rule: every registered check met in the single run, every control as stated, all twelve mutations
caught by registered checks, no amendment. The acceptance's clauses: the opt-in Shell4 membrane with the ANDES edge field
(arithmetic 1), ANDES Shell3 and Shell4 share edges (RF1 (a)), the mixed membrane patch of ADR 0454 at `2.1e-14` against `1e-8`
(MP1), the 8 x 2 strip on quadrilaterals within `4.2e-3` of beam theory against `2e-2` (CQ1), defaults bit-identical (B1 to B3).

Remaining questions (options, recommendation):

1. Allman's Shell3 next to Shell4 (L6): (a) keep it refused; (b) an `alpha = 1` quadrilateral of the same construction. Recommend
   (a): Allman's membrane is kept for comparison only.
2. The default Shell4 membrane: (a) keep the bilinear default; (b) make the ANDES quadrilateral the default (changes every Shell4
   result and the hash tables; needs the maintainer). Recommend (a).
3. The coupled (layup) section with the drilling membrane (L1, L3): (a) leave it unreachable; (b) a follow-up giving `Layup` a
   membrane field with the basic rows in the coupling. Recommend (a) until a laminate needs in-plane bending accuracy.

## For the lead: roadmap entries

Proposed status of F-549: **done**.

> Structural solver, advanced tier. Follow-up of F-539 (ADR 0466, 2026-11-14): the PT5 mixed-mesh miss. F-549 (ADR 0470, 2026-11-14): an opt-in drilling membrane for Shell4 through `BlockProperty::shell4_with_membrane(t, Shell4Membrane::AndesOpt)`: the mean of the two diagonal splits into ANDES OPT triangles (the Shell3 membrane of ADR 0466), no drilling penalty; every edge carries the ANDES edge field with weight one (derived), so ANDES Shell3 and Shell4 share edges, and the refusal of mixed drilling edges is lifted for that pairing only. Met: the mixed Shell3 + Shell4 membrane patch of ADR 0454 2.1e-14 (limit 1e-8; 2.1e-1 with the default Shell4), all-Shell4 patches 2.2e-14 and 2.9e-15; 8 x 2 in-plane cantilever strip on quadrilaterals 0.99579 of Timoshenko beam theory (end couple 0.99676; default bilinear 0.884); pure bending of rectangles exact to 2.6e-14; six zero modes per element and per free plate; consistent thermal load and centre stresses; within 4.6e-11 of a numpy prototype. Default Shell4, Shell3 and the ANDES Shell3 bit-identical (17 digests), baseline_hashes unchanged; twelve mutations caught.
