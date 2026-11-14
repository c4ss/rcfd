# ADR 0440: F-408 registration: scaffold of the advanced tier (modules, material variants with a point context and a state layout, Shell3, the layup and the section-stiffness hook, the two extension points of the Newton driver, the complex type)

Status: **REGISTERED 2026-11-14, written before any code of F-408 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-408 was written, compiled or run: the files under "What was read" were read; the only
arithmetic done beforehand is the rounding bound of check Z1 (derived below). Results go in the addenda at the end; nothing above
the addenda is changed after a run.

Work package SA-1 of the structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md), sections 2
and 3, the entry for ADR 0311; stub [ADR 0311](0311-advanced-scaffold-modules-material-context-driver-hooks.md), kept unchanged).
Roadmap item F-408 (deps F-372, F-388, F-397). Size M. Number 0440 given by the lead. Branch `f408-advanced-tier-scaffold` from
main d3d05478, worktree `w49i-work/f408`, scratch `w49i-work/f408-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact); each registered run made once; defaults bit-identical; no edit of `baseline_hashes.rs`, of the case
and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph unchanged; no existing test edited;
clean-room. Another agent (F-524) adds a test file to `rcfd-fem` in parallel; this item adds its own test files only.

## What F-408 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The modules, material variants, point context, state layout, element kind, layup property, section-stiffness hook and the two driver extension points of ADR 0310 section 2 exist; every test of the basic and intermediate tiers (the sb_ and si_ files) compiles and passes unedited, and a registered set of models (patch test, cantilever, bar modes, the elastica of F-397, the free vibration of F-398, a Shell4 plate) gives displacements, stresses, frequencies and histories with the same bits as before the change; with an empty contribution list and no step hook the Newton driver takes the same iterations with the same residuals bit for bit; the default point context (no temperature, no time increment, global axes) reproduces the isotropic stress and tangent bit for bit; each new material variant, the element kind Shell3 and the layup property are refused by every solve with an error naming the variant and the set, never ignored; the complex type satisfies (a b) conj(b) = a |b|^2 and a/a = 1 within 1e-15 on a registered sample and prints and parses losslessly; the baseline hashes are unchanged.

Clauses: (c1) every limit registered before the run (this document); (c2) the modules of ADR 0310 section 2.2 exist (24 in
`rcfd-fem`, 4 in `rcfd-linalg`, 3 in `rcfd-post`); (c3) the material variants `Orthotropic`, `Tabulated`, `Viscoelastic`, `Creep`,
`J2`, `Hyperelastic` exist; (c4) the point context (temperature, time and time increment, material axes) exists; (c5) the state
layout (a declared state size per material and integration point, the per-element layout of a model) exists; (c6) the element kind
`Shell3` exists; (c7) the shell property `Layup { plies }` exists; (c8) the shell takes its membrane, coupling, bending and shear
stiffness from one section-stiffness function; (c9) the Newton driver has a list of extra contributions (residual and tangent) and
a step-control hook; (c10) every `sb_` and `si_` test file compiles and passes unedited; (c11) the registered models give the bits
of main d3d05478; (c12) with an empty contribution list and no step hook the driver takes the same iterations with the same
residuals bit for bit; (c13) the default point context reproduces the isotropic stress and tangent bit for bit; (c14) each new
material variant, `Shell3` and `Layup` is refused by every solve with an error naming the variant and the set; (c15) never
ignored; (c16) the complex identities within 1e-15 on a registered sample; (c17) printed and parsed losslessly; (c18) the baseline
hashes are unchanged.

ADR 0310 retires no `si_` row and no `si_` test: nothing is retired here, and no records-and-copies step is needed.

## What was read (main d3d05478, read only)

1. `AGENT_RULES.md`; ADR 0310 sections 2.1 to 2.5, the entries 0311, 0312, 0313, 0315, 0327, 0331 of section 3 and the lead's
   note (roadmap ids F-408 to F-436); the stubs 0311, 0315 (loss factors per material), 0327 (follower and arc-length through the
   driver's extension points); ADR 0291 (the intermediate scaffold: pattern of `PENDING`, `NotYetSupported`, the bit record
   procedure, its checks and mutations); ADR 0404 (F-397: the Newton driver, `ElementState`, commit and roll-back); ADR 0389 (F-398,
   the transient integrator); ADRs 0417, 0432, 0435, 0437 (beams, Shell4, shells on the transformation, shell output).
2. Code: `crates/rcfd-fem/src/{lib.rs, material.rs, model.rs (Material, DofMap::for_mesh, check_elements and the check_* family),
   mesh.rs (ElementKind, BlockProperty, add_block, set_block_property, element_volume), element.rs (element_for, Pending),
   shell.rs (whole), nonlinear.rs (whole), recover.rs (fit_basis, extrapolation_matrix), error.rs (NotYetSupported)}`; the entry
   lines of every solve (`solve.rs`, `modal.rs`, `buckling.rs`, `geometric.rs`, `cyclic.rs`, `harmonic.rs`, `mpc.rs`,
   `rotating.rs`, `transient.rs`, `assemble.rs`); `crates/rcfd-linalg/src/{lib.rs, matrix.rs (CsrMatrix API)}`;
   `crates/rcfd-post/src/lib.rs`, the `C64` of `rcfd-post::rotordyn` (not edited); the tests `si_scaffold_bits.rs` (whole),
   `si_nonlinear.rs` (the elastica cantilever, the bar, a7), `si_transient.rs` (the bar, the free run), `si_shell.rs` (the
   simply supported plate), `sb_elements.rs` (the calls of `Material::stress`).
3. **Not read:** any finite-element or numerical code outside this repository. Smith's complex division (R. L. Smith, Algorithm
   116, Comm. ACM 5 (1962) 435) is cited from memory; its two branches are derived in D9.

Findings that shape the design:

1. `Material`, `ElementKind`, `BlockProperty`, `FemError` are `#[non_exhaustive]` and built through constructors: new variants
   break no test crate. Inside `rcfd-fem` the matches on `Material` (seven: `elasticity_matrix`, `state_size`, the four accessors
   of `model.rs`, `isotropic_elastic`'s construction), on `ElementKind` (`mesh.rs` nine methods and `element_volume`, `element_for`,
   `recover.rs` two) and on `BlockProperty` (`name`, `set_block_property`) are exhaustive and get arms. No other crate matches
   `Material`; `rcfd-post::vtk_from_fem` has a wildcard that refuses a kind by name.
2. The constitutive call of ADR 0273 already takes `&PointContext` (an empty non-exhaustive struct with `Default`), and
   `sb_elements.rs` builds it by `PointContext::default()`. ADR 0310 section 2.4 point 1 was followed: the context gains fields, no
   second entry point is needed, and the default keeps the arithmetic of the call untouched.
3. `Material::state_size` exists (0 for the isotropic material) and `nonlinear::initial_states` gives `n_points x state_size`
   zeros per element; commit and roll-back are the driver's copies of `Vec<ElementState>` (ADR 0404 D5). They stay as they are.
4. Every solve entry of `rcfd-fem` calls one of `check_elements_supported`, `check_elements_supported_with_beams`,
   `check_elements_supported_with_structures` or the `check_supported*` forms built on them, all through the private
   `SolidModel::check_elements` (listed under "What was read": `solve_static_report`, `modal_analysis_with`, `stiffness_with`,
   `buckling::factors`, `cyclic::{modal_analysis_harmonic, solve_static_harmonic}`, `geometric::{Prestress::uniform,
   geometric_stiffness, prestressed_modes}`, the two `harmonic.rs` entries, `mpc::{solve_static_mpc, modal_analysis_mpc}` through
   `check_transformation_model`, `nonlinear::{solve, internal_force, tangent, initial_states}`, `rotating.rs` `check_model`,
   `transient.rs` both constructors). One refusal in `check_elements` therefore reaches every solve.
5. The Newton driver (`nonlinear::solve`) is one function over a private `Problem::attempt`. The extension points go in as
   optional work inside the existing loops, executed only when the list or the hook is given; with neither the executed
   operations are those of main (D7).
6. The shell integrates a homogeneous thickness in line in `local_stiffness` (`plane_matrix(e, nu, t)`, `plane_matrix(e, nu,
   t^3/12)`, `SHEAR_FACTOR g t`, `options.drilling g t`) and again in `stresses`. The section function must produce these numbers
   with the same operations in the same order (D6).
7. `rcfd-linalg` has no complex type; `rcfd-post::rotordyn` and `duct_modes` have their own `C64` (not edited, ADR 0310 section
   2.3).

## Design

### D1. Modules (c2)

`rcfd-fem/src/lib.rs` gains 24 `pub mod` lines in alphabetical order: `arclength`, `cms`, `contact`, `damped`, `estimate`,
`fatigue`, `follower`, `fracture`, `geometric_struct`, `gyro`, `harmonic_direct`, `laminate`, `mat_creep`, `mat_hyper`, `mat_j2`,
`mat_ortho`, `mat_table`, `mat_visco`, `nl_transient`, `nonlinear_md`, `random`, `rotor`, `shell3`, `tie`. `rcfd-linalg` gains
`complex` (filled, D9), `cskyline`, `eigen_general`, `sparse_chol`; `rcfd-post` gains `solid_acoustic`, `solid_aerodamp`,
`solid_rotor`. Every stub is a module comment naming its content and package and `pub const PENDING: Option<&str>`, the one place
that says the content has not landed, so that the landing package flips it in its own file. Registered values (ADR of the plan and
the roadmap id of the lead's note):

| Module | `PENDING` |
| --- | --- |
| `mat_ortho`, `mat_table` | `Some("ADR 0312 (F-409)")` |
| `mat_visco` | `Some("ADR 0313 (F-410)")` |
| `mat_creep` | `Some("ADR 0314 (F-411)")` |
| `harmonic_direct`; linalg `cskyline` | `Some("ADR 0315 (F-412)")` |
| `fatigue` | `Some("ADR 0316 (F-413)")` |
| `damped`; linalg `eigen_general` | `Some("ADR 0317 (F-414)")` |
| `gyro` | `Some("ADR 0318 (F-415)")` |
| `rotor`; post `solid_rotor` | `Some("ADR 0319 (F-416)")` |
| `random` | `Some("ADR 0320 (F-417)")` |
| post `solid_acoustic` | `Some("ADR 0321 (F-418)")` |
| `shell3` | `Some("ADR 0322 (F-419)")` |
| `geometric_struct` | `Some("ADR 0323 (F-420)")` |
| post `solid_aerodamp` | `Some("ADR 0324 (F-421)")` |
| `mat_j2` | `Some("ADR 0325 (F-422)")` |
| `nl_transient` | `Some("ADR 0326 (F-423)")` |
| `follower`, `arclength` | `Some("ADR 0327 (F-424)")` |
| `mat_hyper`, `nonlinear_md` | `Some("ADR 0328 (F-425)")` |
| `tie` | `Some("ADR 0329 (F-426)")` |
| `contact` | `Some("ADR 0330 (F-427)")` |
| `laminate` | `Some("ADR 0331 (F-428)")` |
| `fracture` | `Some("ADR 0332 (F-429)")` |
| `cms` | `Some("ADR 0333 (F-430)")` |
| linalg `sparse_chol` | `Some("ADR 0334 (F-431)")` |
| `estimate` | `Some("ADR 0335 (F-432)")` |

Besides `PENDING`: `mat_visco::state_size(terms)`, `mat_creep::STATE_SIZE`, `mat_j2::STATE_SIZE` (D3); `shell3` the kind data of
D4 (`element()`, `reference_nodes()`, `faces()`, `shape_gradients()`, as `hex20` had before ADR 0292); `laminate::section_stiffness`
(D6), refusing.

### D2. Material variants (c3)

`Material` (in `model.rs`) gains six non-exhaustive variants, each with a validating constructor; the sub-data types live in
`material.rs`, are plain data (`Debug, Clone, PartialEq`) and non-exhaustive:

- `Orthotropic { e: [f64; 3], nu: [f64; 3], g: [f64; 3], density, expansion: [f64; 3], axes: MaterialAxes }`: `E_1, E_2, E_3`;
  `nu_12, nu_13, nu_23`; `G_12, G_13, G_23` (the nine constants of ADR 0310 section 2.3); `MaterialAxes::{Fixed(axes),
  Cylindrical { origin, axis }, PerElement(Vec<axes>)}`, `axes` a `[[f64; 3]; 3]` whose rows are the material axes in global
  components. `Material::orthotropic(e, nu, g, density, expansion, axes)` refuses (`InvalidMaterial` naming the key) a modulus that
  is not positive and finite, a non-finite ratio or expansion, a negative or non-finite density; `MaterialAxes::fixed`,
  `cylindrical`, `per_element` refuse axes that are not orthonormal and right-handed within 1e-12 per entry of `R R^T - I` and of
  `det R - 1`, a zero or non-finite axis, an empty list (`InvalidValue`, key `"axes"`). The positive definiteness of the
  orthotropic compliance is ADR 0312's check.
- `Tabulated { base: Box<Material>, temperatures: Vec<f64>, columns: Vec<(String, Vec<f64>)> }`: properties of an elastic base
  against temperature. `Material::tabulated` refuses a base that is not `IsotropicElastic` or `Orthotropic` (key `"base"`), fewer
  than two temperatures, temperatures not strictly increasing or not finite, no column, an empty column name, a column whose length
  is not the number of temperatures, a non-finite entry.
- `Viscoelastic { base: Box<Material>, terms: Vec<PronyTerm>, shift: Option<TimeTemperatureShift> }`: `PronyTerm::new(shear, bulk,
  tau)` (relative weights in [0, 1], `tau` positive and finite); `TimeTemperatureShift::{Wlf { reference, c1, c2 }, Arrhenius {
  reference, activation }}` (finite; `c2`, `activation` positive). `Material::viscoelastic` refuses a non-elastic base, no term, a
  sum of shear or of bulk weights above 1.
- `Creep { base: Box<Material>, a, n, m }`: the time-hardening power law `d eps_c / dt = a sigma^n t^m`; refuses a non-elastic
  base, `a` or `n` not positive and finite, `m` not finite or `m <= -1`.
- `J2 { base: Box<Material>, yield_stress, isotropic, kinematic }`: refuses a base that is not `IsotropicElastic`, a yield stress
  not positive and finite, a hardening modulus negative or not finite.
- `Hyperelastic { energy: HyperelasticEnergy, density }`: `HyperelasticEnergy::{NeoHookean { mu, kappa }, MooneyRivlin { c10,
  c01, kappa }}`; refuses `mu`, `kappa` not positive and finite, `c10`, `c01` not finite or `c10 + c01 <= 0`, a negative density.

New methods: `Material::name()` ("IsotropicElastic", "Orthotropic", "Tabulated", "Viscoelastic", "Creep", "J2", "Hyperelastic")
and `Material::pending()` (`None` for `IsotropicElastic`; the module's `PENDING` for the others: `mat_ortho` for Orthotropic,
`mat_table` for Tabulated, `mat_visco`, `mat_creep`, `mat_j2`, `mat_hyper`). The accessors `youngs_modulus`, `poisson_ratio`,
`density`, `expansion` give the base's value for the four wrapping variants, the variant's density, and NaN where the variant has
no single value (documented; no solve reaches them, D5). `elasticity_matrix` and `stress` of a pending variant return
`NotYetSupported { item: "material", variant, location: "the constitutive call", package }`.

No loss factor is added (open point 3).

### D3. Point context and state layout (c4, c5, c13)

`PointContext` gains public fields `temperature: Option<f64>` (K), `time: Option<f64>`, `time_increment: Option<f64>` (s),
`axes: Option<[[f64; 3]; 3]>` (the material axes at the point, rows in global components; `None` = global axes); `Default` is all
`None`. Builders `PointContext::new()` (the default), `with_temperature(t)` (finite), `with_time(time, increment)` (finite,
`increment >= 0`), `with_axes(axes)` (orthonormal and right-handed as in D2), each `Result<PointContext>`; `is_default()`. Strain
and stress stay in global axes; the axes are for an anisotropic material to rotate into. The isotropic material's call is
unchanged: its arithmetic does not read the context (isotropy makes the axes irrelevant and its properties do not depend on
temperature or time; the thermal strain stays the load's, ADR 0273).

State layout: `Material::state_size()` gets arms: 0 for `IsotropicElastic`, `Orthotropic`, `Hyperelastic`; the base's for
`Tabulated`; `mat_visco::state_size(terms) = 6 x terms` (the internal strain of each Prony term) for `Viscoelastic`;
`mat_creep::STATE_SIZE = 7` (creep strain, accumulated equivalent creep strain); `mat_j2::STATE_SIZE = 13` (plastic strain, back
stress, equivalent plastic strain). These are declared layouts; the landing package may change its own module's number.
`nonlinear::StateLayout::of(model)` (validates the material assignment, does not refuse a pending variant: it is a layout, not a
solve) gives per element the number of points of the stiffness rule and the values per point, `total()`, `offset(e)` and
`empty_states()`; for any model of the earlier tiers `empty_states()` equals `initial_states(model)`. Commit and roll-back are not
changed (ADR 0404 D5).

### D4. Element kind Shell3 (c6)

`ElementKind::Shell3`: 3 nodes, 3 corners, `name` "Shell3", not solid, `pending()` = `shell3::PENDING`, `face_corners` 0,
`edges` `[[0, 1], [1, 2], [2, 0]]`, `faces`, `reference_nodes`, `shape_gradients` from `shell3` (empty), `element_volume` NaN,
`element_for` the placeholder `shell3::element()` (a `Pending` naming "ADR 0322 (F-419)"), `fit_basis` and `extrapolation_matrix`
empty; `DofMap::for_mesh` gives its nodes six unknowns (with Beam2 and Shell4). `set_block_property` admits `Shell` and `Layup` on
Shell3 and Shell4. A mesh without a Shell3 block runs exactly the code it ran before.

### D5. Layup property and the refusals (c7, c14, c15)

`mesh.rs`: `Ply::new(material: Material, thickness, angle)` (thickness positive and finite, angle in radians finite, the ply
material any variant) and `BlockProperty::Layup { plies: Vec<Ply> }` with `BlockProperty::layup(plies)` (refuses an empty list);
`name()` "Layup".

`SolidModel::check_elements` (private; reached by every solve, finding 4) gains, in this order, per block after the existing kind
and formulation refusals: a block whose property is `Layup` is refused as `NotYetSupported { item: "shell property", variant:
"Layup", location: "block {b}", package: laminate::PENDING }`; then, after the block loop, for each material `(set, m)` of the model
in order, `m.pending()` is refused as `NotYetSupported { item: "material", variant: m.name(), location: "material {i}, element set
'{set}'", package }`. `Shell3` is refused by the existing `pending()` test as `{ item: "element kind", variant: "Shell3", location:
"block {b}", package: "ADR 0322 (F-419)" }`. A model of the earlier tiers passes both new loops without any change of a number.
The layup check stands before the "path without shells" refusal, so the layup is named on every path.

### D6. Section-stiffness hook (c8)

`shell.rs` gains `SectionStiffness { membrane: [[f64; 3]; 3], coupling: Option<[[f64; 3]; 3]>, bending: [[f64; 3]; 3], shear:
ShearStiffness, drilling: [f64; 2] }` with `ShearStiffness::{Isotropic(f64), Matrix([[f64; 2]; 2])}` (resultants `N = A eps + B
kappa`, `M = B eps + D kappa`, `Q = H gamma`; the drilling penalty is `alpha drilling[0] drilling[1]`, the pair `(G, t)` so that
the product keeps the order of main), and:

- `homogeneous_section(material, thickness)`: `membrane = plane_matrix(E, nu, t)`, `bending = plane_matrix(E, nu, t * t * t /
  12.0)`, `coupling = None`, `shear = Isotropic(SHEAR_FACTOR * g * t)`, `drilling = [g, t]`, with `(E, nu, g) = moduli(material)`:
  the operations of main in their order (finding 6).
- `section_of(property, material) -> Result<SectionStiffness>`: the hook. `Shell { thickness }` gives `homogeneous_section`;
  `Layup { plies }` gives `laminate::section_stiffness(plies)`, which refuses now (`NotYetSupported { item: "shell property",
  variant: "Layup", location: "the section stiffness", package }`) and is replaced by ADR 0331 in its own file.
- `local_stiffness_section(facet, section, options)`: the element with the section's matrices. With `coupling: None` and
  `Isotropic(ds)` it executes the operations of main's `local_stiffness`; `Some(B)` adds `w (B_m^T B B_b + B_b^T B^T B_m)` per
  point; `Matrix(H)` uses `sum_ab H_ab s_a s_b` in place of `ds (s_0 s_0 + s_1 s_1)`. `local_stiffness(f, t, material, options)`
  and `element_stiffness*` keep their signatures and call it with `homogeneous_section`; `element_stiffness_section(coords,
  section, options)` is new.
- The model paths (`model_element_stiffness`, `stresses`) take the section from `section_of` of the block's property; `stresses`
  forms `N = A eps (+ B kappa)`, `M = D kappa (+ B eps)`, `Q` from the section, the surface stresses from the thickness as now.
  The mass and the loads keep the thickness (a layup's mass is ADR 0331's).

### D7. Newton driver extension points (c9, c12)

In `nonlinear.rs`:

- `trait Contribution { fn name(&self) -> &str; fn add(&self, u: &[f64], load_factor: f64, force: &mut [f64], tangent: &mut
  Vec<(usize, usize, f64)>) -> Result<()>; }`: at every iteration the driver calls each contribution in list order with the
  current displacement and the attempt's load factor; `force` (one value per equation, zero on entry) is added to the internal
  force and `tangent` triplets to the tangent. An external load that depends on `u` (a follower pressure) enters with a minus
  sign; contact enters as a force. Triplets on the tangent's pattern are added in place in triplet order; any triplet off the
  pattern makes the driver rebuild the matrix on the union pattern (`CsrMatrix::from_triplets` of all entries). The residual,
  its reference norm and the reactions use the internal force with the contributions.
- `trait StepControl { fn name(&self) -> &str; fn load_factor(&mut self, committed: f64, proposed: f64, step: usize, halvings:
  usize) -> Result<f64>; fn converged(&mut self, increment: &LoadStep) {} fn failed(&mut self, committed: f64, attempted: f64) {}
  }`: before each attempt the hook is given the committed load factor and the load-control proposal and returns the attempt's
  target, which must satisfy `committed < value <= the user step's target` (else `InvalidValue`, key `"step control"`); after the
  attempt it is told the outcome. Halving and the step loop are the driver's as in ADR 0404.
- `DriverExtensions<'a> { contributions: Vec<&'a dyn Contribution>, step_control: Option<&'a mut dyn StepControl> }`, `Default`
  empty; `solve_with(model, prescribed, forces, options, extensions)`; `solve(...)` is `solve_with(..., DriverExtensions::default())`.
  An error of a contribution ends the attempt as a failure naming the contribution (`"contribution '{name}': {error}"`).
- With an empty list and no hook the code executes no extra operation: the contribution block and the hook calls are behind
  `if` tests on the list and the option, and the arithmetic of the attempt is unchanged (finding 5).

The per-iteration load-factor correction of arc-length (Riks, Crisfield) is not a hook of this driver: ADR 0310 section 2.3 lists
`arclength::trace` as its own analysis function, which will use `internal_force`, `tangent` and the contributions (open point 2).

### D8. Files and what stays

Touched: `rcfd-fem/src/{lib.rs, model.rs, material.rs, mesh.rs, element.rs, shell.rs, nonlinear.rs, recover.rs}` (additions
and arms; `recover.rs` beyond the stub's list for the two `ElementKind` matches, as ADR 0291 D8), the 24 stubs;
`rcfd-linalg/src/{lib.rs, complex.rs}` and three stubs; `rcfd-post/src/lib.rs` and three stubs. Tests: `crates/rcfd-fem/tests/
sa_scaffold_bits.rs` (the record, committed before the library code) and `crates/rcfd-fem/tests/sa_scaffold.rs`;
`crates/rcfd-linalg/tests/sa_complex.rs`. Not touched: the solvers, assembly, loads, mass, recovery arithmetic, the transient,
modal, harmonic, cyclic, rotating paths, `rcfd-io`, `rcfd-api`.

### D9. Complex type (c16, c17)

`rcfd_linalg::complex::C64 { re, im }` (`Copy`, `PartialEq`, `Debug`, `Default`): `new`, `ZERO`, `ONE`, `I`, `from_real`,
`conj`, `norm_sqr = re^2 + im^2`, `abs = re.hypot(im)`, `scale`, `exp_i(theta)`, `Add`, `Sub`, `Mul` (`(a c - b d, a d + b c)`),
`Neg`, `Div` by Smith's method: for `|c| >= |d|`, `r = d / c`, `den = c + d r`, `((a + b r)/den, (b - a r)/den)`; else `r = c /
d`, `den = c r + d`, `((a r + b)/den, (b r - a)/den)` (derivation: multiply numerator and denominator of `(a + i b)/(c + i d)` by
`(c - i d)/c` or `(c - i d)/d`; it avoids forming `c^2 + d^2`), the `Assign` forms, `Mul<f64>`, `Div<f64>`; vector helpers
`dot_unconj(x, y) = sum x_k y_k` and `norm2(x)` in index order. `Display` writes `{re:?}+{im:?}i`, or `{re:?}-{|im|:?}i` when the
sign bit of `im` is set (Rust's `Debug` of `f64` is the shortest string that parses back to the same bits); `FromStr` splits at the
last `+` or `-` that is not the first character and not after an `e` or `E`, requires the trailing `i`, and parses both parts.

Rounding bound of Z1 (the only arithmetic done beforehand), `u = 2^-53`: a component of a product, `x1 x2 - y1 y2`, carries at most
`u (|x1 x2| + |y1 y2|) + u |result| <= 2 u |a| |b|`, so `|fl(a b) - a b| <= 2 sqrt(2) u |a| |b|`; the second product adds the
propagated error times `|b|` and its own, `<= 4 sqrt(2) u |a| |b|^2 ~ 6.3e-16 |a| |b|^2`; `fl(|b|^2)` and its product with `a`
add `<= 3 u |a| |b|^2 ~ 3.3e-16`. Worst case `~9.6e-16 |a| |b|^2`, under 1e-15; typical errors are a fraction of it. For `a/a`
Smith gives `re = 1` exactly (the same expression on top and bottom) and `|im| <= ulp(b)/|den|`, `~2.2e-16`.

## Checks (limits fixed now)

Fast checks in `crates/rcfd-fem/tests/sa_scaffold.rs`, the bit record in `crates/rcfd-fem/tests/sa_scaffold_bits.rs`, the complex
checks in `crates/rcfd-linalg/tests/sa_complex.rs`; release profile, each run once (`cargo test --release -p rcfd-fem --test
sa_scaffold --test sa_scaffold_bits -- --nocapture --test-threads=1` and `-p rcfd-linalg --test sa_complex`).

**Bit record procedure (c11, c12, c13).** `sa_scaffold_bits.rs` uses only the API of main d3d05478. Before any library code it
is run once in the worktree at main d3d05478 plus this ADR and the file itself (no library line changed); it prints one FNV-1a 64
digest of the `to_bits()` of each quantity (with the count) per model; the printed digests become the constants `MAIN` and the
file is committed before the library code. After the change it runs unchanged and compares. Models (E = 2e11, rho = 7850 unless
stated):

- **P (patch)**, **C (cantilever, direct and PCG `BlockJacobi` 1e-8 / 20000)**, **B (bar modes, Hex8 and Tet10, four modes)**,
  **K (the stiffness of P and C)**: the models of ADR 0291 exactly (nu = 0.3); quantities as there.
- **E (the elastica of F-397)**: the cantilever of `si_nonlinear.rs` (Hex20 from `hex_box([0; 3], [1, 0.02, 0.02], [50, 1, 1])`,
  nu = 0, `xmin` clamped, traction `alpha E I / L^2 / h^2` along y on `xmax`) at alpha = 1 with `NewtonOptions::new(5)`:
  displacement, reactions, per increment the load factor, the residual list, the energy list, the halvings, the iteration count
  (all increments concatenated), and the second Piola-Kirchhoff stress of every element at the final state
  (`element_second_piola_kirchhoff`).
- **H (halving)**: the bar of `si_nonlinear.rs` a7 (Hex8 `[1, 0.5, 0.5]`, `[2, 1, 1]`, nu = 0, three symmetry planes, traction
  `E 1.2 (1.2^2 - 1)/2` on `xmax`) with `NewtonOptions::new(1).max_iterations(4)` (one halving on main): the same quantities as E
  without the stress.
- **F (free vibration of F-398)**: the bar of `si_transient.rs` (Hex8 `[1, 0.1, 0.1]`, `[20, 1, 1]`, nu = 0, y and z fixed, `xmin`
  x fixed), `u0` = the first mode of `modal_analysis(ModalOptions::new(2))`, `v0 = 0`, `omega_1 = sqrt(eigenvalue_1)`: F1 Newmark
  average acceleration, `dt = 0.1/omega_1`, 200 steps; F2 generalised-alpha `rho_inf = 0.8` with `Rayleigh::from_ratio(0.02,
  omega_1, 5 omega_1)`, the same `dt`, 100 steps: the history of displacement, velocity and acceleration over all steps (step 0
  included).
- **S (Shell4 plate)**: the simply supported plate of `si_shell.rs` (16 x 16 on the unit square, t = 0.01, nu = 0.3, pressure 1000
  Pa, its prescribed list): displacement, reactions, `shell::stresses` (membrane, bending, shear, top, bottom of every element),
  the stiffness values, and the first four frequencies and modes of `modal_analysis_with` with the same prescribed list.
- **Q (constitutive call)**: 64 strains from a fixed generator (LCG `x = x 6364136223846793005 + 1442695040888963407`, seed
  `0xF408_0440`, components `(x >> 11) 2^-53 - 0.5` times 1e-3), the isotropic material `(2e11, 0.3)`: stress and tangent of
  `stress(strain, &PointContext::default(), &[])`.

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| B1 | c11 | P (Hex8, Tet10): displacement, reactions, stress; K pattern and values | digests equal main's | D8: no arithmetic changed on these paths |
| B2 | c11 | C direct and PCG: displacement, reactions, stress, nodal stress; K of C | equal | D8 |
| B3 | c11 | B (Hex8, Tet10): eigenvalues, frequencies, modes | equal | D8 |
| B4 | c11, c12 | E: displacement, reactions, load factors, residuals, energies, halvings, iterations, stress | equal | D7: no extra operation without extensions |
| B5 | c12 | H: the same quantities (a run with a halving) | equal | D7 |
| B6 | c11 | F1, F2: displacement, velocity, acceleration histories | equal | transient path not touched |
| B7 | c11 | S: displacement, reactions, the five stress arrays, stiffness values, frequencies, modes | equal | D6: the section repeats main's operations |
| B8 | c13 | Q: stress and tangent | equal | D3: the isotropic call does not read the context |
| D1 | c12 | `solve_with(..., DriverExtensions::default())` against `solve` for E and H: `u`, reactions, every `LoadStep` field | `to_bits` equal | D7 |
| D2 | c9 | H with an identity hook (returns `proposed`): every quantity of D1 against `solve`; the hook saw `converged` once per increment and `failed` as many times as the halvings of the last increment of each step | bits equal; counts equal | D7 |
| D3 | c9 | the uniform-stretch bar of `si_nonlinear.rs` a3 (Hex8, the H mesh, `NewtonOptions::new(1)`) with a hook capping the attempt at `committed + 0.5` against `solve` with `NewtonOptions::new(2)`: `u`, residuals, energies, load factors | bits equal | the same targets 0.5 and 1 (`0.5 + 0.5 = 1` exactly) |
| D4 | c9 | a hook returning `committed`, and one returning `target + 0.25`: `Err(InvalidValue)` with key `"step control"` | as stated | D7 |
| D5 | c9 | the a3 bar (E) with a contribution that adds the model's own `internal_force` and `tangent` (as triplets) against `solve` of the same bar with E = 4e11: `u`, reactions, residuals, energies | bits equal | D(2E) = 2 D(E) exactly in binary; `x + x = 2x` exactly; the sums scale by 2 exactly |
| D6 | c9 | the a3 bar with a contribution adding zero triplets off the pattern at (first, last free equation) and its transpose: `u` against `solve` | max `|u - u_ref|` / max `|u_ref|` <= 1e-10 | the union pattern; the factor may order differently |
| D7 | c9 | a contribution returning `Err` on the a3 bar: `Err(Solver)` whose reason contains the contribution's name | as stated | D7 |
| D8 | c9 | the helper that adds triplets to a CSR matrix on a 3 x 3 example (one in-pattern entry, one off): every entry equals the dense sum | exact | D7 |
| M1 | c3 | the six constructors build valid data; `name()` and `pending()` as registered (D2, D1 table) | as stated | D2 |
| M2 | c3 | refused data: orthotropic with `E_2 = 0`, with a NaN ratio; axes not orthonormal, left-handed, a zero cylindrical axis, an empty per-element list; tabulated with one temperature, decreasing temperatures, a short column, a J2 base; Prony weights summing to 1.5, `tau = 0`; creep `n = 0`, `m = -1`; J2 on an orthotropic base, yield 0, kinematic -1; neo-Hookean `kappa = 0`, Mooney-Rivlin `c10 = c01 = 0`; ply thickness 0, empty layup | each `Err` | D2, D5 |
| M3 | c5 | `state_size()`: isotropic 0, orthotropic 0, tabulated 0, viscoelastic with 3 terms 18, creep 7, J2 13, hyperelastic 0; `StateLayout::of` of C and of P-Tet10: values per point 0, points 8 and 4 per element, `empty_states()` equal `initial_states` | as stated | D3 |
| M4 | c3, c15 | `elasticity_matrix` and `stress` of each pending variant: `NotYetSupported` naming the variant | 12 of 12 | D2 |
| P1 | c4, c13 | `PointContext::default()` has every field `None` and `is_default()`; the isotropic stress and tangent at the Q strains with a context of temperature 350 K, time 1 s, increment 0.1 s and the axes rotated by 0.3 rad about (1, 2, 3) are bit-equal to the default's | as stated | D3 |
| P2 | c4 | refused contexts: temperature NaN, increment -1, time infinite, axes scaled by 1.001, axes with one row negated | each `Err` | D3 |
| K1 | c6 | `ElementKind::Shell3`: name, 3 nodes, 3 corners, not solid, `pending()` = "ADR 0322 (F-419)", three edges, `element_for(Shell3).stiffness` refused naming Shell3; a mesh of two Shell3 elements builds, `Shell` and `Layup` properties set on it, `Solid` refused | as stated | D4 |
| R1 | c14 | each of the six pending materials on C's mesh (material 0 on "all", the three `xmin` constraints): E1 `solve_static`, E2 `solve_static_with` (CG `BlockJacobi`, `threads(8)`), E3 `modal_analysis`, E4 `assemble::stiffness`, E5 `nonlinear::solve`, E6 `transient::Integrator::new`, E7 `buckling::factors` and E8 `geometric::prestressed_modes` (with a uniform prestress built on the isotropic twin), E9 `mpc::solve_static_mpc`, E10 `mpc::modal_analysis_mpc` | each `NotYetSupported { "material", name, "material 0, element set 'all'", package }`, message contains the name and the set | D5 |
| R2 | c14 | a Shell3 block (the 16 x 16 plate's squares split into two triangles each, thickness 0.01, its prescribed list): E1 to E6, E9, E10 | `{ "element kind", "Shell3", "block 0", "ADR 0322 (F-419)" }` | D5 |
| R3 | c14 | the plate S with `Layup` (two plies of the isotropic material, 0.005 m, angles 0 and pi/2) on block 0: E1 to E6, E9, E10 | `{ "shell property", "Layup", "block 0", "ADR 0331 (F-428)" }` | D5 |
| R4 | c15 | `shell::section_of(Layup)` and `laminate::section_stiffness`: `NotYetSupported` naming Layup | as stated | D6 |
| H1 | c8 | `section_of(Shell { t })` equals `homogeneous_section(material, t)`; `element_stiffness_section` with it is bit-equal to `element_stiffness` on two facets (the plate's element 0 and a warped facet of `si_shell.rs` size) | bits equal | D6 |
| H2 | c8 | the coupling path: on the unit square facet in the xy plane, with `B = [[1, 2, 3], [2, 5, 4], [3, 4, 6]] 1e5`: for `u_A` (`u_x = 1e-3 x`) and `u_B` (`theta_y = 2e-3 x`), `u_A^T K u_B` against `area eps kappa B_00` and, with `u_A'` (`u_y = 1e-3 y`), against `area eps kappa B_10` | relative 1e-12 | the cross terms of membrane, shear and drilling vanish for these fields (D6); 2 x 2 Gauss is exact for constant strains |
| H3 | c8 | `ShearStiffness::Matrix([[ds, 0], [0, ds]])` against `Isotropic(ds)`: stiffness | max relative entry difference <= 1e-14 | the same sums in another order |
| X1 | c2 | `sa_scaffold` compiles against the 31 modules by path; every `PENDING` equals the D1 table | as stated | D1 |
| Z1 | c16 | 200 pairs `(a, b)` from the generator of Q (seed `0xF408_0441`; components `s m 10^k`, `m` in [1, 10), `k` in -8..=8, `s` a random sign) plus `(1, 0)`, `(0, 1)`, `(-3, 4)`, `(x, x)`, `(x, -x)` for `x = 0.7`, `(1, 1e-20)`, `(1e-20, 1)`: `|(a b) conj(b) - a |b|^2| / (|a| |b|^2)` and `|a / a - 1|` | each <= 1e-15 | the bound of D9 |
| Z2 | c17 | `to_string` then `parse` of every `C64` of Z1 and of the pairs built from `{0, -0, 5e-324, 2.2250738585072014e-308, 1.7976931348623157e308, inf, -inf, 0.1, 1/3, -2.5e-7}` (100 pairs): `re` and `im` `to_bits` equal; `NaN` parses to a NaN | 100 % | D9 |
| Z3 | c16 | `I * I = -1`, `exp_i(pi/2)` within 1e-16 of `I`, `dot_unconj`, `norm2` on a 3-vector against the written sums | exact or as stated | D9 |
| S1 | c10 | `cargo test --release -p rcfd-fem` (every `sb_`, `si_` and other test file, and the unit tests), `-p rcfd-linalg`, `-p rcfd-post` (tests and unit tests); `git diff d3d05478 -- 'crates/*/tests/s[bi]_*'` empty | 0 failed, no file edited | finding 1 |
| S2 | c10 | `cargo check --workspace --all-targets` | no error | finding 1 |
| G1 | c18 | `cargo test -p rcfd-api --release --test baseline_hashes`; the file unchanged | passes | D8 |

**Negative controls** (each must hold, else its check is not sensitive):

| Id | Control | Expected |
| --- | --- | --- |
| N1 | B1's digest of the Hex8 patch displacement with one entry moved by one ulp | differs |
| N2 | D5's displacement against `solve` with E = `next_up(4e11)` | some entry differs |
| N3 | the isotropic twins of R1 (C), R2 (the Shell4 plate) and R3 (the plate with `Shell { 0.01 }`) through every entry of their row | each `Ok` |
| N4 | Z1's measure for `(a b) b` in place of `(a b) conj(b)` on the sample | above 1e-3 for some pair |

**Planned mutations** (on a `git archive` copy of the final tree in `w49i-work/f408-scratch` with its own target directory; each
alone, by exact text replacement; the named test files run once; the file restored):

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `check_elements`: the material loop skipped | R1 |
| MU2 | `check_elements`: the layup test skipped | R3 |
| MU3 | `shell3::PENDING = None` | R2, X1 |
| MU4 | `homogeneous_section`: bending `t * t * t / 12.0` becomes `t * (t * t / 12.0)` | B7, H1 |
| MU5 | the contributions left out of the reactions | D5 |
| MU6 | the contributions' tangent triplets dropped | D5 |
| MU7 | the hook's value ignored (the driver keeps `proposed`) | D3 |
| MU8 | `StepControl::failed` never called | D2 |
| MU9 | `Display` of `C64` with `{:.15e}` | Z2 |
| MU10 | `conj` returns `self` | Z1 |
| MU11 | `with_axes` without the orthonormality test | P2 |
| MU12 | `mat_visco::state_size` returns 6 | M3 |
| MU13 | the driver's halving `dl *= 0.5` becomes `dl *= 0.25` | B5 |
| MU14 | the coupling term of `local_stiffness_section` added once (no transpose term) | H2 |

## Decision rule

**Done** if every check meets its limit, every negative control holds and every mutation is caught by its registered check on the
first run; **partial** if a check misses or is not run, a mutation survives, or a check is met only after an amendment made after
its run; **todo** if the scaffold does not compile or a bit of the earlier tiers changes and cannot be restored without changing a
default.

## Limits (not covered)

- No analysis numerics: no new material, Shell3 or layup computes anything; their packages do (0312 to 0335).
- The coupling and matrix-shear paths of the section hook are reached only through `element_stiffness_section` (H2, H3); no model
  reaches them until ADR 0331. The layup's mass, loads and ply stresses are ADR 0331's.
- The elements of the earlier tiers still form one elasticity matrix per element (`elasticity_matrix`), not a call per point
  (ADR 0310 section 2.4 point 2); the first package with a non-constant material (ADR 0312) moves them to the per-point call.
- `Constraint::Tie` and the cyclic matching rule of ADR 0310 section 2.3 are not in this item's acceptance and are not added
  (ADR 0329 adds them).
- The step-control hook sets the target of each attempt; the per-iteration load-factor correction of arc-length is not a hook
  (D7).
- `StateLayout` is per element (as `ElementState`), not one array per block as ADR 0310 section 2.3 words it; the saved state
  of the integrators is not touched.
- The refusal by "every solve" is checked on ten entries (R1) and eight (R2, R3); the other entries are covered by finding 4
  (inspection), not by a test.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The material variants carry their sub-data as plain structures in `material.rs` with validating constructors, the deeper
   checks (positive definiteness, table interpolation) left to their packages. Taken on the maintainer's standing instruction of
   2026-11-11 (take the recommendation).
2. The step-control hook is a per-attempt target with outcome callbacks; arc-length is the separate `arclength::trace` of ADR 0310
   section 2.3 built on the public driver pieces. Taken on the maintainer's standing instruction of 2026-11-11 (take the
   recommendation).
3. No loss factor is added: ADR 0315 owns where it lives (a field of the elastic variants or a per-set setting of the harmonic
   solve); ADR 0311's goal does not list it. Taken on the maintainer's standing instruction of 2026-11-11 (take the
   recommendation).
4. `NotYetSupported` names the package as "ADR 03xx (F-4xx)", the plan's number and the roadmap id. Taken on the maintainer's
   standing instruction of 2026-11-11 (take the recommendation).

## Amendment 1 (2026-11-14, written and committed after the library code and before any run of `sa_scaffold.rs`)

Two set-up defects of the registration, found while writing the test file and before any run of a registered check:

1. **N3 for the shell twins.** The row of R2 and R3 includes E5 (`nonlinear::solve`) and E6 (`Integrator::new`), which refuse
   every Shell4 model on main (ADR 0404 takes solids only; the transient path does not take shells, ADR 0432 D3), so the isotropic
   Shell4 twin cannot give `Ok` there, and E2's block-Jacobi CG on the thin plate has no registered convergence. Corrected: N3 for
   the Shell4 twins (the plate with `Shell { 0.01 }`, which is also the Shell3 twin) is `Ok` through E1, E3, E4, E9, E10; through
   E5 and E6 the twin is refused with an error that names Shell4 and neither Shell3 nor Layup; E2 is called with the direct
   solver and `threads(8)` for the shell rows of R2, R3 and N3 (the refusal comes before the solver in every case). N3 for C is
   unchanged (all ten entries `Ok`).
2. **H3's measure.** "Max relative entry difference" is defined as `max_ij |K1_ij - K2_ij| / max_ij |K2_ij|` (an entry-wise
   ratio is undefined at zero entries). The limit 1e-14 stays.

Nothing else changes.

## Amendment 2 (2026-11-14, written and committed after the first run of `sa_scaffold.rs`, before the corrected runs)

The single registered run of `sa_scaffold.rs` (commit 144c2d4a) showed two set-up defects; nothing in the library is changed:

1. **D6.** The pair "first and last free equation" of the bar (equations 3 and 35) is on the tangent's pattern (the two-element
   bar couples them), so D6 added zeros in place and did not exercise the off-pattern path: the set-up assertion "the pair is off
   the pattern" failed (the measure itself was 0, met, but on the wrong path). Corrected as D6b: the pair is the smallest free
   equation `i` and the smallest free equation `j` not in row `i` of the stiffness pattern (asserted off the pattern before the
   solve); the limit (1e-10) is unchanged. The test `d5_to_d8_contributions` stays as an ignored record; D5, D7, D8 and N2 were
   met in it and are not re-run.
2. **R2.** The Shell3 model of R2 was built with the plate's `Load::shell_pressure`, which `add_load` refuses for a Shell3 element
   (a shell pressure acts on Shell4, ADR 0432), so the test function stopped before R2, R3 and R4 ran. Corrected as R2b to R4b:
   the Shell3 model (and only it) has no load; everything else as registered. The test `r1_to_r4_n3_refusals` stays as an
   ignored record; R1 (60 of 60) and N3 (C: 10 of 10; the plate: E1, E3, E4, E9, E10 `Ok`, E2 recorded) were met in it and are
   not re-run.

By the decision rule, D6, R2, R3 and R4 are met (if they are) only after an amendment made after their run.
