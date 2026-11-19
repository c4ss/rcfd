# ADR 0746: F-666 registration: hyperelastic materials on structural elements by default

Status: **REGISTERED 2026-11-19, written before any code of this registration and before any run of its checks.** Before
this text no library or test code of this registration was written and none of its checks was run. One thing was done
before it: a release build of the unchanged library (`cargo build --release -p rcfd-fem`), which runs no test. Every
number below comes from reading the code, the tests and the ADRs. Results go in the addenda at the end; nothing above them
is changed after a run.

This is a further registration of roadmap item F-666. It answers the brief
[docs/adr/briefs/f666-hyper-structures-default.md](briefs/f666-hyper-structures-default.md) and follows
`docs/process/agent-rules.md`, version 3. Parent record:
[ADR 0593](0593-f666-registration-hyperelastic-structural-elements-on-the-cyclic-rotating-cms-and-mpc-paths.md) (the first
registration of F-666, its remaining question 1 and the maintainer's answer of 2026-11-18 at its end). Read at main
4f2722da.

## What is asked

The maintainer approved on 2026-11-18 (ADR 0593, "Answers of the maintainer"): "The default change is approved. Its
registration lists the changed results case by case and the records it re-records, before the code." The change is clause
a5 of the acceptance of F-666: **the refusal of ADR 0555 D3 of a hyperelastic material on a structural element (Beam2,
Shell4, Shell3) on the linear paths is lifted by default**, with RF1 retired as a record and a copy that accepts. This is
an approved change of a default, so rule 3's "defaults stay bit-identical" is lifted for exactly the calls listed under
"What changes, case by case" and for nothing else. No result of a model that was accepted before changes: the change turns
refusals into results.

The acceptance of F-666, unchanged:

> Every limit not stated here is registered in the ADR before the run. With a neo-Hookean material on Beam2 and Shell4 elements, the cyclic, rotating, cms and mpc-transformation paths give the results of the isotropic twin of the same small-strain moduli within 1e-10; on the maintainer's approval the refusal of ADR 0555 D3 on structural elements is lifted by default with RF1 retired as a record and its copy accepting.

Clauses (ADR 0593): a0 every limit registered; a1 to a4 the cyclic, rotating, cms and mpc-transformation paths against the
twin within `1e-10`; a5 the default lift. State before this registration: a2, a3, a4 and the Shell4 half of a1 met with the
setting (ADR 0593); the Beam2 half of a1 met with the setting by H-T of ADR 0744; a5 not made.

## What was read

- `docs/process/agent-rules.md` (version 3), the brief, ADR 0593 whole, ADR 0744 whole (the form of a registration of an
  approved default change; "What F-666 will find changed"), ADR 0555 whole (D1 to D3, RF1 to RF4, D5), ADR 0584 (the audit
  of the tests, D1, D2, D4, the limits, N1 to N4).
- `crates/rcfd-fem/src/mat_hyper.rs` whole; `model.rs` lines 990 to 1025 (the field), 1126 to 1135 (the getter and the
  setter), 1346 to 1580 (the `check_elements_supported*` and `check_supported*` family and `check_elements`);
  `material.rs` 468 to 516 (`linear_*`, `structural_young_poisson`); `beam.rs` 244 to 247 and `shell.rs` 301 to 304
  (`moduli`); `rotating.rs` 340 to 350 (`unit_centrifugal_model`, the copy of the setting, ADR 0593 D5); `rotor.rs` 447
  to 452, and by search the refusals of a non-isotropic material in `shell_corot.rs`, `laminate.rs`, `fracture.rs`.
- Tests: `sa_hyper_structures_f666.rs` whole, `sa_hyper_structures.rs` whole, `sa_hyper_paths.rs` (head, the helpers,
  `rf1_to_rf4_refusals`), `sa_cyclic_beams_default_adr0744.rs` (head, the builders of W,
  `ab_a_h_d_h_t_hyperelastic_beams_on_the_cyclic_path`) and its committed mutation patch, `sa_laminate.rs` and
  `sa_scaffold.rs` (the rows that name the material).
- Searches of the workspace: `hyperelastic_structures`, `LINEAR_FOLLOW_UP`, "a structural element", "follow-up of ADR
  0555", and every Rust file that names the hyperelastic material together with Beam2, Shell4, Shell3 or a shell or beam
  property. The material is named in no crate but `rcfd-fem`: no case key, schema, golden or hash table reaches it.

### Audit: every test that asserts the refusal of a hyperelastic material on a structural element

| # | Test | What it asserts | After this registration |
|---|---|---|---|
| T1 | `sa_hyper_paths.rs::rf1_to_rf4_refusals`, row RF1 (ADR 0555) | `solve_static` of a 2 x 2 Shell4 plate with `neo_hookean(1e6, 1e7)`, default model, refused `NotYetSupported { variant: "Hyperelastic", package: LINEAR_FOLLOW_UP }`, location "a structural element" | **Changed.** Retired as an ignored record; restated in the new file with RF1 as acceptance, RF2, RF3 and RF4 as they are |
| T2 | `sa_hyper_structures.rs::n1_to_n4_settings_and_refusals`, N1, six rows (ADR 0584) | `solve_static` and `modal_analysis` of Beam B, P4 and P3 with NHc, without the setting, refused the same | **Changed.** Retired as an ignored record; restated with N1 as acceptance, N2, N3 and N4 as they are |
| T3 | `sa_hyper_structures_f666.rs::n1_refused_without_the_setting`, N1, four rows (ADR 0593) | without the setting: Q4 `modal_analysis_harmonic` `k = 0`, Beam B `RotatingPart::new`, Beam B `cms::reduce`, P4 with the mpc constraints `solve_static_mpc`, refused the same | **Changed.** Retired as an ignored record; restated with the four rows as acceptance |
| T4 | `sa_cyclic_beams_default_adr0744.rs::ab_a_h_d_h_t_hyperelastic_beams_on_the_cyclic_path`, row H-D (ADR 0744) | W with the hyperelastic material, without the setting: `modal_analysis_harmonic` `k = 0` refused the same | **Changed.** Retired as an ignored record; restated with H-D as acceptance, AB-a and H-T as they are |
| T5 | `sa_hyper_structures_f666.rs::a_b_beam2_cyclic_refused_for_its_own_reason` (ADR 0593 A-B) | the Beam2 cyclic refusal | already an ignored record of ADR 0744; not touched (the brief) |
| T6 | `sa_hyper_paths.rs` RF2, `sa_hyper_structures.rs` N2 | a temperature load with the material refused naming `LINEAR_FOLLOW_UP`, location "a temperature load" | unchanged, still refused (D1); copied into the new file with their tests |
| T7 | `sa_hyper_paths.rs` RF3, RF4; `sa_laminate.rs` RF2 (a hyperelastic ply); `sa_scaffold.rs` (the element-constant matrix) | other refusals of the material | unchanged, not reached by D1 |

Two names of the brief differ from what was found, both stated here and not a change of scope. The brief's "T2 of ADR
0593" is no row of ADR 0593: T2 is the audit number that ADR 0744 gave to A-B (T5 above, which the brief says to leave);
the test of ADR 0593 that asserts the refusal without the setting is its N1 (T3). The brief places H-D in
`sa_cyclic_shells_refusals_adr0744.rs`; it is in `sa_cyclic_beams_default_adr0744.rs` (T4), as ADR 0744's addendum says.
T2 (N1 of ADR 0584) is not named by the brief and was found by reading: it asserts the same refusal, so it cannot stay.

## What changes, case by case

A model with a `Material::Hyperelastic` on an element set that holds a Beam2, Shell4 or Shell3 element, on a path that
calls `SolidModel::check_elements` with `history = false` (every linear path: the static solve, the modes, the harmonic,
transient, buckling and pre-stress paths, the damped modes, cms, the mpc transformation, the cyclic and the rotating
paths, beam warping).

| Model | Before | After |
|---|---|---|
| without the setting, on a path that takes the element kind | refused `NotYetSupported { variant: "Hyperelastic", package: LINEAR_FOLLOW_UP }`, location "material i, element set 's': a structural element (Beam2, Shell4, Shell3)" | **taken**, with the code and the bits of the same model with `set_hyperelastic_structures(true)` (ADR 0584 D2: the isotropic element of the small-strain `E`, `nu`). **Changed.** |
| with `set_hyperelastic_structures(true)` | taken (ADR 0584, ADR 0593, H-T of ADR 0744) | the same bits |
| without the setting, with a `Load::Temperature` | refused as the first row (the structural test came first) | still refused, naming the same package, location "... a temperature load". **Changed (the error only).** |
| with the setting, with a `Load::Temperature` | refused, location "a temperature load" | the same |
| without the setting, and something a later test of the path refuses (a later material of the list that the path does not take, `beam::check_beam_model`, `shell::check_shell_model`, `mat_ortho::check_model`, the supports, a path's own refusal) | refused as the first row | refused by that later test, as the same model with the setting was. **Changed (the error only).** |
| on a path that does not take the element kind (a Beam2 or shell block refused by the block loop of `check_elements`) | refused for the element kind, before the material is looked at | the same |
| on a path with `history = true` (the nonlinear driver and its callers) | the block of ADR 0555 D3 not reached | the same |

Not changed: every model without a hyperelastic material on a structural element (the block of D1 is entered only for a
`Material::Hyperelastic`, and for one on solid elements only the removed test, which was false, is gone); every case file.

What still refuses a hyperelastic material afterwards: a temperature load on a linear path (ADR 0555 D3; T6) and a
temperature field (the finite-expansion checks, key `expansion`; RF4); `Material::elasticity_matrix` (ADR 0541 D2; RF3);
a hyperelastic ply of a layup (`laminate`; `sa_laminate.rs` RF2); the corotated shells (`shell_corot`: "the isotropic
elastic material only"); the rotor shaft (`rotor.rs`, key `material`); the fracture functions; the energy-momentum step
with `AlgorithmicStress::Auto` (ADR 0544). None of these is touched.

## Design

**D1 (the lift).** In `check_elements` (`model.rs`) the block of ADR 0555 D3 keeps its temperature-load refusal, with the
same item, variant, location text and package, and loses its structural-element refusal and the read of
`self.hyperelastic_structures`. Nothing else of `check_elements` changes. The element matrices are not touched: a
structural element reads `Material::structural_young_poisson`, which for the hyperelastic material is
`mat_hyper::small_strain_young_poisson` (ADR 0584 D2, derived there from first principles: `E = 9 K G / (3K + G)`, `nu =
(3K - 2G) / (2 (3K + G))` of the tangent at rest of ADR 0555 D1). So a model without the setting runs, after the lift, the
code the same model ran with the setting before it.

**D2 (the setting afterwards: a documented no-op).** `SolidModel::set_hyperelastic_structures` and
`hyperelastic_structures` stay, with the field: the setter stores the value, the getter returns it (default `false`), and
no path reads it to decide anything. Their doc comments say so and name this ADR. Why not removed: the existing tests and
records call the setter and are not edited (rule 2), and the brief expects a no-op. Why the value is still stored and not
a constant `true`: N4 of ADR 0584 (`!hyperelastic_structures()` on a new model) stays true in its record and its copy, and
`SolidModel`'s `PartialEq` and `Debug` keep their shape. `rotating::unit_centrifugal_model` still copies the setting (ADR
0593 D5); the line is left as it is, now without effect (`rotating.rs` is not touched). Removing the setter is an API
change for a later decision; it is not proposed.

**D3 (no result of an accepted model changes).** `check_elements` only returns `Ok` or an error. For a model it accepted
before, D1 removes a test that was false, so the same code runs afterwards. Shown by the existing tests unedited (B1), the
existing bit records (B2), `baseline_hashes` (B3) and bit-equality of the lifted default with the setting (the E rows).

**D4 (the retired tests and the new file).** T1, T2, T3 and T4 each get one line `#[ignore = "..."]` naming ADR 0746;
their bodies are not touched. One new test file, `crates/rcfd-fem/tests/sa_hyper_structures_default_adr0746.rs` (header:
ADR 0746, F-666), holds the four restated tests and the checks below. Its builders are copies: Beam B, P4, P3 of
`sa_hyper_structures.rs` (ADR 0584 D3); Beam B and the plates with their extra node sets, the sector Q4 / Q3, the mpc
constraints, the measures and the path rows of `sa_hyper_structures_f666.rs` (ADR 0593 D3, D4); the cantilever, Cube F
and the Method options of `sa_hyper_paths.rs` (for RF2 and RF4); the wheel sector W of
`sa_cyclic_beams_default_adr0744.rs`. "By default" below means: `set_hyperelastic_structures` is never called on the
model. NHc is `neo_hookean(1e6, 1e7)`, density 1000; its twin and its perturbed twin (`G (1 + 1e-6)`) as ADR 0593 D2.

Set-ups fixed now, where they are not a copy:

- **RF1 restated.** (a) The plate of RF1 exactly as T1 builds it (no support, no load): `solve_static` returns something
  other than the hyperelastic refusal (the outcome is printed and not asserted further: the plate is unsupported, so a
  refusal for a singular stiffness is expected). (b) The same plate with the nodes of the column `x = 0` (nodes 0, 3, 6)
  held in their six components and, at the corner node 8, the force `[0.1, 0.2, 0.3]` and the moment `[0.01, 0.02,
  0.005]`, with `gravity([0, 0, -9.81])` (the loads of P4): `solve_static` by default against the twin, `e_u`.
- **N1 of ADR 0584 restated.** Beam B, P4, P3 by default with NHc against the twin: `solve_static` on the loaded model
  (`e_u`) and `modal_analysis` on the unloaded one with the file's modal options (`e_lambda`, `e_phi`), the set-ups and
  measures of ADR 0584 D3.
- **N1 of ADR 0593 restated.** (i) The four calls of T3 exactly as written there return `Ok`. (ii) Every path row of ADR
  0593 run by default: CY-S-Q4, CY-H-Q4, CY-M-Q4, RO-B, RO-P4, CM-B, CM-P4, MP-B, MP-P4 and the extras X-CY-Q3,
  X-RS-Q4, X-MP-P3, with the set-ups, speeds, options and measures of ADR 0593 D3 and D4, the hyperelastic model built
  without the setting.
- **H-D restated.** W with the hyperelastic material by default against W with the twin, both `shells(true)`: the
  eigenvalues of `k = 0` and `k = 1` (4 each), `max |lambda_h / lambda_t - 1|` (the measure and options of H-T).
- **Temperature loads by default.** Beam B and P4 by default with NHc and `Load::temperature(303 at every node, 293)`:
  `solve_static` refused naming `LINEAR_FOLLOW_UP`, location "a temperature load".
- **The setting is a no-op.** The hyperelastic model by default against the same model with
  `set_hyperelastic_structures(true)`, and against the same with `true` then `false`: the bits of `solve_static` (Beam
  B, P4, P3 displacements), `modal_analysis` (Beam B, P4 eigenvalues), CY-S-Q4 displacements, RO-B eigenvalues at the
  row's speed, CM-B `stiffness()`, `mass()` and `load()`, MP-P4 static displacements, and the eigenvalues of W at `k =
  0`.

## Checks

Limits `1e-10` are the acceptance of F-666; exact limits are the tolerance table's row "Bit-identity of default output";
the control limit `1e-8` is that of ADR 0593 (its controls C). By D1 the model by default runs the code the model ran with
the setting, and ADR 0584, ADR 0593 and ADR 0744 measured exactly 0 on every such row; the estimate column keeps the bound
of ADR 0593 arithmetic 1 (`<= 1e-13` should an ulp differ). Where the estimate equals an exact limit the margin is
written 1.

| # | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| A-RF1 | a5, T1 restated: (a) the plate of RF1 as built is not refused for the material; (b) supported and loaded, by default against the twin: `e_u` | (a) holds; (b) `<= 1e-10` | acceptance of F-666 | holds; `<= 1e-13` | 1000 | code verification |
| A-S | a5, T2 restated, static: Beam B, P4, P3 by default against the twin: `e_u`, three rows | `<= 1e-10` each | acceptance of F-666 | `<= 1e-13` | 1000 | code verification |
| A-M | a5, T2 restated, modes: the same three models: `e_lambda`, `e_phi` | `<= 1e-10` each | acceptance of F-666 | `<= 1e-13` | 1000 | code verification |
| A-N1 | a5, T3 restated: the four calls of N1 of ADR 0593 as written there | each `Ok` | D1 | `Ok` | 1 | code verification |
| A-CY | a1 by default: CY-S-Q4 `e_u`, CY-H-Q4 `e_u`, CY-M-Q4 `e_lambda` (`k = 0 .. 3`) and `e_phi` (`k = 0, 3`) | `<= 1e-10` each | acceptance of F-666 | `<= 1e-13` | 1000 | code verification |
| A-W | a1 by default, the Beam2 half, T4 restated (H-D as acceptance): W by default against the twin, `k = 0, 1`: `max abs(lambda_h / lambda_t - 1)` | `<= 1e-10` | acceptance of F-666 | `<= 1e-13` | 1000 | code verification |
| A-RO | a2 by default: RO-B, RO-P4: `e_lambda`, `e_phi` | `<= 1e-10` each | acceptance of F-666 | `<= 1e-13` | 1000 | code verification |
| A-CM | a3 by default: CM-B, CM-P4: `e_K`, `e_M`, `e_f`, the fixed-interface and the reduced `e_lambda`, the recovered `e_u` | `<= 1e-10` each | acceptance of F-666 | `<= 1e-13` | 1000 | code verification |
| A-MP | a4 by default: MP-B, MP-P4: static `e_u`, modes `e_lambda`, `e_phi` | `<= 1e-10` each | acceptance of F-666 | `<= 1e-13` | 1000 | code verification |
| X | extras by default: X-CY-Q3 (the three cyclic rows), X-RS-Q4 (`k = 0 .. 3`), X-MP-P3 | `<= 1e-10` each | ADR 0593 (its extras) | `<= 1e-13` | 1000 | code verification |
| E | D2, the setting is a no-op: the default against `set(true)` and against `set(true)` then `set(false)`, the values listed under "The setting is a no-op" | bits equal | tolerance table, "Bit-identity of default output"; D3 | equal | 1 | code verification |
| K-0744 | T4 restated: AB-a and H-T of ADR 0744, bodies copied | each as in ADR 0744 | ADR 0744 | as there | 1 | unchanged default output |
| K-0555 | T1 restated: RF2, RF3, RF4 of ADR 0555, bodies copied | each as in ADR 0555 | ADR 0555 | as there | 1 | unchanged default output |
| K-0584 | T2 restated: N2 (the temperature load with the setting), N3 (the setting on an isotropic model changes no bit), N4 (the getter of a new model is `false`) of ADR 0584, bodies copied; and the getter returns `true` after `set(true)` | each as in ADR 0584; holds | ADR 0584; D2 | as there | 1 | unchanged default output |
| N-T | what must stay refused: Beam B and P4 by default with a temperature load, `solve_static` | refused, `LINEAR_FOLLOW_UP`, location "a temperature load" | ADR 0555 D3 | as the limit | 1 | code verification (negative control) |
| C | the comparison sees a wrong twin: by default against the perturbed twin: Beam B static `e_u`; CY-S-Q4 `e_u`; RO-B `e_lambda`; CM-B `e_K`; MP-P4 static `e_u` | `> 1e-8` each | ADR 0593 (controls C) | `9.4e-7 .. 1.0e-6` (measured there) | 94 | code verification (negative control) |
| R-T | the retired records, run once with `--ignored`: T1 fails with exactly its RF1 line missed, T2 with exactly its six N1 lines, T3 with exactly its four N1 lines, T4 with exactly its H-D line | fail there | D4 | fail there | 1 | code verification (negative control) |
| B1 | Defaults: existing tests of `rcfd-fem` that reach `check_elements` with the material or with structural elements, unedited but for the four ignore lines, pass: `--lib`, `sa_hyper_paths`, `sa_hyper_structures`, `sa_hyper_structures_f666`, `sa_cyclic_beams_default_adr0744`, `sa_hyper`, `sa_hyper_paths_f644`, `sa_hyper_prestress_f646`, `sa_hyper_state_paths_f669`, `sa_laminate`, `sa_scaffold`, `sa_geometric_struct`, `sa_cyclic_shells`, `sa_cms`, `si_beam`, `si_shell`, `si_rotating` | 0 failures | rule 7; tolerance table, "Bit-identity of default output" | 0 | 1 | unchanged default output |
| B2 | Defaults, the existing bit records: `sa_hyper_paths_default_bits`, `sa_hyper_default_bits`, `sa_geometric_struct_bits`, `sa_prestress_mixed_bits`, `sa_ortho_default_bits`, `sa_ortho_rotating_default_bits`, `sa_rotating_cyclic_shells_default_bits`, `sa_cms_default_bits`, `sa_shell3_bits`, `sa_scaffold_bits`, `sa_tie_default_bits` | 0 failures | tolerance table, "Bit-identity of default output" | 0 | 1 | unchanged default output |
| B3 | Defaults: `cargo test -p rcfd-api --release --test baseline_hashes` (`model.rs` is a shared solver file) | pass | rule 3 | pass | 1 | unchanged default output |

No row is of kind validation: the acceptance of F-666 claims agreement with the isotropic twin computed by the same code,
not with a measured case.

The registered run of the new file is `cargo test -p rcfd-fem --release --test sa_hyper_structures_default_adr0746 --
--test-threads=1 --nocapture`, once. The models are at most 77 nodes; seconds are expected, no heavy run.

### Negative controls and planned mutations

Controls: C (the twin comparison sees a twin that is wrong by `1e-6`), N-T (what must stay refused), R-T (the retired
records fail for the registered reason and no other). Mutations, each on a scratch copy with its own target directory,
the new file run once for each:

| Id | Mutation | Must be caught by |
|---|---|---|
| Z1 (committed, rule 16) | `check_elements`: the structural refusal of ADR 0555 D3 back under `!self.hyperelastic_structures` (the code of before). Patch `crates/rcfd-fem/tests/sa_hyper_structures_default_adr0746.mutation.patch` | every A row, X, C (refused); E (the default refused where the setting is taken) |
| Z2 | the whole block of ADR 0555 D3 removed (the temperature-load refusal too) | N-T, and RF2 and N2 in K-0555 and K-0584 |
| Z3 | `mat_hyper::small_strain_young_poisson`: `9 K G / (3K + G)` -> `9 K G / (3K + 2G)` | every A row but A-N1 and A-RF1 (a), and X |
| Z4 | the lift made for the shells only: a Beam2 element of the material's set still refused | the Beam B rows of A-S, A-M, A-N1, A-RO, A-CM, A-MP, and A-W, C; not the plate rows |
| Z5 | the lift made without Shell3: a Shell3 element of the material's set still refused | the P3 rows of A-S and A-M, X-CY-Q3, X-MP-P3; not the Beam B and Shell4 rows |

Mutation 7 of ADR 0593 (the copy of the setting in `unit_centrifugal_model` removed) becomes an equivalent mutation by D2
and is not run.

## Size

Size (agent): done; 0; 0; 0; 300000

## Decision rule

- **done**: every row A-RF1 to B3 is met in its one registered run, and every mutation is caught by at least one check.
  With a1 to a4 met by default and a5 made, the item F-666 is then proposed done.
- **partial**: a row is missed (recorded by rule 2) or not run, or a mutation escapes.
- **todo**: the lift cannot be made without changing the result of a model that was accepted before, or without an edit
  of `baseline_hashes.rs`, a hash table or a golden file. A changed registered hash is not re-recorded: the work stops
  and it is reported.

## What is not covered

- Everything "What still refuses a hyperelastic material afterwards" lists.
- The linear paths with structures that ADR 0584 and ADR 0593 did not run (the whirl paths, the Campbell tables, the full
  rotor, the cms assembly and the superelement, beam warping, harmonic and transient through the modes, the pre-stressed
  paths other than the beam buckling of ADR 0584) are opened by default by this lift as they were by the setting, and are
  not run here; they read the same moduli through the same three functions (ADR 0593, "Limits").
- The Mooney-Rivlin energy by default: run with the setting by ADR 0584 through the same functions; not repeated.
- The linearised material is the small-strain tangent: no claim on large rotations or finite strain of beams and shells.
- The case schema: no case key reaches the material.
- Removing `set_hyperelastic_structures` (D2).

## Addendum 1: results (2026-11-19)

Built as registered: the registration 4cd5e54f, then the code and the tests 78e6b108, then the runs. Library changes
(shared solver file `crates/rcfd-fem/src/model.rs`, lines of the new file): 994 to 995 (the doc of the field), 1127 and
1131 to 1135 (the docs of the getter and the setter), 1503 to 1516 (`check_elements`: the block of ADR 0555 D3 with its
temperature-load refusal alone; 14 lines where 26 were). `crates/rcfd-fem/src/mat_hyper.rs` 16 to 20 and 31 to 32: doc
comments only. `rotating.rs` is not touched. The four retired tests have one `#[ignore = "ADR 0746: ..."]` line each and
no other change. All runs in the release profile, each registered run made once; logs `run1.log`, `rt.log`, `b1.log`,
`b2.log`, `b3.log`, `docs_site.log`, `mutations.log`, `clippy.log` in the scratch folder.

| # | Limit | Measured | Met |
|---|---|---|---|
| A-RF1 | (a) not refused for the material; (b) `e_u <= 1e-10` | (a) `RigidBodyMotionFree { part_node: 0, part_nodes: 9, count: 6 }` (the unsupported plate); (b) 0 | yes |
| A-S | `<= 1e-10` each | Beam2 0, Shell4 0, Shell3 0 | yes |
| A-M | `<= 1e-10` each | `e_lambda` and `e_phi` 0 for Beam2, Shell4 and Shell3 | yes |
| A-N1 | each `Ok` | the four calls `Ok` | yes |
| A-CY | `<= 1e-10` each | CY-S-Q4 0; CY-H-Q4 0; CY-M-Q4 `e_lambda` 0 for `k = 0 .. 3`, `e_phi` 0 for `k = 0, 3` | yes |
| A-W | `<= 1e-10` | 0 (the eight eigenvalues of `k = 0, 1` equal those of the twin to the last bit) | yes |
| A-RO | `<= 1e-10` each | RO-B 0, 0 (Omega = 0.32788 rad/s); RO-P4 0, 0 | yes |
| A-CM | `<= 1e-10` each | CM-B and CM-P4: 0 on each of the six measures | yes |
| A-MP | `<= 1e-10` each | MP-B 0, 0, 0; MP-P4 0, 0, 0 | yes |
| X | `<= 1e-10` each | X-CY-Q3 0 on every row; X-RS-Q4 0 for `k = 0 .. 3` (Omega = 6.1786 rad/s); X-MP-P3 0, 0, 0 | yes |
| E | bits equal | 12 sets of values (1702 values) equal against `set(true)`, and the same 12 against `set(true)` then `set(false)` | yes |
| K-0744 | as in ADR 0744 | AB-a: not the beam refusal, `InvalidValue { key: "cyclic" }` (the boundary match); H-T 0 | yes |
| K-0555 | as in ADR 0555 | RF2 refused, location "a temperature load", package `a follow-up of ADR 0555`; RF3 the four rows hold; RF4 refused, key `expansion` | yes |
| K-0584 | as in ADR 0584; holds | N2 refused, "a temperature load"; N3 the four bit rows hold; N4 holds; the getter `false` by default and `true` after `set(true)` | yes |
| N-T | refused, "a temperature load" | Beam B and P4: `NotYetSupported { variant: "Hyperelastic", location: "material 0, element set 'all': a temperature load", package: "a follow-up of ADR 0555" }` | yes |
| C | `> 1e-8` each | C-B `9.677e-7`, C-CY `9.431e-7`, C-RO `9.999e-7`, C-CM `9.677e-7`, C-MP `9.439e-7` | yes |
| R-T | the retired records fail at their refusal rows only | T1: exactly "RF1 a Shell4 model refused naming LINEAR_FOLLOW_UP" (`RigidBodyMotionFree`); T2: exactly the six N1 lines (each `Ok`); T3: exactly the four N1 lines (each `Ok`); T4: exactly "H-D without the setting" (`Ok`) | yes |
| B1 | 0 failures | `--lib` 15; `sa_hyper_paths` 16 (3 ignored: 2 before and T1); `sa_hyper_structures` 5 (1 ignored: T2); `sa_hyper_structures_f666` 9 (5 ignored: 4 before and T3); `sa_cyclic_beams_default_adr0744` 6 (1 ignored: T4); `sa_hyper` 10 (4 ignored before); `sa_hyper_paths_f644` 4 (1 ignored before); `sa_hyper_prestress_f646` 2; `sa_hyper_state_paths_f669` 4; `sa_laminate` 9 (1 ignored before); `sa_scaffold` 9 (32 ignored before); `sa_geometric_struct` 9 (2 ignored before); `sa_cyclic_shells` 8 (1 ignored before); `sa_cms` 10 (1 ignored before); `si_beam` 9 (4 ignored before); `si_shell` 11 (3 ignored before); `si_rotating` 9 (1 ignored before). 0 failed | yes |
| B2 | 0 failures | the eleven bit records pass (`sa_scaffold_bits` 2 tests, `sa_cms_default_bits` 1 with 1 ignored before, the others 1 each). 0 failed | yes |
| B3 | pass | `baseline_hashes`: 3 passed, 1 ignored (as before) | yes |

Every difference against the twin is exactly 0, as on the rows of ADR 0584, ADR 0593 and ADR 0744 with the setting. The
registered run of the new file: 14 tests passed, 0.89 s.

### Mutations (scratch copy of 78e6b108, own target directory, the new file run once for each)

| Id | Registered to be caught by | Caught by |
|---|---|---|
| Z1, committed as `crates/rcfd-fem/tests/sa_hyper_structures_default_adr0746.mutation.patch` | every A row, X, C, E | all 14 tests of the file fail: every A row, X, C and E (refused, "a structural element"), and with them N-T (the default with a temperature load is refused for the element first) |
| Z2 | N-T, RF2, N2 | N-T (both rows), RF2 and N2; 3 tests fail. The models are still refused, further down and with another error (the element routines: no expansion coefficient), so the block of ADR 0555 D3 is not their only guard |
| Z3 | every A row but A-N1 and A-RF1 (a), and X | those rows (`3.1e-2 .. 3.2e-2` on displacements and eigenvalues), and H-T; 10 tests fail; A-N1, E, N-T and C pass, as registered |
| Z4 | the Beam B rows of A-S, A-M, A-N1, A-RO, A-CM, A-MP, and A-W, C; not the plate rows | those rows, and E, N-T (Beam B) and AB-a; 9 tests fail; the Shell4 and Shell3 rows, A-CY, A-RF1 and X pass |
| Z5 | the P3 rows of A-S and A-M, X-CY-Q3, X-MP-P3; not the others | those rows and E (its P3 values); 4 tests fail; the Beam B and Shell4 rows pass |

Five of five caught.

### Checks of rule 7

`cargo clippy -p rcfd-fem --all-targets -- -D warnings`: clean. Format: `python scripts/fmt_check.py rcfd-fem` (374 target
files): clean; `cargo fmt -p rcfd-fem -- --check` cannot run on this machine (os error 206, the brief). `baseline_hashes`:
row B3. `docs_site` (`cargo test -p rcfd-cli --release --test docs_site`): 2 passed, run after the topic page was
changed. `npm test` not run: `apps/studio-web` is not touched.

### Deviations

1. A-RF1 (a): the unsupported plate is refused as `RigidBodyMotionFree`, where the registration expected a refusal for a
   singular stiffness; the row asserted only "not refused for the material", which holds.
2. The two names of the brief that differ from the files, and T2 retired beyond the tests the brief names (all registered
   in the audit before the code).
3. The format check by `scripts/fmt_check.py` in place of `cargo fmt -p` (above).

### Not run, not done

- The whole workspace suite and the other integration tests of `rcfd-fem` (the lead's gate).
- `docs/adr/index.md` is not current for this ADR (`python scripts/process_report.py --check` reports that and nothing
  else): the lead's.
- Nothing of "What is not covered" was run.
- The comment of `rotating::unit_centrifugal_model` still calls the copied setting "the opt-in of ADR 0584 D1"; the line
  is without effect now and the file was left untouched (D2).

### Size

Size (agent): done; 0; 0; 0; about 270000

### Decision

By the registered rule: **done**. Every row A-RF1 to B3 is met in its one registered run and five of five mutations are
caught. Clause a5 of F-666 is made: the refusal is lifted by default, RF1 of ADR 0555, N1 of ADR 0584, N1 of ADR 0593 and
H-D of ADR 0744 are ignored records with restated copies that accept and agree with the isotropic twin exactly. Clauses a1
to a4 hold by default (A-CY and A-W, A-RO, A-CM, A-MP: 0, limit `1e-10`), and every model that was accepted before keeps
its bits (B1, B2, B3, E).

## Open questions for the maintainer

None.

## For the lead: roadmap entries

Status proposed for F-666: done.

> ADR 0746, 2026-11-19, done: the refusal of ADR 0555 D3 of a hyperelastic material on Beam2, Shell4 and Shell3 elements on the linear paths is lifted by default (the maintainer's approval of 2026-11-18), set_hyperelastic_structures kept as a documented no-op; without the setting a neo-Hookean material on the static solve, the modes, the cyclic path (Shell4, Shell3 and the Beam2 wheel sector), the rotating, cms and mpc-transformation paths gives the isotropic twin with difference exactly 0 (limit 1e-10) and the bits of the setting; RF1 of ADR 0555, N1 of ADR 0584, N1 of ADR 0593 and H-D of ADR 0744 are ignored records restated in one new file; a temperature load with the material stays refused; existing tests, bit records and baseline hashes unchanged; five of five mutations caught.

No gap and no follow-up item are proposed.
