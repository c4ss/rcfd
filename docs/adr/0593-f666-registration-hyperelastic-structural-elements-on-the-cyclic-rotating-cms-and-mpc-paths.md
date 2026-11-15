# ADR 0593: F-666 registration: hyperelastic structural elements on the cyclic, rotating, cms and mpc-transformation paths

Status: **REGISTERED 2026-11-15, written before any code of F-666 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-666 was written and nothing of F-666 was compiled or run: the files
under "What was read" were read and the arithmetic below was done by hand (no script, no probe, no rcfd call). Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-666 (dep F-645), structural solver, advanced tier, the follow-up proposed by
[ADR 0584](0584-f645-registration-hyperelastic-materials-on-beam2-shell4-and-shell3.md) (its remaining questions 1 and 2).
Number 0593 given by the lead. Branch `f666-work` from 21b526d0 (local main, which includes F-645's
`SolidModel::set_hyperelastic_structures`), worktree `w49i-work/f666`, scratch `w49i-work/f666-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests and ignored records not
edited; defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every choice below
that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation), except the default lift, which the brief names as not approved (open question 1).

## What F-666 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. With a neo-Hookean material on Beam2 and Shell4 elements, the cyclic, rotating, cms and mpc-transformation paths give the results of the isotropic twin of the same small-strain moduli within 1e-10; on the maintainer's approval the refusal of ADR 0555 D3 on structural elements is lifted by default with RF1 retired as a record and its copy accepting.

Clauses: **a0** every limit registered here; **a1** cyclic path, Beam2 and Shell4, against the twin within `1e-10`; **a2** rotating
path, Beam2 and Shell4; **a3** cms path, Beam2 and Shell4; **a4** mpc-transformation path, Beam2 and Shell4; **a5** on the
maintainer's approval, the default lift with `sa_hyper_paths.rs::rf1_to_rf4_refusals` retired as an ignored record and a copy
whose RF1 row accepts.

**Scope of this work (the brief).** a0 to a4 only, with `set_hyperelastic_structures(true)`. a5 is not approved by the
maintainer: the default is not lifted, RF1 and every existing test are not touched. The item can therefore be at most
**partial** until the maintainer approves a5 (decision rule).

## What was read

`AGENT_RULES.md`; the brief; ADR 0584 (D1 to D5, arithmetic 2 and 3: the modal floor of the default relative residual on thin
rubber structures and the backward residual used instead, the addendum: every row exactly 0); ADR 0555 (D3, RF1); ADR 0581 (the
residual floor diagnosis); ADR 0572, 0573 (the form of "For the lead"). Sources at 21b526d0: `crates/rcfd-fem/src/model.rs`
(`check_elements` 1235-1380, the ADR 0584 condition 1341; the `check_supported_*` family 1181-1415), `cyclic.rs` (module doc 1-30,
`shells_opted_in` 174, `check_cyclic_model` 787-804, `refuse_cyclic_beams` 807-829, `modal_analysis_harmonic` 847,
`solve_static_harmonic` 1395, `HarmonicModes`, `HarmonicStatic`), `mpc.rs` (`uses_transformation` 62, `check_transformation_model`
1355-1393, `solve_static_mpc` 1421, `modal_analysis_mpc` 1747, `MpcModes`), `rotating.rs` (module doc, `RotatingOptions` 160-252,
`check_model` 578-603, `RotatingPart::new`, `modes` 447, `RotatingSector::new` 1083, `modes` 1140), `cms.rs` (module doc, `CmsOptions`,
`check_model` 351-366, `reduce` 485, `DENSE_LIMIT = 600`, `ReducedPart` accessors 784-985), `mat_ortho.rs` (`is_point_material`,
`uses_point_materials`, `check_cyclic_axes` 514: a no-op without a per-point material), `beam.rs` 239-243, `shell.rs` 297-302,
`material.rs` 505-515. A grep of `youngs_modulus()`, `poisson_ratio()` and `Material::` over `geometric.rs`, `geometric_struct.rs`,
`rotating.rs`, `rotating/`, `cms.rs`, `cms/`, `mpc.rs`, `cyclic.rs`, `mass.rs`, `loads.rs`, `assemble.rs`, `element.rs`: no read of
the material outside `beam::moduli`, `shell::moduli`, `beam::axial_force` (ADR 0584 D2), the density and one `Tabulated` test
(cyclic.rs 1426). Tests: `sa_hyper_structures.rs` (the set-ups and measures of ADR 0584 D3), `sa_cyclic_shells.rs` (the frustum
sector of ADR 0556 D3, its loads and the harmonic calls), `sa_geometric_struct.rs` (the rotating beam and plate), `sa_cms.rs`,
`si_modal_mpc.rs` (the cms and rigid-set calls), `sa_rotating_cyclic_shells.rs` (`RotatingSector` on the shell sector).

## Audit of the four paths (before any code)

| Path | Entry check | Beam2 | Shell4 (Shell3) | Hyperelastic with the setting |
|---|---|---|---|---|
| cyclic (`solve_static_mpc` with a cyclic constraint, `solve_static_harmonic`, `modal_analysis_harmonic`) | `check_cyclic_model`: with `CyclicOptions::shells` `check_supported_with_structures` and `refuse_cyclic_beams`; `check_transformation_model` | **refused** (ADR 0556 D1: a beam's orientation vector is a global direction and is not rotated sector to sector; `beam::FOLLOW_UP`), for every material | taken with `CyclicOptions::shells(true)` (ADR 0556), refused without (unchanged) | passes `check_elements` (not a per-point material, so the shells branch is taken) |
| rotating (`RotatingPart::new` / `modes`, `RotatingSector::new` / `modes`) | `rotating::check_model`: `check_supported_with_materials` | taken (ADR 0473 D6) | taken (ADR 0473 D6); a sector takes them through its cyclic constraint's shells option | passes |
| cms (`cms::reduce`, `ReducedPart`) | `cms::check_model`: `check_supported_with_materials`, no cyclic constraint | taken | taken | passes |
| mpc transformation (`solve_static_mpc`, `modal_analysis_mpc` with rigid sets and linear constraints) | `check_transformation_model`: `check_supported_with_materials`; Beam2 or shells with a cyclic constraint as in the cyclic row | taken (without a cyclic constraint) | taken | passes |

So with the setting on, every path takes the hyperelastic material wherever it takes the element, and the element matrices are
the isotropic ones of `mat_hyper::small_strain_young_poisson` (ADR 0584 D2), so no library change is needed for a1 to a4 on the
combinations the paths take. **The one combination refused for its own reason is Beam2 on the cyclic path** (ADR 0556 D1, for
every material). Lifting it is not in the scope of F-666: it is a kinematic statement about beams in a cyclic sector (the
orientation vector rotated per sector, the beam's rotation unknowns rotated as the shells' are), independent of the material,
and its check would be a beam ring against its sector, not a twin. It is registered here as a refusal row (A-B) and proposed as a
follow-up. Clause a1 is therefore met at most for Shell4; a1's Beam2 half is stated as refused, and the item is partial on it in
any case (decision rule).

F-666 is **test-only**: no library line is changed (D1). Shell3 is not in the acceptance; it takes the same route (ADR 0584 D2)
and is run as a registered extra on the cyclic and mpc paths.

## Design

### D1. No library change

The audit shows no read of the material on the four paths beyond the three functions of ADR 0584 D2 and the density. Nothing in
the library is changed; the defaults are trivially bit-identical. Should a row miss with an error or a NaN, that is a diagnosed
miss (`*_known_miss`) and a library fix is a follow-up, not part of this run.

### D2. Materials and the twin

As ADR 0584 D3: **NHc** `neo_hookean(1e6, 1e7)`, density 1000; its twin `Material::isotropic_elastic(E, nu, 1000, 0)` with `(G, K)
= small_strain_moduli`, `s = 3K + G`, `E = 9 K G / s`, `nu = (3K - 2G) / (2 s)` written out in the test. The hyperelastic model sets
`set_hyperelastic_structures(true)`, the twin model does not. Only the neo-Hookean energy (the acceptance names it); the
Mooney-Rivlin energy was run on every row of ADR 0584 through the same three functions. The control material: the twin of
`neo_hookean(1e6 (1 + 1e-6), 1e7)`.

### D3. Set-ups (registered)

- **Beam B**: as ADR 0584 D3 (Beam2 along `x`, `L = 1`, 10 elements, `rectangle_section(0.05, 0.03, 2.6e-7, 5/6)`, orientation
  `[0, 1, 0]`, node sets `x0` (node 0), `xl` (node 10), `tip` (nodes 8, 9, 10), `mid` (node 5); all six components of `x0` held).
  Loads (when loaded): `nodal_force("xl", [0.1, 0.2, 0.3])`, `nodal_moment("xl", [0.01, 0.02, 0.03])`, `gravity([0, 0, -9.81])`.
- **Plate P4 / P3**: as ADR 0584 D3 (`[0, 1] x [0, 0.6]`, `10 x 6` cells, `shell(0.01)`, column `x = 0` held in all six
  components, corner `c = (1, 0.6)`), with the node sets `xl` (the seven nodes of `x = 1`), `p1` (node `(5, 6)`), `p0` (node
  `(5, 0)`). Same loads as ADR 0584.
- **Sector Q4 / Q3** (cyclic): the frustum sector of `sa_cyclic_shells.rs` (ADR 0556 D3): axis `(1, 2, 2)/3`, origin `(0.3,
  -0.2, 0.5)`, `N = 6` sectors, `NC = 2` element columns, `NA = 3` rows, radius `1 - 0.4 s`, height `0.8 s`, `shell(0.05)`, Shell4
  (Q4) or each quad cut along its first diagonal (Q3); node sets `low`, `high` (the cut columns), `clamp` (row 0, all six
  components held), `p<node>` on the loaded nodes (rows 1..3, columns 0..1); constraint `Constraint::cyclic_with("low", "high",
  axis, origin, 6, CyclicOptions::new().shells(true))`. Static loads (on `solve_static_mpc` only): `centrifugal(axis, origin,
  10)`, `shell_pressure("all", 2e2)`, and at each loaded node `q` the force `10 (sin(1.3q + 0.2), cos(0.7q + 0.5), sin(2.1q +
  1.1))` and the moment `(cos(1.9q + 0.3), sin(0.4q + 0.9), cos(1.1q + 1.7))` (the D4 loads of `sa_cyclic_shells.rs` scaled by
  `1e-2` for the rubber).
- **Harmonic load** (on `solve_static_harmonic`, the sector without its own loads): harmonic `k = 1`, cosine part `f_a[e] =
  sin(1.3 e + 0.2)`, sine part `f_b[e] = cos(0.7 e + 0.5)` over the mesh equations `e` (the length of `nodal_load_vector`).
- **Modal options** everywhere: `ModalOptions::new(n).residual_test(ResidualTest::Backward)` (default tolerance `1e-10`, ADR 0584
  arithmetic 3); `n = 6` on the part paths, `n = 3` per harmonic on the sector.
- **Rotating**: `RotatingOptions::new(6).modal(<modal options>)` (stiffening and softening on, no Coriolis). Beam B and P4 about
  the axis `z` through the origin (in the plane of both, a blade). Speed `Omega = 0.2 sqrt(lambda_1)` with `lambda_1` the lowest
  eigenvalue at rest (`modes(0.0)`) of the reference model of the row (the twin, or the perturbed twin for a control), the
  same speed given to both models of the row. Sector: `RotatingSector::new` on Q4, harmonics `k = 0..3`, speed `0.2 sqrt` of the
  smallest of the four lowest rest eigenvalues of the reference.
- **cms**: `cms::reduce(model, "xl", &CmsOptions::new(4))` on loaded Beam B (interface node 10) and loaded P4 (interface `x = 1`).
- **mpc**: Beam B (loaded) with `Constraint::rigid_set("tip", [1.05, 0, 0])` (constraint 6) and
  `Constraint::linear([LinearTerm::new(5, 1, 1), LinearTerm::new(5, 2, -1)], 0)` (`u_y = u_z` at node 5); P4 (loaded) with
  `rigid_set("xl", [1.05, 0.3, 0])` and `linear([(p1, 2, 1), (p0, 2, -1)], 0)`. `solve_static_mpc(model, &[], &[], &[ReferenceLoad::new(6,
  [0.1, -0.2, 0.05], [0.01, 0, 0.02])], &StaticOptions::default())` and `modal_analysis_mpc(model, &[], <modal options>)`.

### D4. Measures

As ADR 0584: `e_u = max_i |a_i - b_i| / max_i |b_i|` over a result vector (static displacements, harmonic `(cos, sin)`
concatenated, the reduced dense matrices and load of cms, the recovered static field of cms, the expanded mpc reference motions
included in `constraint_forces`-free form: the mesh displacements and the reference motions concatenated); `e_lambda = max_k
|lambda_h,k / lambda_t,k - 1|`; `e_phi` the sign-aligned shape difference. Mode shapes are compared only where each eigenvalue is
single: the part paths and the harmonics `k = 0` and `k = N/2 = 3` of the sector (a harmonic `0 < 2k < N` has pairs whose basis is
not unique; their eigenvalues are compared). A non-finite value, an error or a loss of stiffness on a row is a miss.

## Arithmetic (disclosed, by hand)

1. **Expected difference.** By D1 and ADR 0584 D2 the hyperelastic and the twin models build every element matrix, load vector
   and constraint row from the same bits (`(E, nu)` the same expression on the same `(G, K)`), and every path then runs the same
   operations in the same order: expected `0` on every row, as measured on every row of ADR 0584. Should an ulp differ, the
   estimate of ADR 0584 arithmetic 2 (`<= 1e-13` on these small models) holds for the static and the dense rows. The rotating rows
   add a second solve (the pre-stress at 1 rad/s) linear in the same matrices: same estimate. Limits `1e-10` (the acceptance).
2. **Modal convergence.** Beam B and P4: ADR 0584 measured 6 to 7 iterations with backward residuals `<= 2.8e-11`. At `Omega = 0.2
   omega_1` the operator `K + Omega^2 (K_sigma - M_perp)` changes the lowest eigenvalues by `~4 %` of `lambda_1` (the ratio
   `lambda_max / lambda_1` of the floor discussion unchanged): the backward test converges as at rest. Sector Q4: steel in
   `sa_cyclic_shells.rs` converges under the backward test at `1e-12`; the rubber changes `E/rho` by a common factor (`2.9e6/1000`
   against `2e11/7850`) and `nu` from 0.3 to 0.45: the conditioning of the pencil is unchanged to within the `nu` effect, so `1e-10`
   is reached. No loss of stiffness: `Omega^2 = 0.04 lambda_1` is below every softened eigenvalue (the softening is at most
   `Omega^2` on an eigenvalue `>= lambda_1`).
3. **Controls.** As ADR 0584 arithmetic 4: `G (1 + 1e-6)` changes `E` by `0.97e-6` and `nu` by `-4.7e-8`; static and modal results
   by `~5e-7 .. 1e-6` (measured `9.5e-7 .. 1.0e-6` there). The rotating eigenvalue `lambda(Omega) = lambda_0 + Omega^2 c` with `c`
   independent of the moduli to first order (the centrifugal stress of a clamped blade is statically determined): relative change
   `1e-6 lambda_0 / lambda(Omega) ~ 0.96e-6`. The cms matrices: the stiffness blocks scale with the moduli (`~1e-6`), the mass blocks
   change through `Psi` and `Phi` (`~1e-6` relative on the coupling blocks); the measure on `K_r`. Limit `> 1e-8` on each control.

## Checks (limits fixed now)

| Check | Clause | Set-up | Measure | Limit | Estimate |
|---|---|---|---|---|---|
| CY-S-Q4 | a1 | Q4 loaded, `solve_static_mpc`, NHc vs twin | `e_u` | `<= 1e-10` | 0 |
| CY-H-Q4 | a1 | Q4 unloaded, `solve_static_harmonic` `k = 1` | `e_u` over `(cos, sin)` | `<= 1e-10` | 0 |
| CY-M-Q4 | a1 | Q4, `modal_analysis_harmonic` `k = 0..3` | `e_lambda` (all `k`), `e_phi` (`k = 0, 3`) | `<= 1e-10` each | 0 |
| A-B | a1 (audit) | Beam B with NHc and the setting, a cyclic constraint (`low` node 0 and `high` node 10 as node sets with the shells option, axis `z`, `N = 6`): `modal_analysis_harmonic` `k = 0` | refused `NotYetSupported`, package `beam::FOLLOW_UP` (ADR 0556 D1, not the hyperelastic refusal) | holds | holds |
| RO-B | a2 | Beam B unloaded, `RotatingPart::modes(Omega)` | `e_lambda`, `e_phi` | `<= 1e-10` each | 0 |
| RO-P4 | a2 | P4 unloaded, `RotatingPart::modes(Omega)` | `e_lambda`, `e_phi` | `<= 1e-10` each | 0 |
| CM-B | a3 | Beam B loaded, `cms::reduce` | `e_u` of `stiffness()`, `mass()`, `load()`, `fixed_interface_eigenvalues()`, `eigen().values`, `recover_static(solve_static())` | `<= 1e-10` each | 0 |
| CM-P4 | a3 | P4 loaded, the same | the same | `<= 1e-10` each | 0 |
| MP-B | a4 | Beam B loaded with the rigid set and the linear constraint, `solve_static_mpc` and `modal_analysis_mpc` | `e_u` (displacements, then the reference motions), `e_lambda`, `e_phi` | `<= 1e-10` each | 0 |
| MP-P4 | a4 | P4, the same | the same | `<= 1e-10` each | 0 |
| X-CY-Q3 | extra | Q3: the rows CY-S, CY-H, CY-M | as there | `<= 1e-10` each | 0 |
| X-RS-Q4 | extra | Q4, `RotatingSector::modes(k, Omega)` `k = 0..3` | `e_lambda` (all), `e_phi` (`k = 0, 3`) | `<= 1e-10` each | 0 |
| X-MP-P3 | extra | P3, the rows MP | as there | `<= 1e-10` each | 0 |
| C-CY, C-RO, C-CM, C-MP | control | NHc against the perturbed twin: CY-S-Q4 `e_u`; RO-B `e_lambda`; CM-B `e_u` of `stiffness()`; MP-P4 static `e_u` | as listed | `> 1e-8` each | `~1e-6` |
| N1 | control | without the setting, NHc: Q4 `modal_analysis_harmonic` `k = 0`; Beam B `RotatingPart::new`; Beam B `cms::reduce`; P4 with the mpc constraints `solve_static_mpc` | refused naming `LINEAR_FOLLOW_UP` and "a structural element" | holds (4 rows) | holds |
| N2 | control | the setting on an isotropic (twin) Q4 `solve_static_mpc` and Beam B `RotatingPart::modes(Omega)` against the same models without it | bits equal | holds | holds |
| B0 | defaults | no library line changed: `git diff --stat 21b526d0 -- crates/*/src` empty; the existing `sa_hyper_structures` and `sa_hyper_paths` (RF1) pass | holds, pass | holds | holds |

Rows run in release (`cargo test -p rcfd-fem --release --test sa_hyper_structures_f666 -- --test-threads=1 --nocapture`); the
models are at most 77 nodes; expected seconds.

## Planned mutations (on a scratch copy of the library, one at a time)

1. `mat_hyper::small_strain_young_poisson`: `9 K G / (3K + G)` -> `9 K G / (3K + 2G)`. Expected caught by every acceptance row.
2. `check_elements`: the setting ignored, never lifted (`!self.hyperelastic_structures &&` -> `true &&`). Caught by every
   acceptance row (refused).
3. The setting ignored, always lifted (`-> false &&`). Caught by N1 (4 rows).
4. `shell::moduli` reads `youngs_modulus`, `poisson_ratio` again. Caught by every Q4, P4 row (NaN).
5. `beam::moduli` reads the two getters again. Caught by every Beam B row (NaN).
6. `beam::axial_force` reads `youngs_modulus` again. Caught by RO-B only (the beam pre-stress of the rotating path; ADR 0584
   caught it only by buckling).

## Decision rule

The default lift (a5) is not approved: the item is **at most partial** until the maintainer approves it, whatever the run gives.
**Partial, path clauses met** if every acceptance row (CY on Q4, RO-B, RO-P4, CM-B, CM-P4, MP-B, MP-P4) meets its limit in the one
registered run, A-B holds (the Beam2 half of a1 refused for its own reason, stated), every control holds, B0 holds, mutations 1 to
6 are each caught, fmt and clippy clean. **Partial, path clauses missed** if an acceptance row misses (recorded `*_known_miss`)
or a control, B0 or a mutation fails. Extras (Q3, the rotating sector, P3) that miss are stated, not decisive. **Todo** if no
acceptance row could be run. **Done** is reachable only after a5 (the maintainer's approval, the retirement of RF1 and a copy
accepting) and a decision on the Beam2 cyclic refusal (open question 2), in a later amendment.

## Limits (not covered)

- a5, the default lift and RF1 (not approved; open question 1).
- Beam2 on the cyclic path, refused for every material (ADR 0556 D1); proposed as a follow-up.
- The whirl paths (Coriolis, `whirl_modes`, `whirl_state`, the split modes), the Campbell tables and the full rotor are not run;
  they read the same matrices plus the Coriolis matrix (density only).
- The cms assembly of several parts and the superelement model are not run (they consume `ReducedPart` only).
- The beam warping path, harmonic and transient through the modes, the pre-stressed paths: not run here.
- The Mooney-Rivlin energy only by ADR 0584 (same functions).

## For the lead: roadmap entries

Written in the addendum after the run.

## Amendment 1 (2026-11-15, written and committed after run 1 and before the corrected run)

**Run 1** (the registered run, made once: `cargo test -p rcfd-fem --release --test sa_hyper_structures_f666 -- --nocapture
--test-threads=1` at 6cb7beca, log `w49i-work/f666-scratch/run1.log`, 0.97 s): 8 of 11 tests passed; the cyclic, cms and mpc
rows, the Q3 and P3 extras, A-B, N1, N2 and the controls C-CY, C-CM, C-MP held (their numbers go in Addendum 1). Three tests
failed, every failed row on the **rotating** path, all with the same error before any number: RO-B, RO-P4 (a2), X-RS-Q4 (extra)
and C-RO (control) were refused by `RotatingPart::new` with `NotYetSupported { variant: "Hyperelastic", package: LINEAR_FOLLOW_UP,
location: "... a structural element (Beam2, Shell4, Shell3)" }`, although the model has the setting on.

**Diagnosis.** `RotatingPart::new` passes its own entry check (`rotating::check_model`, with the setting), then solves the
centrifugal pre-stress at 1 rad/s on a copy built by `rotating::unit_centrifugal_model` (rotating.rs 312-334) from
`SolidModel::new(model.mesh().clone())` with the materials, constraints and supports copied one by one. The copy does not carry
`hyperelastic_structures` (it is the default `false`), so the static solve of the copy refuses the material. The audit missed it:
it searched the paths for reads of the material, not for copies of the model. It is the only `SolidModel::new` in the library
outside `model.rs` (grep). `RotatingSector::new` goes through `RotatingPart::new`, hence X-RS-Q4. A library defect of F-645 (the
setting was not propagated to the one place that rebuilds a model), not a defect of the set-up; the limits are not touched.

**Records.** The three failed tests stay in the file as ignored records with their limits intact: `ro_rotating_part_known_miss`,
`x_rs_q4_rotating_sector_known_miss`, `c_perturbed_twin_is_seen_known_miss` (the last missed only its C-RO row; its other three
rows held). Their bodies are not changed.

**D5 (the fix).** In `unit_centrifugal_model`, after `SolidModel::new`: `copy.set_hyperelastic_structures(model.
hyperelastic_structures());`. For a model without the setting the copy has `false`, as before, so every default run builds the
same copy and gives the same bits (B0: `rcfd-api --test baseline_hashes` and the rotating tests listed in the corrected B0 row).
Shared solver file `rotating.rs`, one line.

**The corrected tests** (copies of the failed ones under new names, the same set-ups, measures and limits as registered, run
once after the fix): `ro_rotating_part_amended` (RO-B, RO-P4), `x_rs_q4_rotating_sector_amended` (X-RS-Q4),
`c_perturbed_twin_is_seen_amended` (C-CY, C-RO, C-CM, C-MP). The other tests of run 1 are not re-run as registered runs (they
run again only as part of the build and B0).

**Mutation 7** (added): the D5 line removed. Expected caught by the amended RO-B, RO-P4, X-RS-Q4, C-RO rows (refused as in run 1).

**B0 (corrected)**: no library line changed except D5 (`git diff --stat 21b526d0 -- crates/*/src`: `rotating.rs` one line);
`rcfd-api --release --test baseline_hashes`; the rotating tests `sa_geometric_struct`, `sa_geometric_struct_bits`,
`sa_ortho_rotating_default_bits`, `sa_rotating_cyclic_shells_default_bits`, `si_rotating`; and `sa_hyper_structures`,
`sa_hyper_paths` (RF1). Pass.

The decision rule is unchanged: at most partial (a5 not approved).

## Addendum 1: results (2026-11-15)

Run 1 (registered, at 6cb7beca, log `w49i-work/f666-scratch/run1.log`) as described in Amendment 1; the corrected run of the
amended tests (once, at b9602743 after D5, `cargo test -p rcfd-fem --release --test sa_hyper_structures_f666 amended --
--nocapture --test-threads=1`, log `run2.log`, 0.10 s). Every difference is exactly 0, as arithmetic 1 expected.

| Check | Limit | Measured | Met |
|---|---|---|---|
| CY-S-Q4 `e_u` (a1, run 1) | `<= 1e-10` | 0 | yes |
| CY-H-Q4 `e_u` (a1, run 1) | `<= 1e-10` | 0 | yes |
| CY-M-Q4 `e_lambda` k = 0..3, `e_phi` k = 0, 3 (a1, run 1) | `<= 1e-10` | 0 each (7 / 16 / 11 / 7 iterations, backward residuals <= 7.6e-11) | yes |
| A-B Beam2 on the cyclic path (a1 audit, run 1) | refused, `beam::FOLLOW_UP` | refused: "constraint 6, a model with Beam2", "a follow-up of ADR 0417" | holds |
| RO-B `e_lambda`, `e_phi` (a2) | `<= 1e-10` | run 1: refused (Amendment 1, record `ro_rotating_part_known_miss`); corrected: 0, 0 (Omega = 0.328 rad/s, 7 / 7 iterations) | yes after D5 |
| RO-P4 `e_lambda`, `e_phi` (a2) | `<= 1e-10` | run 1: refused (same record); corrected: 0, 0 (Omega = 0.117 rad/s, 6 / 6) | yes after D5 |
| CM-B `e_K`, `e_M`, `e_f`, fixed-interface and reduced `e_lambda`, recovered `e_u` (a3, run 1) | `<= 1e-10` | 0 each (6 boundary, 54 interior, 4 modes) | yes |
| CM-P4 the same (a3, run 1) | `<= 1e-10` | 0 each (42 boundary, 378 interior) | yes |
| MP-B static `e_u`, modes `e_lambda`, `e_phi` (a4, run 1) | `<= 1e-10` | 0, 0, 0 (6 / 6 iterations) | yes |
| MP-P4 the same (a4, run 1) | `<= 1e-10` | 0, 0, 0 | yes |
| X-CY-Q3 static, harmonic, modes (extra, run 1) | `<= 1e-10` | 0 each | yes |
| X-RS-Q4 k = 0..3 (extra) | `<= 1e-10` | run 1: refused (record `x_rs_q4_rotating_sector_known_miss`); corrected: 0 each (Omega = 6.18 rad/s) | yes after D5 |
| X-MP-P3 (extra, run 1) | `<= 1e-10` | 0, 0, 0 | yes |
| C-CY, C-CM, C-MP (run 1 and corrected) | `> 1e-8` | 9.43e-7, 9.68e-7, 9.44e-7 | yes |
| C-RO | `> 1e-8` | run 1: refused (record `c_perturbed_twin_is_seen_known_miss`); corrected: 1.00e-6 | yes after D5 |
| N1 (4 rows, run 1) | refused, "a structural element" | refused, every row | holds |
| N2 cyclic static, rotating B (run 1) | bits equal | equal | holds |
| B0 (corrected) | pass | `git diff --stat 21b526d0 -- crates/*/src`: rotating.rs +2 (D5 and its comment); `sa_geometric_struct` 9, `sa_geometric_struct_bits` 1, `sa_ortho_rotating_default_bits` 1, `sa_rotating_cyclic_shells_default_bits` 1, `si_rotating` 9, `sa_hyper_structures` 6, `sa_hyper_paths` 17 (RF1 among them), `rcfd-api --test baseline_hashes` 3: all pass (ignored records not run). The B0 command also ran the whole `sa_hyper_structures_f666` file: 11 pass, 3 ignored | yes |

fmt (`cargo fmt -p rcfd-fem --check`) clean; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean.

Mutations (scratch copy `w49i-work/f666-scratch/mut` of b9602743, script `mutate.py`, log `mutations.log`), each caught:

| Mutation | Caught by |
|---|---|
| 1 `E = 9KG/(3K + 2G)` | every acceptance and extra row (`3.0e-2 .. 3.2e-2` on eigenvalues and displacements) |
| 2 setting never lifted | every acceptance, extra and control row (refused), and A-B (refused by the material before the Beam2 cyclic check) |
| 3 setting always lifted | N1, all 4 rows |
| 4 `shell::moduli` back to the getters | every Q4, Q3, P4, P3 row, C-CY, C-MP (NaN) |
| 5 `beam::moduli` back to the getters | RO-B, CM-B, MP-B, C-RO, C-CM (NaN) |
| 6 `beam::axial_force` back to `youngs_modulus` | RO-B and C-RO only (the Beam2 pre-stress not finite), as registered |
| 7 D5 removed | RO-B, RO-P4, X-RS-Q4, C-RO (refused as in run 1), as registered |

Deviations: the rotating rows missed in run 1 (Amendment 1: a defect of F-645, the pre-stress copy dropped the setting); fixed
by D5 (one line plus a comment in the shared solver file `crates/rcfd-fem/src/rotating.rs`, lines 314-315) and met in the
corrected run. Not run: a5; Beam2 on the cyclic path (refused, A-B); the whirl, Campbell, full-rotor, cms assembly and
superelement paths; the whole workspace suite (the lead's gate).

**Decision by the registered rule: partial, path clauses met.** Every acceptance row of a1 (Shell4) to a4 met (the rotating rows
after Amendment 1), A-B holds, every control holds, B0 passes, seven of seven mutations caught, fmt and clippy clean. Partial
because a5 (the default lift and RF1) is not approved, and because the Beam2 half of a1 is refused by the cyclic path for every
material (ADR 0556 D1).

### Remaining questions

1. **The default lift (a5).** Options: (a) keep the opt-in; (b) lift by default: drop the structural branch of the ADR 0555 D3
   refusal (keep the temperature refusal), retire `sa_hyper_paths.rs::rf1_to_rf4_refusals` as an ignored record with a copy
   whose RF1 row accepts (the ADR 0291 pattern), and keep `set_hyperelastic_structures` as a documented no-op. Recommendation:
   (b). Every linear path that takes structures now has a twin check at exact equality, and the one defect the checks found
   (the rotating copy) is fixed, so the refusal protects nothing. It needs the maintainer's approval (it changes a default
   result: a refused model becomes accepted). Not taken.
2. **Beam2 on the cyclic path.** Refused for every material (ADR 0556 D1). Options: (a) a follow-up that rotates the beam's
   orientation vector and rotation unknowns per sector, checked on a beam ring against its sector; (b) leave it.
   Recommendation: (a), independent of the material, proposed below. Until then a1 can only be met for shells; the lead may
   prefer to restate a1 as "Shell4 on the cyclic path" and leave the beam half to the follow-up.
3. **Other model copies.** D5 was found because a path rebuilds a model field by field. Recommendation: a `SolidModel` method
   that copies the mesh with every setting, for future copies; not done here (no other copy exists today).

## For the lead: roadmap entries

> Proposed status: partial.

> Structural solver, advanced tier. Follow-up of F-645 (ADR 0584). PARTIAL. ADR 0593, 2026-11-15: with SolidModel::set_hyperelastic_structures(true) a neo-Hookean material on Shell4 (and Shell3) elements on the cyclic path (solve_static_mpc on a frustum sector with CyclicOptions::shells, solve_static_harmonic k = 1, modal_analysis_harmonic k = 0..3), and on Beam2 and Shell4 elements on the rotating path (RotatingPart::modes at 0.2 omega_1 with stress stiffening and spin softening; RotatingSector::modes on the sector), the cms path (cms::reduce: reduced stiffness, mass, load, fixed-interface and reduced eigenvalues, recovered static field) and the mpc-transformation path (solve_static_mpc and modal_analysis_mpc with a rigid set and a linear constraint), gives the results of the isotropic twin of the same small-strain moduli with difference exactly 0 (limit 1e-10); the twin with G (1 + 1e-6) is seen at 9.4e-7 .. 1.0e-6. The registered run found the rotating path refusing the material: its unit pre-stress copy did not carry the setting (fixed by one line in rotating.rs, defaults bit-identical: baseline hashes and the rotating default-bits records pass; the run-1 tests kept as ignored known-miss records). Seven of seven mutations caught. Not done: Beam2 on the cyclic path stays refused for every material (ADR 0556 D1); the default lift with RF1 retired awaits the maintainer's approval.

> Follow-up proposed: Beam2 elements in the cyclic constraint. Deps F-628. Acceptance: Every limit not stated here is registered in the ADR before the run. A cyclic sector holding Beam2 elements (the orientation vector and the rotation unknowns rotated per sector) gives the static displacements and the modes of each harmonic index of the full ring of beams within 1e-10, the defaults bit-identical.
