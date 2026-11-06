# ADR 0190: F-256 registration: case-tree wiring of the gradient options

Status: **REGISTERED 2026-11-05**, written before any code of F-256. Not yet accepted by the maintainer.
Parent: [ADR 0016](0016-wave13-numerics.md) D2 (the library `GradientOptions` of F-208, "not claimed through the tree"); roadmap
item F-256 (split from F-246 by the user decision of 2026-10-22). Every key, rule, check and limit below is fixed here, before any
code is written or any test is run. Results go in the addendum at the end.

Read at main 201ea17. Number: 0188 is the highest on main; 0189 is held by F-274 (branch `f274-design-evals`) and 0182 by the M-0
branch (`m0-gate-roadmap`); no branch and no worktree holds a 0190.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md)): defaults stay bit-identical; no solver result or
settings hash of an existing case changes; `crates/rcfd-api/tests/baseline_hashes.rs` and the H1 record R0 are not re-recorded;
no new third-party crate; no commit trailers. Limits are not changed after a run; a miss is reported as measured and its
assertion moves to an ignored `*_known_miss` test with the limit intact.

## What F-256 asks

The roadmap entry: case-tree wiring of the existing `GradientOptions` (face correction, secondary limiter, boundary-gradient
treatment; options added later, for example by F-246, wire under the same checks): invalid values give located errors, a tree
run equals the library run with the same options to 1e-12, and a case without the keys is bit-identical (baseline hashes
unchanged). Out of scope: new gradient schemes, limiter changes. Size S.

## What the code read shows (main 201ea17, nothing run)

1. **The options exist only in `laplacian_with`.** `rcfd_fv::diffusion::GradientOptions` {`face_correction: bool`,
   `secondary_limiter: Option<Limiter>`, `boundary: BoundaryGradient` (`Owner` default, `Corrected`, `Dropped`)} is read only by
   `laplacian_with`, which no solver calls; only `tests/mms_poor_meshes.rs` does. ADR 0016 D2 says the library options sit "in
   `SweepInputs`"; they do not: `SweepInputs` and `SweepExtras` (`rcfd-fv/src/scalar.rs`) have no such field.
2. **The scalar sweep.** `scalar::sweep_ext` assembles `convection + laplacian(geom, gamma, bc, grad)` with
   `grad = scheme.needs_gradient().then(least_squares)`. Its users: `EnergySolver::iterate`/`advance` (energy of the
   incompressible constant-density solvers: steady simple/simplec, coupled, piso/pimple, prescribed flow, Rosseland radiation
   through `en.iterate`), `ScalarTransport::iterate`/`advance` (built by `rcfd-api/src/scalars.rs` `make` for `scalars.<n>`
   and the transported `species.<n>`), and `electric::solve_with_source` (the electric potential, `SweepExtras` with
   `..Default`).
3. **The options can be inert.** `needs_gradient` is false for `upwind`, `central` and `blended`. Then `grad` is `None`, the
   operator has no explicit non-orthogonal (secondary) flux at all, and none of the three options changes anything. Energy and
   scalars take their scheme from `solver.convection` (`solver_conv_scheme`, `sim.rs` lines 1499, 1792, 1931).
4. **Energy and scalar paths that do not go through the sweep.** P1 radiation (`rcfd_fv::radiation::iterate_with_implicit`
   assembles `laplacian` itself on the `EnergySolver`'s data); the enthalpy-method phase change (`PhaseChange::advance`, its own
   assembly); variable-density, compressible and VOF energy (other solvers); turbulence transport (`rcfd-turb` assembles its own
   operators; ADR 0016 D2's "used by ... turbulence transport" is not what the code does); momentum and pressure.
5. **Prescribed flow iterates the energy only.** With `solver.frozen_flow = true` the steady and transient branches of
   `Simulation::step` call only `EnergySolver::iterate`/`advance` on the prescribed flux; scalars are not swept there.
6. **Reports.** The `heat_flow` report of a conjugate case uses `cht::conductive_face_flows(..., non_orthogonal)`, which adds
   the owner-gradient non-orthogonal flux on internal and Dirichlet boundary faces (the default operator's terms).
   `EnergySolver::heat_flow_out` and `scalar_flux` use the boundary coefficients only.
7. **Hash and written text.** A key with `Schema::hash_omit_default` leaves the settings hash of every earlier case unchanged
   (ADR 0156 D6), but `Case::to_toml` and `Case::to_json_string` write every key, defaults included. `case_hashes_w48u.rs` H1
   compares the written text after removing the keys of its `DEFAULT_KEYS` list at their defaults (ADR 0163 S1); each earlier
   key of this kind extended that list (ADR 0181 addendum F2, ADR 0183 P-AB).
8. **Name clash to avoid.** `solver.periodic_gradient` already exists (the momentum least-squares gradient on periodic faces).
9. **`Limiter::VenkatakrishnanRange`** is "a library diagnostic without a case key" (ADR 0115).
10. **Non-orthogonal tree meshes.** Of the generators only `wedge_2d` and `nozzle_2d` (and the fan and pipe meshes) are not
    orthogonal; ADR 0016 measured the options inert on orthogonal meshes (M4 S: ratio 1.0).

## Choices of this registration

**K1. Four keys in `[solver]`, all `.hash_omit_default()`** (Q1 for the names and the place):

| Key | Kind, default | Library value |
|---|---|---|
| `solver.gradient_face_correction` | bool, `false` | `face_correction` |
| `solver.gradient_secondary_limiter` | enum `none` (default), `barth_jespersen`, `venkatakrishnan` | `secondary_limiter`: `None`, `Some(BarthJespersen)`, `Some(Venkatakrishnan(k))` |
| `solver.gradient_limiter_k` | float, `5.0`, at least 0 (finite) | the `k` of `Venkatakrishnan(k)` |
| `solver.gradient_boundary` | enum `owner` (default), `corrected`, `dropped` | `boundary`: `Owner`, `Corrected`, `Dropped` |

The default of `gradient_limiter_k` is the value of ADR 0016's option set C2 (`Venkatakrishnan(5.0)`), the only value the
library checks used (Q2). `VenkatakrishnanRange` gets no key (ADR 0115, item 9). The key documentation says that the options act
only on the explicit non-orthogonal flux of the diffusion term of the energy equation and of the transported scalars and
species, not on momentum, pressure, turbulence or the electric potential, and that `periodic_gradient` is unrelated.

**K2. Mapping.** When all four keys hold their defaults the case maps to no options (`None`), and the solvers call the plain
`laplacian` exactly as today, so a case without the keys and a case with the keys written at their defaults run the same code.
Otherwise the case maps to `Some(GradientOptions)` with the values of the table.

**K3. Library wiring (`rcfd-fv`).** `SweepExtras` gains `gradient: Option<GradientOptions>` (default `None`); `sweep_ext`
assembles `laplacian_with(geom, gamma, bc, grad, cells, bvals, opts)` when it is `Some` and `laplacian` when it is `None`, with
the same `grad` as today (no gradient is computed where none was). `EnergySolver` and `ScalarTransport` gain a public
`gradient: Option<GradientOptions>` field (`None` from `new`) that they pass in their `SweepExtras`.
`radiation::iterate_with_implicit` (P1) uses the same rule with the `EnergySolver`'s field, so every assembly of the energy
equation of an `EnergySolver` honours it. The electric potential keeps `..Default` (`None`). Struct literals that name every
field of `SweepExtras` (energy.rs) gain the field.

**K4. Case wiring (`rcfd-api`).** `Simulation::new` sets the mapped options on the `EnergySolver` it builds; `scalars.rs` `make`
sets them on every `ScalarTransport` it builds (scalars and transported species).

**K5. Located errors** (each a `Diagnostic` with the key's path; the first applicable rule wins):

* **R1 (values).** The schema refuses a value that is not one of the enumerated names, a non-boolean `gradient_face_correction`,
  and a `gradient_limiter_k` that is negative or not finite, each at its key.
* **R2 (no secondary flux).** A non-default `gradient_face_correction`, `gradient_secondary_limiter` or `gradient_boundary` with
  `solver.convection` = `upwind`, `central` or `blended`: the options would act on nothing (item 3). The error is at the first
  non-default key of those three in that order and names the reason. Computing a gradient for these schemes anyway would add a
  correction the operator never had (a new scheme), which F-256 excludes.
* **R3 (no equation takes the options).** A non-default key of those three when the built simulation has neither an energy
  equation solved by an `EnergySolver` with the phase change off, nor at least one scalar or species swept by `ScalarTransport`
  (a non-algebraic entry) on a flow that is not prescribed (item 5). Same path rule as R2. This covers energy off, variable
  density, compressible, VOF without such an equation, and the phase change.
* **R4 (K without its limiter).** `gradient_limiter_k` other than `5.0` while `gradient_secondary_limiter` is not
  `venkatakrishnan`, at `solver.gradient_limiter_k`.

R2 to R4 are made in `Simulation::new` (as the other opt-in solver keys are, e.g. `solver.linear_upwind_limiter`).

**K6. Not wired, stated now.** Momentum, pressure, turbulence transport, the electric potential, the energy of the
variable-density, compressible and VOF solvers, the phase change; reports keep their formulas (Q3); no gradient scheme and no
limiter is changed; `VenkatakrishnanRange` stays library-only.

**K7. Separable commits.** (1) this text; (2) `rcfd-fv` (K3) with its library test; (3) `rcfd-api` keys, wiring and errors
(K1, K2, K4, K5) with the tree tests and the H1 list (H2 below); (4) documents and roadmap; (5) the addendum.

## Open questions

**Q1. Names and place of the keys.** (a) Four flat keys `solver.gradient_*` as K1 (the struct's name and field names, next to the
other opt-in numerics keys of `[solver]`); (b) a table `[solver.gradient_options]` with the struct's field names; (c) per-equation
keys (`energy.gradient_*`, `scalars.<n>.gradient_*`), which F-208 never asked for. Recommendation (a), reversible before any case
uses them. Proceeding on (a).

**Q2. The default of `gradient_limiter_k`.** (a) 5.0, ADR 0016's C2 value; (b) make the key required with `venkatakrishnan`.
Recommendation (a): no other value was ever checked. Proceeding on (a); reversible (one constant).

**Q3. Reports with the options.** With `gradient_boundary = corrected` or `dropped`, or with the face correction or a limiter, the
conjugate `heat_flow` report (item 6) still adds the default operator's non-orthogonal terms, so on skewed cells it is not the flux
the matrix solved. (a) Leave it and say so in the docs; (b) give `conductive_face_flows` the options; (c) refuse the options in a
conjugate case. Recommendation (b) later, as its own check (a balance of the reported flows against the source to the solver
tolerance); proceeding on (a) for F-256, documented, since F-256 asks for the solution fields only.

**Q4. Turbulence transport, momentum, pressure, electric potential.** ADR 0016 D2 defined and checked the options on the scalar
sweep only. (a) Keep them there (K6); (b) extend them to further equations under new checks (a new item). Recommendation (a);
proceeding on (a).

## Acceptance checks and limits (fixed here, before any code)

Common set-up. Mesh W: `wedge_2d`, `nx = 16`, `ny = 16`, `lx = 1`, `ly = 1`, `ramp_start = 0`, `ramp_angle = 30` (degrees),
patches `xmin`, `xmax`, `ymin` (the ramp), `ymax` and the empty pair. Field data: Dirichlet value 1 on `xmin` and 0 on `ymin`
(`FixedValue::scalar`), zero gradient on `xmax` and `ymax`; the discontinuous corner at (0, 0) gives a field with curvature, so
the limiters have cells to act on. `solver.convection = "linear_upwind"`.

* **P (precondition, set-up).** `rcfd_mesh::quality::check_mesh` of W built through the tree gives a maximum non-orthogonality of
  at least 20 degrees. A miss is a set-up defect: the test stops there.
* **Option sets.** O1 `gradient_face_correction = true`; O2 `gradient_boundary = "corrected"`; O3 `gradient_boundary =
  "dropped"`; O4 `gradient_secondary_limiter = "barth_jespersen"`; O5 `gradient_secondary_limiter = "venkatakrishnan"` (K 5 by
  default); O6 the same with `gradient_limiter_k = 0.5`; C2 = O1 + O2 + O5 (ADR 0016's C2). The test lists them in one table, so
  that options added later (F-246) add a row.
* **Cases.**
  * E1 (steady energy, prescribed flow): `simplec`, `solver.frozen_flow = true`, every patch `velocity = "fixed"` (zero),
    `initial.velocity` zero (zero prescribed flux), `energy.enabled`, `energy.relax = 0.9`, `physics` `rho = 1`, `cp = 1`,
    `conductivity = 0.1`, `initial.temperature = 0.5`, `max_iterations = 60`, `tolerance_u = tolerance_p = 1e-14`; 60
    iterations. Sets O1 to O6 and C2.
  * E2 (transient energy, prescribed flow): as E1 with `pimple`, `dt = 0.0625`, `end_time = 1` (16 steps, exact in binary),
    `time_scheme = "euler"`. Set C2.
  * S1 (steady scalar): `simplec` lid cavity on W (`ymax` `velocity = "fixed"`, `velocity_value = [1, 0, 0]`, other patches
    fixed zero), `physics.nu = 0.1`, a scalar `s` with `equation = "poisson"`, `diffusivity = 0.1`, `relax = 0.9`, `initial = 0.5`,
    the field data above; `max_iterations = 60`, `tolerance_u = tolerance_p = 1e-14`; 60 iterations (a poisson scalar is swept on
    a zero flux whatever the flow does). Set C2.
* **G1 (located errors).** Each refused with a diagnostic whose path is the key named: (i) R1:
  `gradient_secondary_limiter = "minmod"`, `gradient_boundary = "wall"`, `gradient_limiter_k = -1.0`,
  `gradient_face_correction = "yes"`, each at its own key (`Case::parse_toml`); (ii) R2: E1 with `convection = "upwind"` and
  O1 at `solver.gradient_face_correction`, with `central` and O3 at `solver.gradient_boundary`, with `blended` and O4 at
  `solver.gradient_secondary_limiter`; (iii) R3: S1 without the scalar and with O1, at `solver.gradient_face_correction`, and a
  phase-change case (the set-up of `phase_change_case.rs`: prescribed zero flux, `piso`, here with `linear_upwind`) with O2 at
  `solver.gradient_boundary`; (iv) R4: E1 with `gradient_limiter_k = 1`
  and no limiter, at `solver.gradient_limiter_k`. Controls (accepted): E1 with O1, S1 with O1, E1 with O6.
* **G2 (tree equals library, the acceptance's 1e-12).** For each case and set: the largest absolute difference over all cells
  between the tree's field (`T` or `s`) after the iterations or steps above and the library run's is at most **1e-12**. The library
  run uses `rcfd_fv` only (no `rcfd-api` helper): the tree's geometry (`Simulation::geom`), a `ScalarField::uniform` with the
  same initial value and boundary conditions, and an `EnergySolver::new(geom, T, k / (rho cp))` with `convection =
  LinearUpwind`, `relax = 0.9` and, for E2, the tree's transient solve controls (`tol_rel = 1e-10`, `tol_abs = 1e-300`,
  `max_iter = 2000`), or a `ScalarTransport::new(geom, s, 0.1)` with `convection = LinearUpwind`, `relax = 0.9`; the options
  are set on its `gradient` field directly; then 60 `iterate` calls on a zero flux, 16 `advance(zero, 0.0625, Euler)` calls, or
  60 `iterate` calls on a zero flux. Control: the tree's default run (no keys) equals the library run with `gradient = None` to
  1e-12.
* **G3 (the check has teeth).** For every set of G2, the tree's field differs from the tree's default run by more than **1e-9**
  (largest absolute difference; the fields lie in [0, 1]), so G2's 1e-12 cannot be met by an unwired run. And O5's E1 field
  differs from O6's by more than 1e-12 (the K value reaches the limiter).
* **G4 (defaults bit-identical).** (i) E1, E2 and S1 with the four keys written at their defaults have the same settings
  content hash as without them, and the same fields bit for bit (`to_bits`) after the same run; (ii) `baseline_hashes.rs`
  passes unedited; (iii) `case_hashes_w48u.rs` H1 passes with the one registered edit H2; (iv) library: `sweep_ext` with
  `gradient: Some(GradientOptions::default())` equals `gradient: None` bit for bit after 5 sweeps on ADR 0016's skewed mesh K
  at n = 8 (`LinearUpwind` on a zero flux, so the gradient and the secondary flux exist; Dirichlet values on every boundary from
  a smooth function).
* **H2 (the one edit of an existing test).** `case_hashes_w48u.rs`: `DEFAULT_KEYS` gains the four keys under `/solver` with
  their defaults (`false`, `"none"`, `5.0`, `"owner"`), its C4 loop gains one non-default copy per key (`true`,
  `"barth_jespersen"`, `1.0`, `"corrected"`), and its module comment names this ADR. R0 is not re-recorded.
* **Can fail.** One mutation, restored afterwards and the file compared byte for byte with the committed one: the line of K4
  that sets the options on the `EnergySolver` removed; then G2 for E1 must fail. `scripts/limit_audit.py` is run on the new
  tests; `rules_w19c` E2 (it reads every case text of the tests) passes.
* **Gate.** fmt per package, clippy `-D warnings`, every test in release on a `git archive` copy, one process per test, three
  jobs: no failure; a flaky failure is rerun alone and reported with its name.

## Not in F-256

New gradient schemes, limiter changes, pressure-interpolation variants (F-246), the options on momentum, pressure, turbulence or
the electric potential (Q4), reports that follow the options (Q3), a case key for `VenkatakrishnanRange`.

## Where to go next

* [ADR 0016](0016-wave13-numerics.md) D2 and its F-208 results for what the options do on the manufactured problem.
* [Case schema](../case-schema.md) for the keys.

## Addendum 1: what was built, and the results (2026-11-05)

Branch `f256-gradient-wiring`, after this text: `rcfd-fv` (K3) with its library test, then `rcfd-api` (K1, K2, K4, K5) with the
tree tests and H2, then the documents. Not merged.

**Built as registered.** K1 to K7 as written. `rcfd-fv`: `SweepExtras::gradient`, `EnergySolver::gradient` and
`ScalarTransport::gradient` (all `None` by default), and one public helper `diffusion::laplacian_opt` (plain `laplacian` for
`None`, `laplacian_with` otherwise) used by `sweep_ext` and by `radiation::iterate_with_implicit`. `rcfd-api`: the four keys in
`schema.rs` (`hash_omit_default`), `sim::gradient_options` (K2) and `check_gradient_options` (R2 to R4, called in
`Simulation::new` after the energy, scalar and phase-change states are built), and one line each in `sim.rs` (energy) and
`scalars.rs` (`make`) that set the options. `ScalarState::sweeps_any` gives R3 its scalar part. No Studio file changed.
Existing test edited: `case_hashes_w48u.rs` (H2 only). New tests: `crates/rcfd-fv/tests/gradient_sweep_f256.rs` (1) and
`crates/rcfd-api/tests/gradient_options_f256.rs` (5).

**Differences found while building.** (1) G1 has one control more than registered: the phase-change case without the keys is
accepted, so R3's refusal of that case is caused by the key. (2) G4 (iv) also asserts a control: ADR 0016's C2 changes the
library result (1.717e-2), so its bit comparison can fail. (3) E1 and S1 solve the same equation (diffusivity 0.1, the same
field data, relaxation 0.9, 60 sweeps on a zero flux), so their C2 numbers agree. (4) Every tree run equals its library run
bit for bit (difference 0), not only to 1e-12.

**Results.**

| Check | Result |
|---|---|
| P | mesh W: maximum non-orthogonality 29.641 degrees (at least 20) |
| G1 | refused at the registered key, each with its message: R1 `minmod`, `wall`, `-1.0`, `"yes"` (schema); R2 `upwind` + O1, `central` + O3, `blended` + O4; R3 S1 without the scalar + O1, the phase-change case + O2; R4 K = 1 without the limiter. Accepted: E1 + O1, S1 + O1, E1 + O6, the phase-change case without the keys |
| G2 | tree vs library, largest absolute difference (limit 1e-12): E1 control (no keys, library `None`) 0, O1 to O6 and C2 0 each; E2 control 0, C2 0; S1 control 0, C2 0 |
| G3 | change against the default tree run (limit > 1e-9): E1 O1 8.106e-3, O2 7.414e-3, O3 1.770e-2, O4 2.150e-5, O5 1.724e-3, O6 1.207e-3, C2 1.500e-2; E2 C2 1.499e-2; S1 C2 1.500e-2. O5 (K 5) vs O6 (K 0.5) on E1: 7.724e-4 (limit > 1e-12) |
| G4 | (i) the keys written at their defaults: settings hash unchanged (E1 `6a4cf3ebb71aa79b`, E2 `0b5356c774cf0c69`, S1 `2d39a6acfe080ca1`), 0 of 256 cells differ in a bit in each; (ii) `baseline_hashes.rs` unedited: 3 passed, 1 ignored (as before); (iii) H1 with H2: 41 recorded sources, 0 differ; C3 30 of 30 valid sources differ raw in the text columns only; C4 the seven non-default copies differ from R0; (iv) 0 of 512 cells differ (K mesh, n = 8) |
| Can fail | the line of K4 that sets the options on the `EnergySolver` removed: G2 E1 O1 failed (tree vs library 8.106e-3, G3 change 0); `sim.rs` restored and equal byte for byte to the kept copy (`cmp`) |
| Audit | `scripts/limit_audit.py`: no hit in the two new test files; its one hit in `case_hashes_w48u.rs` is the existing `return` of the R0 record mode (moved from line 266 to 272 by H2's lines) |
| Gate 1 | `3c7bf10` (the code and documents before this row, on main 201ea17), `git archive` copy, three jobs (`scripts/gate_test.py`, one process per test): fmt 0; clippy `-D warnings` **failed** on the new test file (`clippy::type_complexity` on the array of case-text closures; fixed by a type alias in `40e8cc6`, test unchanged otherwise, workspace clippy then clean); tests 2853 passed, 1 failed, 800 ignored, 548 binaries, 62 min of tests. The failure is `rcfd-project` `journal_f273::c3s_two_appenders_without_a_shared_edit_lock_number_every_line_once` ("a writer waited longer than the lock's wait": 1 of 600 appends without a seq after 250 ms); run alone on the same binary it failed 1 of 3 times. `rcfd-project` depends only on `rcfd-core` and F-256 changes neither: an intermittent failure of F-273's test, not of this work |
| Gate 2 | `c4b93e3` (`40e8cc6` merged with main b367ae9, F-274), `git archive` copy, three jobs: fmt 0, clippy `-D warnings` 0, tests 2859 passed, 1 failed, 800 ignored, 552 binaries, 81 min of tests. Against the F-274 gate (2854 / 0 / 800, 550 binaries): the 5 tree tests and 1 library test of F-256 in 2 new binaries. The failure is `rcfd-project` `case_path_wfix::r9_case_path_is_recorded_normalised` ("Access is denied" on the atomic replace of a `run.json` after 12 attempts, a Windows file lock); run alone on the same binary it passed 5 of 5 times; F-256 does not touch `rcfd-project`. `journal_f273` C3-S passed in this gate. All F-256 tests, `case_hashes_w48u` H1, `baseline_hashes` and `rules_w19c` E2 passed |

**Questions for the maintainer.** Q1 to Q4 stand as registered, each proceeding on its reversible choice. Two failures outside
F-256 were seen in the gates: the F-273 lock test (intermittent also when run alone) and an `rcfd-project` file-lock failure
(passes alone).

## Decision record 2026-11-06

The maintainer's answers of 2026-11-06 to the open questions of this ADR. A question marked *not asked* was not put to the
maintainer that day; it stays on the reversible choice the work proceeded on, which is not a maintainer decision. This
record changes no limit, check or result above.

* **Q1 (names and place of the keys).** Not asked; proceeding on (a).
* **Q2 (the default of `gradient_limiter_k`).** Not asked; proceeding on (a).
* **Q3 (reports with the options).** Answered: (b), as its own item with a balance check of the reported flows against the
  source to the solver tolerance: roadmap item F-289. Until it lands, (a): the conjugate `heat_flow` report keeps the default
  operator's non-orthogonal terms, as documented.
* **Q4 (turbulence transport, momentum, pressure, electric potential).** Answered: (b), a new item with new checks: roadmap
  item F-291 (the recommendation was (a)). F-256 keeps the options on the scalar sweep, as built.
