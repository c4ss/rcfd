# ADR 0462: F-544 registration: cyclic symmetry with cylindrical material axes

Status: **REGISTERED 2026-11-14, written before any code of F-544 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-544 was written, compiled or run, except the bit record of the default paths (B1,
a test file using only the API of main ea76abd8, run once at that commit with no library line changed) and a rounding probe
of the isotropic set-ups (a temporary test file using only that API, deleted before this commit; both disclosed below).
Results go in the addenda at the end; nothing above the addenda is changed after a run.

Parent: [ADR 0459](0459-f542-registration-orthotropic-and-tabulated-materials-on-rotating-buckling-transient.md) (F-542:
`mat_ortho::refuse_with_cyclic`, its D3, its remaining question 1, recommendation (a), and its TR2 lesson: a 1e-12 limit can
sit below a set-up's own rounding). Grandparents: [ADR 0457](0457-f537-registration-orthotropic-and-tabulated-materials-on-modal-paths.md)
(F-537: the per-point stiffness on the modal and transformation paths, its D3 refusal of a cyclic constraint with either
variant, its question 3) and [ADR 0452](0452-f409-registration-orthotropic-elasticity-and-temperature-tables.md) (F-409: the
two variants, the cylindrical axes, `mat_ortho::FOLLOW_UP`). The cyclic paths: ADR 0386 (F-394, the sector rows, the full
rotor), ADR 0395 (F-489, the static harmonic solve), ADR 0407 (F-500, the rotating sector), ADR 0411 (F-499, the whirl
modes). Work package SA-2 of the structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)).
Roadmap item F-544 (deps F-542). Number 0462 given by the lead. Branch `f544-work` from main ea76abd8, worktree
`w49i-work/f544`, scratch `w49i-work/f544-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate
graph unchanged; no existing test edited except the records-and-copies step of the maintainer's standing decision (ADR 0291
pattern; D7); clean-room. Another agent (F-540, shell pressure) edits `rcfd-fem` concurrently: the edits of shared files are
kept to the lines named in D2.

## What F-544 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The cyclic harmonic modal analysis, the static harmonic solve and the rotating sector (modes and whirl) take the orthotropic and tabulated materials of ADR 0452 when every material with its own axes has cylindrical axes about the cyclic constraint's axis and origin; other axes stay refused by name; the harmonics of a sector match the full rotor of the same material within registered limits; isotropic constants in an orthotropic material give the isotropic results within 1e-12 on stocky models; default paths bit-identical.

Clauses: (a0) every limit registered before the run (this document); (a1) the cyclic harmonic modal analysis, the static
harmonic solve and the rotating sector (modes and whirl) take both variants when every material with its own axes has
cylindrical axes about the cyclic constraint's axis and origin; (a2) other axes stay refused by name; (a3) the harmonics of
a sector match the full rotor of the same material within registered limits; (a4) isotropic constants in an orthotropic
material give the isotropic results within 1e-12 on stocky models; (a5) default paths bit-identical. The brief of the lead
adds: (b1) derive why axes coincident with the cyclic axis make the material sector-invariant, before registering (D1);
(b2) probe the one-ulp sensitivity of each set-up of a 1e-12-class limit before registering it, and disclose the probe
(arithmetic 2).

## What was read (main ea76abd8, read only)

1. `AGENT_RULES.md`; ADR 0459 whole (D3, amendment 1, the TR2 diagnosis, remaining question 1); ADR 0457 and ADR 0452 (the
   refusal rule, the per-point stiffness, the cylindrical axes); the measured rows of ADR 0386 (E1 3.96e-14, E2 1.78e-13),
   ADR 0395 (H1 1.50e-13), ADR 0407 (C1 8.8e-14) and ADR 0411 (S1 3.0e-13) for the sector-against-full-rotor procedure.
2. Code: `crates/rcfd-fem/src/{cyclic.rs (whole), mat_ortho.rs (whole), rotating.rs (the module text, `unit_centrifugal_model`,
   `RotatingPart::new`, `with_unit_prestress`, `check_model`, `RotatingSector::new`, `with_unit_prestress`, `frame`, `modes`),
   rotating/whirl.rs (the sector entry points: `modal_analysis_harmonic` at speed 0, the part's operator otherwise), mpc.rs
   (`check_transformation_model`, `solve_static_mpc`, `modal_analysis_mpc`), assemble.rs (`stiffness_with`,
   `stiffness_with_materials`, `point_stiffness`), model.rs (`check_supported` and the `check_*` family), geometric.rs
   (`prestressed_modes` and its transformation path), thermal.rs (`temperature_field`), material.rs (`MaterialAxes`),
   mat_table.rs (the column names)}`; the tests `si_cyclic.rs` (E1 to E6), `si_cyclic_harmonic.rs` (H1, the loads and the
   full model), `si_rotating_cyclic.rs` (C1, C2, the sector D), `si_rotating_whirl.rs` (S1, the sector DS),
   `sa_ortho_rotating.rs` (whole), `sa_ortho_rotating_default_bits.rs`.
3. No paper is used. The design is derived below from the definitions of the cylindrical axes (ADR 0452) and of the cyclic
   rows (ADR 0386).

Findings that shape the design:

1. Four places refuse a cyclic constraint with either variant today: `mat_ortho::refuse_with_cyclic` (called by
   `rotating::check_model`, `RotatingSector::new` and `with_unit_prestress`, `buckling::factors` and both constructors of the
   integrator) and an inline copy of the same test in `mpc::check_transformation_model` (the entry of `solve_static_mpc` and
   `modal_analysis_mpc`, hence of the index-0 modal analysis, of `modal_analysis_with` and `solve_static` on a cyclic model,
   and of the static solve of the pre-stress at 1 rad/s of a rotating sector). Both give `NotYetSupported { item:
   "material", variant, location: "material {i}, element set '{set}', with the cyclic constraint {j}", package: FOLLOW_UP }`.
2. `cyclic::modal_analysis_harmonic` (index `k > 0`) and `cyclic::solve_static_harmonic` (every index) check
   `check_supported` (refuses both variants naming the set) and assemble `stiffness_with` (refuses them too); the index 0 of
   the modal analysis is `modal_analysis_mpc` (finding 1).
3. `solve_static_harmonic` refuses a model with any load of its own; a temperature field is a `Load::Temperature`, and a
   tabulated material needs the model's field (`mat_ortho::check_model`). Without a change the static harmonic solve could
   never take a tabulated material.
4. The rotating sector at speed uses the part's operator (`K` from `point_stiffness` since ADR 0459, the geometric stiffness
   of the unit pre-stress, `M_perp`), and at speed 0 and in the whirl analyses at speed 0 `modal_analysis_harmonic`. The
   whirl analyses of a sector use the part's operator and the Coriolis matrix (density only). Opening the entry checks
   opens them with the same matrices.
5. `geometric::prestressed_modes` on the transformation does not refuse a cyclic constraint with either variant (it never
   had the check of ADR 0457 D3); it is outside F-544 and is not changed (remaining question 2).
6. The non-ignored test `sa_ortho_rotating::r3_paths_not_opened_after_f542` asserts three things F-544 changes: the sector
   `SEC` (cylindrical axes about the cyclic axis) refused by `RotatingSector::new` and `with_unit_prestress`, refused by
   `modal_analysis_harmonic`, and `modal_analysis_harmonic` of the ADR 0452 cantilever (no cyclic constraint) refused naming
   the variant (after F-544 the entry check passes the material and the model is refused for having no cyclic constraint,
   key `"cyclic"`). It becomes a record with a copy (D7). Its other rows (a cyclic constraint with `Fixed(I)` axes) keep
   their message and pass unchanged.

## Arithmetic before this registration (disclosed)

1. **B1**: `crates/rcfd-fem/tests/sa_ortho_cyclic_default_bits.rs` was written with the API of main ea76abd8 only and run
   once at that commit (release; log `f544-scratch/b1-base.log`, the run ended at the `BASE = None` expectation after
   printing); the 23 digests it printed are its constants `BASE`.
2. **The rounding probe** (temporary test file, source `f544-scratch/probe_source.rs`, logs `f544-scratch/probe.log`,
   `probe-tol13.log`, `probe-ring.log`). On isotropic sectors (steel, `E = 2e11`, `nu = 0.3`, no expansion) the change of the
   results of the four set-up types of the 1e-12 checks under a one-ulp change of `E`, of `nu`, and of every node coordinate
   (one ulp up or down by a fixed pattern): the modal harmonic analysis (`k = 0 .. 6`, four sector frequencies each, the
   pair means), the static harmonic solve (`k = 0 .. 6`, `max |u' - u| / max |u|` over both parts), the rotating sector at
   half the lowest rest frequency (`k = 0 .. 6`) and the dense sector whirl (6 sectors, `k = 0 .. 3`, at 0.4 of the lowest
   rest whirl frequency). Worst over the three perturbations:

   | set-up | modal harmonic | static harmonic | rotating sector | sector whirl |
   | --- | --- | --- | --- | --- |
   | disc D (`r` 0.25 to 1, `h` 0.2, Hex20 3 x 2 x 1; whirl DS 2 x 1 x 1) | 2.2e-13 | 1.7e-13 | 1.7e-13 | 1.7e-13 |
   | the same with the eigensolver tolerance 1e-13 | 2.2e-13 | | 1.7e-13 | 1.7e-13 |
   | ring SR (`r` 0.5 to 1, `z` -0.25 to 0.25, Hex20 2 x 2 x 2; whirl SR6 2 x 1 x 2) | 6.2e-15 | 1.1e-14 | 7.1e-15 | 2.2e-14 |

   On the thin disc a one-ulp change moves the results by up to 2.2e-13, within a factor 5 of 1e-12, and the isotropic
   constants in an orthotropic material with cylindrical axes differ from the isotropic material by a few ulps at every
   point (the compliance inverted by cofactors, the rotation `T^T C T`): the disc is not a safe set-up for a 1e-12 limit
   (the TR2 lesson). The stocky ring of square section moves by at most 2.2e-14, 45 times below 1e-12; the checks of clause
   (a4) are registered on it (D8). The tolerance of the eigensolver is not the cause (the second row).
3. **The axes rows** (by hand, D1): with the cyclic axis `n = (0, 0, 1)` through the origin, cylindrical axes about `(0, 0,
   -1)` or through `(0, 0, 0.3)` give at every point the bits of the radial direction (`r = d - (d . e_3) e_3` is `(d_x, d_y,
   0)` exactly) and the tangential and axial rows negated exactly or unchanged; the global elasticity `T^T C T` is then equal
   as numbers (D1, the A1 check).

No other run was made.

## Design

Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation); none changes a default result, a golden or a hash table.

### D1 Why cylindrical axes about the cyclic axis make the material sector-invariant (b1)

Let `n` be the unit cyclic axis through `o`, and `Q` the rotation by `phi` about `n` (so `Q n = n`, `Q (I - n n^T) = (I - n
n^T) Q`). The cylindrical axes of ADR 0452 about the same line at a point `x` are the rows `e_r(x) = P (x - o) / |P (x -
o)|` (`P = I - n n^T`), `e_t(x) = n x e_r(x)`, `e_z = n`. At the image `y = o + Q (x - o)`: `P (y - o) = Q P (x - o)` and
`|Q v| = |v|`, so `e_r(y) = Q e_r(x)`; `e_t(y) = n x Q e_r(x) = Q n x Q e_r(x) = Q (n x e_r(x)) = Q e_t(x)` (a rotation
preserves the cross product); `e_z = n = Q n`. With `A(x)` the matrix of the rows, `A(y) = A(x) Q^T`. The global elasticity
tensor is `c_ijkl(x) = A_ai A_bj A_ck A_dl C_abcd` (`C` the constants in material axes), so

`c_ijkl(y) = Q_ip Q_jq Q_kr Q_ls c_pqrs(x)`:

the tensor at the image point is the rotated tensor. The element of sector `j` is the element of sector 0 with nodes `y =
o + Q^j (x - o)`; with the rotated tensor its stiffness in global components is `blockdiag(Q^j) K_e blockdiag(Q^j)^T`
(objectivity of `int B^T D B`: the gradient of a rotated field at the rotated point is `Q grad Q^T`). The full rotor's
stiffness is therefore block-circulant in the sector's own components, which is the assumption of the cyclic reduction
(ADR 0386: the rows `u_H = R (cos theta u_L - ...)` with the same sector `K` in every sector). The mass, `M_perp` about the
cyclic axis and the Coriolis matrix need the density only. The centrifugal load at 1 rad/s about the cyclic axis rotates
with the sector, so its index-0 solution and its stress do (`sigma(y) = Q sigma(x) Q^T`), and so the geometric stiffness.
A tabulated material takes its stiffness at the model's temperature field; on a sector model the field is that of every
sector (the full rotor is defined as the copies), so it is sector-invariant whatever its shape, and a table on an isotropic
base has no axes at all.

The other axes are not: with `Fixed(A)` axes `c(y) = c(x)`, which equals the rotated tensor only if `c` is invariant under
`Q` (transversely isotropic about `n`); with cylindrical axes about another line `e_r(y) != Q e_r(x)`; `PerElement` axes
list one triad per sector element, and the rotated copies would need the rotated triads, which the list does not state. A
sector model with such axes describes no rotor whose sectors are copies of it with the material as given; the result would
depend on which reading the user meant, so they are refused (a2). Two sign variants are the same material: cylindrical
axes about `-n` have `e_t` and `e_z` negated, a rotation by pi about `e_r`, under which an orthotropic `C` (with its three
planes of symmetry) is unchanged; an origin anywhere on the line gives the same `e_r`.

**Coincidence test.** A material with its own axes is accepted with a cyclic constraint if its rule is `Cylindrical {
origin, axis }` with `|n x m| <= CYCLIC_AXES_TOLERANCE` (`m` the unit material axis, either sign) and the distance of
`origin` from the cyclic line `|P (origin - o)| <= CYCLIC_AXES_TOLERANCE L`, `L` the largest distance of a mesh node from the
cyclic line; `CYCLIC_AXES_TOLERANCE = 1e-10` (a new public constant of `mat_ortho`). A tilt or an offset `delta` changes the
frame by about `delta` and the results by about `delta` (relative), so 1e-10 lies two orders below the 1e-8 limits of the
sector-against-full-rotor checks and far above the rounding of an axis given as `(0, 0, 1)`. The test applies to every
material of either variant with its own axes (an `Orthotropic` material and a `Tabulated` one on an orthotropic base); a
table on an isotropic base and an isotropic material have none. It is checked against every cyclic constraint of the model
(the cyclic paths accept exactly one; the transformation path may meet more).

### D2 The code (shared files: the lines named here only)

- `mat_ortho.rs`: `pub const CYCLIC_AXES_TOLERANCE`, and `pub(crate) fn check_cyclic_axes(model)`: `Ok` for a model without
  a cyclic constraint or without either variant; otherwise the first material of either variant whose axes fail the test of
  D1 is refused with exactly the error of ADR 0457 D3 and ADR 0459 D3 (`NotYetSupported { item: "material", variant,
  location: "material {i}, element set '{set}', with the cyclic constraint {j}", package: FOLLOW_UP }`, `j` the first cyclic
  constraint it fails against). `refuse_with_cyclic` is kept unchanged for the paths that stay refused (D6).
- `mpc.rs`, `check_transformation_model`: the inline refusal of ADR 0457 D3 (about 20 lines) becomes the call of
  `check_cyclic_axes`. For a model without the variants it returns `Ok` exactly where the old test did not refuse, and for
  a refused one the same error.
- `cyclic.rs`, `modal_analysis_harmonic` and `solve_static_harmonic`: for a model with either variant the entry check is
  `check_supported_solids_with_materials` (ADR 0457 D2) then `check_cyclic_axes`, and the stiffness `point_stiffness`; for a
  model without them `check_supported` and `stiffness_with` as before (the same bits, the same errors).
- `cyclic.rs`, `solve_static_harmonic`, the loads (finding 3): for a model with a `Tabulated` material a `Load::Temperature`
  is not counted as a load of its own: it gives the stiffness of the table at each point (ADR 0457 D4) and **no thermal
  load** (a harmonic load is given as its cosine and sine vectors; the thermal load of the field at index 0 is
  `loads::nodal_load_vector` of the model, to be added to the cosine part by the caller, check S3). Any other load is
  refused as before; a model without a `Tabulated` material is refused for its temperature field as before (same message).
- `rotating.rs`: `check_model` and `RotatingSector::new` and `with_unit_prestress` call `check_cyclic_axes` in place of
  `refuse_with_cyclic` (the same position, before `frame`). The unit centrifugal model of ADR 0459 D2 (temperature field,
  no expansion) is unchanged; its static solve on the index-0 rows reaches `check_transformation_model`, opened above.

For a model without either variant every changed line takes the old branch or returns `Ok` where the old code passed: the
same bits (B1) and the same errors.

### D3 What comes along

The index-0 paths on the transformation (`modal_analysis_mpc`, `solve_static_mpc`, `modal_analysis_with` and `solve_static`
of a cyclic model, also with the thermal load of a table's expansion, which is periodic) and `RotatingPart` of a cyclic
model about any axis take both variants under the same test: they share `check_transformation_model` and
`rotating::check_model`. The whirl analyses of a sector (dense, sparse, state) take them with the part's matrices (finding
4); W1 and I4 read the dense one.

### D4 The full rotor of a sector with a per-point material

`cyclic::full_rotor` copies the mesh and the sets, not the materials; the full model is built by the caller with the same
material on the same set, which for cylindrical axes about the cyclic axis is the material of every copy (D1). For a
tabulated material the full field is the sector field copied, node `rotor.node(j, p)` taking the value of sector node `p`
(a node shared by two sectors takes the same value: the fields used depend on `r` and `z` only). No library helper is added.

### D5 The isotropic checks on stocky models (a4, b2)

On the ring SR (arithmetic 2) the isotropic constants in an orthotropic material with cylindrical axes about the cyclic axis
(`ISO_C`) against the isotropic material, on each opened path. The probe bounds the rounding sensitivity of these set-ups by
2.2e-14 per ulp; `ISO_C` differs from the isotropic elasticity by a few ulps per point, so about 1e-13 is expected.

### D6 Paths not opened

With a cyclic constraint and either variant (also coaxial cylindrical axes) `buckling::factors` and the transient integrator
keep `refuse_with_cyclic` (ADR 0459 D3): they are not in the acceptance and have no sector-against-full-rotor check here.
Nonlinear, harmonic with the static correction, direct harmonic, damped modes, the public stiffness and geometric stiffness,
beams and shells refuse both variants as before. `prestressed_modes` is unchanged (finding 5).

### D7 Records and copies (the standing decision)

`sa_ortho_rotating::r3_paths_not_opened_after_f542` becomes an ignored record citing ADR 0291 and ADR 0462. Its copy
`r3_paths_not_opened_after_f544` (added to the same file, reusing its helpers) has every row of the original with three
changed: the cantilever's `cyclic::modal_analysis_harmonic` is refused with key `"cyclic"` (no cyclic constraint; the
material passes); `SEC`'s `RotatingSector::new` and `with_unit_prestress` solve; `SEC`'s `modal_analysis_harmonic` at
index 1 solves. Nothing else in an existing test changes.

### D8 Models of the checks

- Materials: `ISO` isotropic `E = 2e11`, `nu = 0.3`, `rho = 7850`, `alpha = 0`. `ISO_C` the same constants as an orthotropic
  material (`G = E/(2 (1 + nu))`) with `Cylindrical { origin: (0, 0, 0), axis: (0, 0, 1) }`. `ORTHO_C` = `E = (1.4e11, 1e10,
  1e10)`, `nu = (0.3, 0.3, 0.45)`, `G = (5e9, 5e9, 3.5e9)`, `rho = 1600`, `alpha = 0`, the same axes (1 radial, 2
  tangential, 3 axial: the fibres along the span of a fan sector). `TAB_O` = `Tabulated(ORTHO_C, [200, 600] K, e_1 = [1.4e11,
  1.0e11], g_12 = [5e9, 4e9])` with the field `T = 293 + 250 (r - 0.25)/0.75 + 200 z`, `r = sqrt(x^2 + y^2)`, reference 293 K
  (on D: 273 to 563 K). `TAB_OA` = `TAB_O` with the base expansion `(2e-5, 3e-5, 1e-5)`. `TAB_U` = `Tabulated(ISO, [200, 600]
  K, e = [2e11, 1.6e11])` with a uniform `T = 443`, `ISO(E(443))` the isotropic material with `E = interpolate([200, 600],
  [2e11, 1.6e11], 443)`.
- **Disc D** (ADR 0395): the sector `r in [0.25, 1]`, `phi in [0, 2 pi/12]`, `z in [-0.1, 0.1]`, Hex20 3 x 2 x 1
  (`upgrade_to_hex20_mapped` of `hex_box` in `(r, phi, z)`), `xmin` (the inner face) clamped, `Cyclic("ymin", "ymax", (0, 0,
  1), (0, 0, 0), 12)`. **DS** (ADR 0411): the same with 6 sectors and Hex20 2 x 1 x 1. The full rotor of each by
  `full_rotor`, `xmin` clamped, the material on `"all"`.
- **Ring SR**: `r in [0.5, 1]`, `z in [-0.25, 0.25]`, 12 sectors, Hex20 2 x 2 x 2, `xmin` clamped, the cyclic constraint as
  D. **SR6**: 6 sectors, Hex20 2 x 1 x 2.
- Modal options: the sector-against-full-rotor checks take the options of ADR 0386 (`ModalOptions::new(4).tolerance(1e-9)`,
  400 iterations; the full model `n_c + 8` modes); the isotropic and table checks `ModalOptions::new(4)` with
  `ResidualTest::Backward` and tolerance 1e-12 (the probe's options).
- Harmonic loads (ADR 0395 H1, `parts` of `si_cyclic_harmonic.rs`): cosine part a pressure 1e6 Pa on `xmax` and two nodal
  forces at the first low and high boundary nodes, sine part a traction `(2e5, -1e5, 5e5)` on `zmax` and a nodal force at
  the node nearest `(0.6 cos(alpha/2), 0.6 sin(alpha/2), 0.1)` (zero sine part at `k = 0` and `N/2`). On SR the same
  construction (the interior target `0.75` in place of `0.6`).
- Speeds: `omega_ref` the lowest sector frequency at rest (rad/s) of the comparison material (`ISO`, `ISO(E(443))`, or the
  model itself for the F checks), computed in the test; for the whirl the lowest rest whirl frequency.

## Checks

Relative frequencies `|f_a/f_b - 1|` (`f = sqrt(lambda)`); displacements `max |u_a - u_b| / max |u_b|`.

| Check | Clause | Quantity | Limit | Estimate |
| --- | --- | --- | --- | --- |
| F1 | a3, a1 | D with `ORTHO_C`, every `k = 0 .. 6`: the procedure of ADR 0386 E2 (sector eigenvalues below the cut-off `0.999 min_k max lambda_k`, pairs as two, against the full rotor's `modal_analysis`): count equal; `max |f_s/f_f - 1|`; pair splits of `k = 1 .. 5`; the residual `|K x - lambda M x| / |K x|` of each expanded mode on the full rotor's `K` (from `RotatingPart::operator(0)` of the full model) and `M`, free rows | equal; `<= 1e-8`; `<= 1e-8`; `<= 1e-7` | 1e-12 (the same discrete problem by D1; measured on the isotropic disc 1.8e-13, ADR 0386) |
| F1t | a3, a1 | the same with `TAB_O` and its field (the full field copied, D4) | as F1 | as F1 |
| F2 | a3, a1 | D with `ORTHO_C`, `k = 0 .. 6`, the loads of D8: `solve_static_harmonic` expanded (`expand_field`) against `solve_static_mpc` of the full rotor under `expand_load` | `<= 1e-8` | 1e-12 (ADR 0395 H1 measured 1.5e-13) |
| F2t | a3, a1 | the same with `TAB_O` and its field (no expansion: the full model's thermal load is zero) | `<= 1e-8` | as F2 |
| F3 | a3, a1 | D with `ORTHO_C`: `RotatingSector::new` against `RotatingPart::new` of the full rotor about the cyclic axis at `0`, `0.5`, `0.9 omega_ref`: count equal and `max |f_s/f_f - 1|` (ADR 0407 C1); the Rayleigh quotient of each expanded sector mode on the full `K_eff` and `M` against its eigenvalue (C2) | equal; `<= 1e-8`; `<= 1e-8` | 1e-12 (ADR 0407 C1 8.8e-14) |
| F3t | a3, a1 | the same with `TAB_O` and its field | as F3 | as F3 |
| W1 | a3, a1 | DS with `ORTHO_C`, `coriolis(true)`: the union over `k = 0 .. 3` of the sector's `frequencies_all` at `0.4` and `0.8 omega_ref` against the full rotor's (`RotatingPart::whirl_modes`, dense): count equal, `max |omega_s/omega_f - 1|` (ADR 0411 S1) | equal; `<= 1e-8` | 1e-12 (S1 measured 3.0e-13) |
| I1 | a4 | SR: `modal_analysis_harmonic`, `k = 0 .. 6`, the four `values`, `ISO_C` against `ISO`, max relative frequency | `<= 1e-12` | 1e-13 (D5, the probe 6.2e-15 per ulp) |
| I2 | a4 | SR: `solve_static_harmonic`, `k = 0 .. 6`, the loads of D8, `ISO_C` against `ISO`, both parts | `<= 1e-12` | 1e-13 (probe 1.1e-14) |
| I3 | a4 | SR: `RotatingSector::modes(k, 0.5 omega_ref)` (`omega_ref` of `ISO`), `k = 0 .. 6`, the four `values`, `ISO_C` against `ISO` | `<= 1e-12` | 1e-13 (probe 7.1e-15) |
| I4 | a4 | SR6, `coriolis(true)`: `RotatingSector::whirl_modes(k, 0.4 omega_ref)`, `k = 0 .. 3`, the four `omega`, `ISO_C` against `ISO` | `<= 1e-12` | 2e-13 (probe 2.2e-14) |
| T1 | a1 | SR (and SR6 for the whirl) with `TAB_U` against `ISO(E(443))`: I1, I2 (the uniform field in the model, no thermal load), I3 and I4 (`omega_ref` of `ISO(E(443))`), and the unit pre-stress of the rotating sector (`max |s_a - s_b| / max |s_b|`) | `<= 1e-12` each | 0 (the same operations at every point; ADR 0459 RP1 measured 0) |
| S3 | a1 (D2) | D with `TAB_OA` and its field: `solve_static_harmonic` at `k = 0` with the cosine part `nodal_load_vector` of the model (its thermal load) against `solve_static` of the model (the transformation path with the thermal load) | `<= 1e-10` | 1e-14 (the same system on two paths) |
| A1 | a1 (D1) | D: `ORTHO_C` with the axis `(0, 0, -1)`, and with the origin `(0, 0, 0.3)`, against `ORTHO_C`: `modal_analysis_harmonic` at `k = 1` (eigenvalues and modes) and `solve_static_harmonic` at `k = 1` | equal as numbers (`==`, so `-0 = +0`) | equal (arithmetic 3) |
| A2 | a1 (D1) | D: `ORTHO_C` with the axis `(0, 1e-11, 1)` (inside the tolerance): `modal_analysis_harmonic` at `k = 1` solves | solves | solves |
| R1 | a2 | D with each of `ORTHO` with `Fixed(I)`; `ORTHO` cylindrical about `(0, 0.1, 1)` through the origin; about `(0, 1e-9, 1)` (outside the tolerance); about `(0, 0, 1)` through `(0.05, 0, 0)`; `PerElement` (one identity per element); `Tabulated` on `ORTHO` with `Fixed(I)` and the field of `TAB_O`: `modal_analysis_harmonic` at `k = 0` and `k = 1`, `solve_static_harmonic` at `k = 0` and `k = 1`, `RotatingSector::new`, `RotatingSector::with_unit_prestress`, `modal_analysis_with`, `solve_static` (a centrifugal load), `RotatingPart::new` about the cyclic axis: each refused `NotYetSupported { item: "material", variant, location: "material 0, element set 'all', with the cyclic constraint {j}", package: FOLLOW_UP }` | as stated (54 rows) | as stated |
| R2 | D6 | D with `ORTHO_C`: `buckling::factors` and `Integrator::new` refused as R1 | as stated | as stated |
| R3 | D2 | `solve_static_harmonic` of D with `ISO` and a temperature field, and of D with `ORTHO_C` and a temperature field (no table): refused, key `"loads"`, the message of main | as stated | as stated |
| R4 | D7 | `r3_paths_not_opened_after_f544` (the copy of D7) | as stated | as stated |
| B1 | a5 | `sa_ortho_cyclic_default_bits.rs` (23 digests of main ea76abd8: the modal harmonic analysis at `k = 0, 1, 6` (eigenvalues, modes), the static harmonic solve at the same indices, `modal_analysis_with` and `solve_static` (centrifugal and thermal) of the sector on the transformation, the rotating sector (the unit pre-stress, the modes at 300 rad/s at `k = 0, 1, 6`, a caller's pre-stress at `k = 1`), the dense sector whirl at `k = 0, 1, 3`) against `BASE`; and the existing `sa_ortho_rotating_default_bits`, `sa_ortho_modal_default_bits`, `sa_ortho_default_bits`, `sa_ortho_rotating` (with D7), `sa_ortho_modal`, `sa_ortho`, `sa_scaffold`, `sa_scaffold_bits`, `si_cyclic*`, `si_rotating*` and `sa_tie*` tests pass | `==` | `==` |

### Negative controls

| Control | Quantity | Expected |
| --- | --- | --- |
| NC1 | the full rotor of D with `ORTHO` and `Fixed(I)` axes (the reading of a refused sector as global fixed axes) against the sector harmonics of `ORTHO_C` (F1): the sorted lists, the first `n_c`, max relative frequency | `> 1e-2` (`E_1/E_2 = 14` along x everywhere instead of along the radius) |
| NC2 | the full rotor of D with `ORTHO` cylindrical about `(0, 0, 1)` through `(0.05, 0, 0)` against the sector harmonics of `ORTHO_C` (F1), the same quantity | `> 1e-4` (the frame turned by up to 0.2 rad at the bore; the off-axis modulus changes by tens of percent there) |
| NC3 | the full rotor of D with `ORTHO` and `Fixed(I)` under the expanded load of F2 at `k = 1` against the expanded sector solution of `ORTHO_C` | `> 1e-2` |

### Planned mutations (scratch copy `w49i-work/f544-scratch/mut`, the test file `sa_ortho_cyclic` and the copy R4)

| Mutation | Expected to catch |
| --- | --- |
| Mu1 | `check_cyclic_axes` returns `Ok` | R1 |
| Mu2 | the coincidence test ignores the origin | R1 (the offset origin) |
| Mu3 | the coincidence test ignores the direction | R1 (the tilted axes) |
| Mu4 | the origin compared by its full distance from `o` (not from the line) | A1 (the origin `(0, 0, 0.3)` refused) |
| Mu5 | the direction compared with its sign (`n . m > 0` required) | A1 (the axis `(0, 0, -1)` refused) |
| Mu6 | `solve_static_harmonic` adds the thermal load of the field to the cosine part | S3 |
| Mu7 | `solve_static_harmonic` accepts a temperature field without a `Tabulated` material | R3 |
| Mu8 | `modal_analysis_harmonic` at `k > 0` assembles `stiffness_with` for a per-point material | F1, I1, R4 |

## Decision rule

- **done**: every check F1 to F3t, W1, I1 to I4, T1, S3, A1, A2, R1 to R4 and B1 met in its single registered run, NC1 to NC3
  as expected, and every mutation caught by at least one registered check.
- **partial**: any registered check missed (kept as a `*_known_miss` record with its limit), a mutation not caught, or a check
  met only after an amendment made after its run.
- **todo**: the cyclic harmonic paths not opened.

## Limits (what is not covered)

- Only cylindrical axes about the cyclic axis are taken; `Fixed` axes (also a material transversely isotropic about the
  axis), cylindrical axes about another line and `PerElement` axes stay refused (D1).
- Buckling and the transient integrator with a cyclic constraint keep refusing both variants (D6); `prestressed_modes` with a
  cyclic constraint is unchanged (finding 5, remaining question 2). Nonlinear, harmonic with the static correction, direct
  harmonic, damped modes, beams and shells refuse the variants.
- The static harmonic solve takes a table's temperature field as the stiffness state only; its thermal load is the caller's
  (D2). A temperature field that is not the same in every sector (a harmonic field) is not provided.
- No library helper expands a nodal field to the full rotor (D4). The extended boundaries of ADR 0456 (axis nodes,
  interpolated nodes) are not part of the checks (`full_rotor` refuses them).

## Addendum 1: results (2026-11-14)

Registered runs at commit 5daba55b (library code and tests committed before any run, rustfmt applied before the commit),
release profile, each file run once: `sa_ortho_cyclic_default_bits` (log `f544-scratch/run-b1.log`), `sa_ortho_cyclic` (log
`f544-scratch/run-sa_ortho_cyclic.log`, 7 tests, `--test-threads=2`) and `sa_ortho_rotating` with the copy of D7 (log
`f544-scratch/run-r4.log`). No amendment was made. Then the existing tests that cover the changed lines (the `rcfd-fem`
lib tests and 49 test files: every file reaching the cyclic paths, the rotating part and sector, the transformation path of
`check_transformation_model` or the per-point materials, among them `si_cyclic*`, `si_rotating*`, `si_mpc`, `si_modal_mpc`,
`si_prestress_mpc`, `si_transient_mpc`, `sa_tie*`, `sa_joint*`, `sa_shell3`, `sa_ortho`, `sa_ortho_modal*`, `sa_mat_table`,
`sa_scaffold*`, `sa_ortho_default_bits`, `sa_ortho_rotating_default_bits`): 50 result blocks, 312 passed, 0 failed, 76
ignored (log `f544-scratch/run-fem-covering.log`).

| Check | Limit | Measured | Met |
| --- | --- | --- | --- |
| F1 `ORTHO_C`, D: count; frequencies; pair splits; expanded residual | equal; `<= 1e-8`; `<= 1e-8`; `<= 1e-7` | 33 = 33; 3.62e-14; 3.47e-14; 1.77e-11 | yes |
| F1t `TAB_O` with its field, the same | as F1 | 31 = 31; 6.45e-14; 3.96e-14; 9.94e-13 | yes |
| F2 `ORTHO_C`, D, `k = 0 .. 6`: `e_u` | `<= 1e-8` | 6.20e-14 (largest, `k = 2`) | yes |
| F2t `TAB_O` with its field | `<= 1e-8` | 1.34e-13 (largest, `k = 0`) | yes |
| F3 `ORTHO_C` rotating sector at 0, 0.5, 0.9 `omega_ref` (`omega_ref` = 1355.887 rad/s): count; frequencies; Rayleigh quotients | equal; `<= 1e-8`; `<= 1e-8` | 33 = 33 at each speed; 3.62e-14, 2.50e-14, 6.0e-15; 5.43e-14, 7.91e-14, 2.07e-14 | yes |
| F3t `TAB_O` with its field (`omega_ref` = 1309.344 rad/s) | as F3 | 31 = 31 at each speed; 6.45e-14, 3.42e-14, 6.02e-14; 5.12e-14, 7.35e-14, 1.30e-13 | yes |
| W1 `ORTHO_C`, DS whirl at 0.4, 0.8 `omega_ref` (1361.292 rad/s): count; frequencies | equal; `<= 1e-8` | 252 = 252 at both; 5.82e-14, 5.88e-14 | yes |
| I1 SR modal harmonic, `ISO_C` against `ISO` | `<= 1e-12` | 4.0e-15 | yes |
| I2 SR static harmonic | `<= 1e-12` | 5.7e-15 | yes |
| I3 SR rotating sector at 0.5 `omega_ref` | `<= 1e-12` | 2.9e-15 | yes |
| I4 SR6 sector whirl at 0.4 `omega_ref` | `<= 1e-12` | 1.75e-14 | yes |
| T1 `TAB_U` against `ISO(E(443))`: modal; static; rotating; unit pre-stress; whirl | `<= 1e-12` each | 0; 0; 0; 0; 0 | yes |
| S3 `TAB_OA`, `k = 0` with the thermal load against `solve_static` (max `|u|` 2.41e-3 m) | `<= 1e-10` | 0 | yes |
| A1 axis `(0, 0, -1)`; origin `(0, 0, 0.3)`: modal and static at `k = 1` | equal as numbers | equal; equal | yes |
| A2 axis `(0, 1e-11, 1)` solves | solves | solves | yes |
| R1 six materials, nine entries | refused as stated | 54 of 54 | yes |
| R2 buckling and the integrator with `ORTHO_C` | refused as stated | 2 of 2 | yes |
| R3 a temperature field without a table, `ISO` and `ORTHO_C` | refused, key `"loads"` | 2 of 2 | yes |
| R4 `r3_paths_not_opened_after_f544` | as stated | 31 of 31 rows | yes |
| B1 23 digests of main ea76abd8 | `==` | 23 of 23 | yes |
| NC1 full rotor with `Fixed(I)` against the sector of `ORTHO_C` | `> 1e-2` | 0.985 | as expected |
| NC2 full rotor with the axes through `(0.05, 0, 0)` | `> 1e-4` | 2.33e-2 | as expected |
| NC3 static full rotor with `Fixed(I)` at `k = 1` | `> 1e-2` | 0.789 | as expected |

Reading. A sector of the orthotropic material with cylindrical axes about the cyclic axis gives the spectrum of the full
rotor of the same material to 3.6e-14 (33 eigenvalues below the cut-off, every harmonic index), its static harmonic
solutions to 6.2e-14, its rotating modes up to 0.9 of the lowest frequency to 7.9e-14 and its whirl frequencies (252 of
them) to 5.9e-14: the same discrete problem, as D1 derives, at the rounding measured for the isotropic disc by ADRs 0386,
0395, 0407 and 0411. A tabulated material on the same axes, with a temperature field that varies over the radius and the
height, does the same (1.3e-13 at most). Read the same sector as global fixed axes, and the full rotor differs by 98 percent
in frequency and 79 percent in displacement (NC1, NC3); move the axes 0.05 off the cyclic axis and it differs by 2.3 percent
(NC2): the coincidence test refuses what would otherwise be a different rotor. On the stocky ring the isotropic constants in
cylindrical axes reproduce the isotropic results to at most 1.8e-14 on the four paths, as the probe predicted (2.2e-14 per
ulp); a uniform table gives the bits of the isotropic material at `E(443)`. The thermal load of a table's expansion given
in the cosine part at `k = 0` gives the bits of the static solve with the field (S3), so taking the field as the state and not
the load (D2) loses nothing at `k = 0`.

### Mutations (scratch copy `f544-scratch/mut` of commit 5daba55b, script `f544-scratch/mutate.py`, `sa_ortho_cyclic` and the copy R4 per mutation, logs `f544-scratch/mut_Mu*.log`)

| Mutation | Caught by |
| --- | --- |
| Mu1 `check_cyclic_axes` returns `Ok` | R1 (54 rows: the sector solves or fails later), R4 (the `Fixed(I)` cyclic rows) |
| Mu2 the origin ignored | R1 (the nine rows of the origin `(0.05, 0, 0)`) |
| Mu3 the direction ignored | R1 (the 18 rows of the tilts `(0, 0.1, 1)` and `(0, 1e-9, 1)`) |
| Mu4 the origin by its full distance | A1 (the origin `(0, 0, 0.3)` refused) |
| Mu5 the direction with its sign | A1 (the axis `(0, 0, -1)` refused) |
| Mu6 the thermal load added to the cosine part | S3 (1.0 against 1e-10: the thermal displacement doubled) |
| Mu7 a temperature field accepted without a table | R3 (both rows) |
| Mu8 `stiffness_with` at `k > 0` for a per-point material | F1, F3 (the rest modes refused), I1, A1, R4 (the sector's `modal_analysis_harmonic`) |

Eight of eight caught.

### Deviations and what was not run

- None in the registered runs: every check ran once at 5daba55b and was met; no amendment.
- Under-specified in D8 and fixed in the code before the run, recorded here: the interior target of the sine-part nodal
  force on SR is on the top face `z = 0.25` (D has `z = 0.1`, its top face), at radius 0.75.
- The probe of arithmetic 2 was a temporary test file, deleted before the registration commit; its source and logs are in
  the scratch folder.
- Not run: the whole workspace (the lead's gate); `baseline_hashes` of `rcfd-api` (no `rcfd-fv` or `rcfd-api` source changed;
  the default paths of `rcfd-fem` are covered by B1 and the bit records of ADRs 0452, 0457 and 0459, which pass); the
  `docs_site` test (no documentation page changed). `cargo fmt -p rcfd-fem --check` and `cargo clippy -p rcfd-fem
  --all-targets -- -D warnings` pass (logs `f544-scratch/fmt.log`, `clippy.log`).

### Decision by the registered rule

**Done**: every registered check (F1 to F3t, W1, I1 to I4, T1, S3, A1, A2, R1 to R4, B1) was met in its single registered
run, NC1 to NC3 came out as expected, no check needed an amendment, and every mutation was caught. Clause (a4) holds with a
45-fold margin on the stocky ring chosen from the probe. On the thin disc D a one-ulp change moves the same quantities by
2.2e-13, which is why the ring was chosen (arithmetic 2); the disc carries the sector-against-full-rotor checks.

### Remaining questions

1. Buckling and the transient integrator with a cyclic constraint and coaxial cylindrical axes stay refused (D6). Options: (a)
   open them through `check_cyclic_axes` with a sector-against-full-rotor check of their own (index 0 only: the buckling
   factors and the transient of a load that repeats in every sector); (b) leave them refused. Recommendation (b) until a fan
   case needs them: the fan rotor is analysed by its harmonics and its rotating modes, both opened.
2. `prestressed_modes` on the transformation accepts a cyclic constraint with either variant and any axes (finding 5, a gap
   since ADR 0457). Options: (a) a small follow-up adds `check_cyclic_axes` there (other axes refused by name, coaxial axes
   kept with a sector-against-full-rotor check of the pre-stressed modes); (b) leave it. Recommendation (a): today a sector
   with `Fixed` axes gives pre-stressed modes of no actual rotor, without a warning.
3. `PerElement` axes on a sector (D1). Options: (a) accept them as the triads of sector 0 with the copies rotated, the full
   rotor's caller rotating them; (b) keep refusing. Recommendation (b): the reading is ambiguous and no current case needs it.

## For the lead: roadmap entries

Proposed status of F-544: **done**.

> Structural solver, advanced tier. ADR 0462, 2026-11-14: the cyclic harmonic modal analysis, the static harmonic solve and the rotating sector (modes and whirl), with the index-0 paths on the transformation, take the orthotropic and tabulated materials of ADR 0452 when every material with its own axes has cylindrical axes about the cyclic constraint's axis and origin (either sense, within 1e-10; derived: such axes make the material the same in every sector); Fixed, PerElement and other cylindrical axes are refused by name on each of these paths (54 rows), and buckling and the transient integrator keep refusing a cyclic constraint with either variant; a table's temperature field on the static harmonic solve gives its stiffness, not a thermal load. Sector against full rotor (disc of 12 sectors, E_r/E_theta = 14): harmonics 3.6e-14 (33 eigenvalues), static harmonic solutions 6.2e-14, rotating modes up to 0.9 of the lowest frequency 7.9e-14, whirl frequencies 5.9e-14 (252); with a table and a temperature field over radius and height 1.3e-13; the same sector read as fixed axes differs by 98 percent. Isotropic constants in cylindrical axes on a stocky ring (chosen by a one-ulp probe, 2.2e-14 per ulp; the thin disc moves 2.2e-13 per ulp): modal, static, rotating and whirl within 1.8e-14; a uniform table gives the bits of the isotropic material. Default paths bit-identical (23 digests of main ea76abd8). Eight of eight mutations caught. Files mat_ortho.rs, cyclic.rs, rotating.rs, mpc.rs; tests sa_ortho_cyclic.rs, sa_ortho_cyclic_default_bits.rs; sa_ortho_rotating r3_paths_not_opened_after_f542 recorded and copied.

Note of F-542 (sentence to append):

> Follow-up F-544 (ADR 0462, 2026-11-14) opened the cyclic harmonic paths and the rotating sector to cylindrical axes about the cyclic axis (remaining question 1); buckling and the transient integrator with a cyclic constraint stay refused.

Note of F-537 (sentence to append):

> Question 3 answered by F-544 (ADR 0462, 2026-11-14): a cyclic constraint with cylindrical axes about its own axis is taken by the cyclic harmonic paths and the rotating sector; other axes stay refused by name.

Follow-up item (id by the lead), title "Pre-stressed modes with a cyclic constraint and per-point materials", deps F-544:

> Every limit not stated here is registered in the ADR before the run. The pre-stressed modes on the transformation refuse a cyclic constraint with an orthotropic or tabulated material by name unless every material with its own axes has cylindrical axes about the cyclic axis and origin; with such axes the index-0 pre-stressed modes of a sector match the full rotor of the same material and pre-stress within registered limits; default paths bit-identical.
