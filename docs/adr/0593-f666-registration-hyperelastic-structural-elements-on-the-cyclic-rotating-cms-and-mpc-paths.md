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
