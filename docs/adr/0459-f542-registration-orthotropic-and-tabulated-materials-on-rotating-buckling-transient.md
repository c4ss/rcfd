# ADR 0459: F-542 registration: orthotropic and tabulated materials on the rotating, buckling and transient paths

Status: **REGISTERED 2026-11-14, written before any code of F-542 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-542 was written, compiled or run, except the bit record of the default paths (B1,
a test file using only the API of main 2504fd62, run once at that commit with no library line changed) and the arithmetic of
the reference values in a numpy script of the scratch folder (both disclosed below). Results go in the addenda at the end;
nothing above the addenda is changed after a run.

Parent: [ADR 0457](0457-f537-registration-orthotropic-and-tabulated-materials-on-modal-paths.md) (F-537: the per-point
stiffness `assemble::point_stiffness` on the modal analysis, the pre-stressed modes and the transformation path; its refusal
list; its equivalent mutant Mu7 on the entry check of `buckling::factors`; its remaining questions 2, recommendation (a)
"the rotating modes first", and 3, cylindrical axes with a cyclic constraint about the same axis). Grandparent:
[ADR 0452](0452-f409-registration-orthotropic-elasticity-and-temperature-tables.md) (F-409: the two variants, the per-point
elasticity, `mat_ortho::FOLLOW_UP = "a follow-up of ADR 0452"`). Work package SA-2 of the structural solver, advanced tier
([ADR 0310](0310-structural-solver-advanced-tier-plan.md)). Roadmap item F-542 (deps F-537). Number 0459 given by the lead.
Branch `f542-work` from main 2504fd62, worktree `w49i-work/f542`, scratch `w49i-work/f542-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate
graph unchanged; no existing test edited except the records-and-copies step of the maintainer's standing decision (ADR 0291
pattern; D6); clean-room. Another agent (F-543) edits `tie.rs` and `mpc.rs` concurrently: F-542 does not edit them.

## What F-542 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The rotating modes (stress stiffening, spin softening), the buckling factors and the transient integrator take the orthotropic and tabulated materials of ADR 0452 through the per-point stiffness; the rotating modes of an orthotropic disc or blade with cylindrical axes match a closed form or a registered reference within registered limits; isotropic constants in an orthotropic material give the results of the isotropic material within 1e-12 on stocky models; a cyclic constraint with cylindrical axes about its own axis is opened or refused by name; the paths still not opened refuse the variants by name; default paths bit-identical.

Clauses: (a0) every limit registered before the run (this document); (a1) the rotating modes (stress stiffening, spin
softening), the buckling factors and the transient integrator take both variants through the per-point stiffness; (a2) the
rotating modes of an orthotropic blade with cylindrical axes against a closed form or a registered reference within
registered limits; (a3) isotropic constants in an orthotropic material give the isotropic results within 1e-12 on stocky
models (rotating, buckling, transient); (a4) a cyclic constraint with cylindrical axes about its own axis opened or refused
by name; (a5) the paths not opened refuse both variants by name; (a6) default paths bit-identical. The brief of the lead adds:
(b1) the reference derived or chosen before the registration, with registered estimates of the discretisation error, the
set-up not tuned; (b2) F-537's question 3 decided only if the item needs it, as an opt-in, and recorded.

## What was read (main 2504fd62, read only)

1. `AGENT_RULES.md`; ADR 0457 whole; ADR 0452 (D3 to D6, the refusal rule); ADR 0403 (the rotating part: `K + Omega^2
   (K_sigma,1 - M_perp)`, the unit centrifugal pre-stress, arithmetic 4: `Q = 1.193336`); ADR 0408 (the flap-lag identity of a
   solid beam, `1 - 2 f_x`).
2. Code: `crates/rcfd-fem/src/{rotating.rs (the module text, `spin_softening_mass`, `RotatingPart`, `check_model`,
   `RotatingSector::new`, `with_unit_prestress`, `frame`), rotating/whirl.rs (the entry points and their use of
   `RotatingPart::operator` and `modal_analysis`), buckling.rs (whole), transient.rs (`Integrator::new`, `with_motion`,
   `on_transformation`, `eliminated`, `start`, `admissible`), geometric.rs (whole), assemble.rs (whole), mat_ortho.rs (whole),
   model.rs (the `check_*` family, `Material`), mpc.rs (`uses_transformation`, `check_transformation_model` with ADR 0457 D3),
   thermal.rs (`temperature_field`), loads.rs (`nodal_load_vector`), recover.rs (the per-point stress), cyclic.rs and
   harmonic.rs, harmonic_direct.rs, damped.rs, nonlinear.rs (their entry checks)}`; the tests `sa_ortho_modal.rs` (whole),
   `sa_ortho_modal_default_bits.rs`, `si_rotating.rs` and `si_rotating_flap_lag.rs` (the beam set-ups and `share`),
   `si_rotating_cyclic.rs` (the sector D), `sa_scaffold.rs` (grep: the rows naming the variants are records or filter them).
3. **Not read** (cited from memory, for the reader only, every value used is derived below): R. V. Southwell and B. S. Gough
   (1921), the coefficient `omega^2 = omega_0^2 + S Omega^2` of a rotating blade; S. P. Timoshenko, *Vibration Problems in
   Engineering* (the beam with shear and rotary inertia); F. Engesser (1891), the column with shear,
   `P = P_E/(1 + P_E/(kappa G A))`; N. M. Newmark (1959), the average-acceleration method and its period elongation.

Findings that shape the design:

1. `RotatingPart::new` and `with_unit_prestress` call `check_model` (`check_supported`: solids, the variants refused), then
   `stiffness_with` (refuses the variants), the public `geometric_stiffness` (refuses them), `consistent_with` and
   `spin_softening_mass` (density only, no material check). `RotatingPart::modes(0)` is `modal_analysis` (opened by ADR
   0457). The whirl analyses of `RotatingPart` and `RotatingSector` use `operator` (the part's `K`, `K_sigma,1`, `M_perp`) and
   the Coriolis matrix (density only): opening the part opens them with the same `K`.
2. The pre-stress at 1 rad/s is the static solve of `unit_centrifugal_model`, a copy with the model's materials, constraints
   and supports and the centrifugal load only: the temperature field is not copied, so a `Tabulated` material would be
   refused there (`"temperature"`, `mat_ortho::check_model`), and if the field were copied the thermal strain would enter the
   pre-stress at 1 rad/s and be scaled by `Omega^2`, which is wrong. ADR 0457 D4 takes a tabulated material's stiffness at the
   model's temperature field on every dynamic path; the same rule here needs the field in the copy without its thermal strain
   (D2).
3. `buckling::factors` checks `check_supported`, then `geometric_stiffness` and `stiffness_with`, whose own checks refuse the
   variants with the same error (the cause of ADR 0457's equivalent mutant Mu7). The transformation path does the same with
   `add_supports`.
4. `Integrator::eliminated` and `on_transformation` check `check_supported` and assemble `stiffness_with`; the loads are given
   as patterns (`LoadHistory::model_loads` calls `nodal_load_vector`, which already takes the thermal load of the variants,
   ADR 0452). The mass needs the density only.
5. `RotatingSector::new` and `with_unit_prestress` find the cyclic constraint (`frame`) and build a `RotatingPart` of the
   sector. With a variant, `RotatingPart::new` would reach `solve_static` and be refused by ADR 0457 D3 inside
   `check_transformation_model` (same message), but `with_unit_prestress` makes no static solve: without a refusal of its own
   the sector would be assembled with the variant. `cyclic::modal_analysis_harmonic` and `solve_static_harmonic` check
   `check_supported` and keep refusing.
6. The non-ignored tests `sa_ortho_modal::r1_r2_paths_after_f537` (E6 `Integrator::new`, E7 `buckling::factors` refused) and
   `sa_ortho_modal::r3_paths_not_opened` (`buckling::factors`, `Integrator::new`, `RotatingPart::new` refused) assert what
   F-542 changes. They become records with copies (D6). No other non-ignored test names the variants on these paths.

## Arithmetic before this registration (disclosed)

1. **The blade reference** (`f542-scratch/py/blade.py`, numpy, log `f542-scratch/py/blade.log`). The blade of D7 (`L = 1`,
   hub radius `R_0 = 1`, tangential thickness `h_y = 0.02`, axial width `h_z = 0.05`, `E_1 = 1.4e11`, `G_12 = G_13 = 7e9`,
   `rho = 1600`, `kappa = 5/6`) as a Timoshenko beam rotating about z (D9): Ritz with `x P_k(2x/L - 1)`, `k < 20`, for the
   deflection and the section rotation, 200 Gauss points; Euler-Bernoulli likewise with `x^2 P_k`. Convergence: 12, 16, 20 and
   24 terms agree to 2.5e-11 in `lambda`; 24 against 20 terms below 5e-11 in the frequency at each speed. The speeds are fixed
   from the lag frequency at rest, `omega_lag,0 = 189.5198 rad/s`, rounded to four digits: `Omega_a = 94.76 rad/s` (`0.5
   omega_lag,0`), `Omega_b = 379.0 rad/s` (`2 omega_lag,0`).

   | speed | lag `f_ref` Hz | flap `f_ref` Hz | `S_ref` lag | `S_ref` flap | EB lag Hz | EB flap Hz |
   | --- | --- | --- | --- | --- | --- | --- |
   | 0 | 30.1630170598 | 74.6547066940 | | | 30.2213794060 | 75.5534485386 |
   | `Omega_a` | 36.1613542828 | 78.7162776575 | 1.74909602 | 2.73871527 | 36.2238878986 | 79.6030122256 |
   | `Omega_b` | 83.3775278011 | 124.1804945103 | 1.66058979 | 2.70648723 | 83.6275559939 | 125.1984475444 |

   (`S = (lambda(Omega) - lambda(0))/Omega^2`.) The Euler-Bernoulli frequencies at rest agree with the closed form
   `(beta_1 L)^2 sqrt(E I/(rho A L^4))/(2 pi)` to 3e-11. The first-order Southwell coefficient of the Euler-Bernoulli mode
   `w_1` (closed form of the cantilever mode, integrals by quadrature): `Q = int N_1 w'^2 / int rho A w^2 = Q_0 + (R_0/L)
   Q_R`, `Q_0 = 1.193336` (the value of ADR 0403 arithmetic 4, reproduced), `Q_R = 1.570878`; flap `Q = 2.764215`, lag `Q - 1
   = 1.764215` (D9).
2. **The tilt of the cylindrical axes** (same script). In the blade the material direction 1 is radial, at the angle
   `theta = atan(y/r)` from the span; the axial modulus of an off-axis orthotropic fibre lies between the free-coupling value
   `1/S'_11(theta)` and the restrained value `Q'_11(theta)`. Weighted by the bending energy of the mode at rest
   (`delta f/f = 1/2 int delta EI w''^2 / int EI w''^2`): flap between -2.1e-5 and -2.1e-4, lag between -3.8e-5 and -3.8e-4.
3. **Column buckling** (same script): the cantilever of ADR 0457 D7 (`L = 4`, `h = 0.1`, `E_1 = 1.4e11`, `G_13 = 7e9`) under
   a uniform `S_xx = -1e6` Pa: `P_E = pi^2 E_1 I/(4 L^2) = 179914.663562 N`, factor `lambda_E = P_E/(1e6 A) = 17.99146636`;
   Engesser `P = P_E/(1 + P_E/(kappa G_13 A))`, factor `lambda_En = 17.93614677` (-3.075e-3); Haringx -3.065e-3 (the two
   differ by 1e-5).
4. **B1**: `crates/rcfd-fem/tests/sa_ortho_rotating_default_bits.rs` was written with the API of main 2504fd62 only and run
   once at that commit (release); the 20 digests it printed are its constants `BASE` (log `f542-scratch/b1-base.log`).

No probe of the solver was run for this registration: the rounding estimates of the 1e-12 checks are those measured by ADR
0457 (C2 to C2tp, at most 5.9e-14 on the same block models).

## Design

Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation); none changes a default result, a golden or a hash table.

### D1 The entry checks and the per-point stiffness

On each opened path the entry check `check_supported` becomes `SolidModel::check_supported_solids_with_materials` (ADR 0457
D2: solid elements, the variants on them, `mat_ortho::check_model`, supports; beams and shells stay refused as before), then
the cyclic refusal of D3; the stiffness `stiffness_with` becomes `assemble::point_stiffness`; the public
`geometric_stiffness` becomes the crate-private `geometric::assemble_geometric` (its body after the check, made
`pub(crate)`). For a model without the two variants each new check passes exactly where the old one passed with the same
error otherwise, `point_stiffness` is `assemble_stiffness(.., None)` (what `stiffness_with` calls) and `assemble_geometric`
is what `geometric_stiffness` calls after a check the entry check already made: the same bits.

- `rotating::check_model` (the entry of `RotatingPart::new` and `with_unit_prestress`) and `with_unit_prestress`'s
  assembly. The spin-softening mass and the Coriolis matrix are unchanged (density only).
- `buckling::factors` (both paths).
- `transient::Integrator::eliminated` and `on_transformation`.
- Not changed: the public `assemble::stiffness`, `stiffness_with`, `geometric::geometric_stiffness`, `spin_softening_mass`.

### D2 The unit centrifugal model with a per-point material

For a model with a material of either variant, `unit_centrifugal_model` copies the model's temperature field (if it has
one) and gives the copy each material with its thermal expansion set to zero (`mat_ortho::without_expansion`: the expansion
of an isotropic or orthotropic material 0, a table's base likewise and its `expansion*` columns removed). The static solve of
the copy then takes the stiffness of a tabulated material at the model's temperature field at each point (ADR 0457 D4) and
no thermal strain: the pre-stress is that of the centrifugal load at 1 rad/s only, as the module states ("its own loads are
not part of the pre-stress"), and scales with `Omega^2`. The stiffness `K` of the rotating part is `point_stiffness` of the
model itself (the same field). For a model without the variants the copy is made as before (no field): the same bits.

### D3 A cyclic constraint with a variant (F-537 question 3: refused, by name)

A new crate-private `mat_ortho::refuse_with_cyclic(model)` refuses a model with a `Cyclic` constraint and a material of
either variant with the error of ADR 0457 D3: `NotYetSupported { item: "material", variant, location: "material {i},
element set '{set}', with the cyclic constraint {j}", package: FOLLOW_UP }`. It is called by `rotating::check_model`, at the
start of `RotatingSector::new` and `RotatingSector::with_unit_prestress` (before `frame`, finding 5), by `buckling::factors`
and by both constructors of the integrator. This covers the case of F-537's question 3 (cylindrical axes about the
constraint's own axis), which stays refused: opening it needs the cyclic harmonic paths (`modal_analysis_harmonic`,
`solve_static_harmonic`, the sector whirl) on the per-point stiffness and a sector-against-full-rotor check of its own, which
F-542 does not need. The question is not taken (rule 6); it is listed as remaining question 1 with a recommendation.

### D4 What comes along

The whirl analyses of a `RotatingPart` (dense, sparse, state) take the variants with the part's `K` (finding 1); one check
(W1) reads them. A `RotatingSector` stays refused (D3).

### D5 Paths not opened

Nonlinear, harmonic with the static correction, direct harmonic, the cyclic harmonic paths, damped modes, the public
stiffness and geometric stiffness, beams and shells, and any path with a cyclic constraint (D3): each refuses both variants
as before, naming the variant and `FOLLOW_UP`.

### D6 Records and copies (the standing decision)

`sa_ortho_modal::r1_r2_paths_after_f537` and `sa_ortho_modal::r3_paths_not_opened` become ignored records citing ADR 0291
and ADR 0459. Their copies `r1_r2_paths_after_f542` and `r3_paths_not_opened_after_f542` (in the new `sa_ortho_rotating.rs`,
with their helpers copied) have E6 and E7, and `buckling::factors`, `Integrator::new`, `RotatingPart::new`, solving; the rows
of R3 below are added to the copy of R3. Nothing else in an existing test changes.

### D7 Models of the checks

- Materials (ADR 0457 D7): `ISO` isotropic `E = 2e11`, `nu = 0.3`, `rho = 7850`, `alpha = 0`; `ISO_O` isotropic constants in an
  orthotropic material (`G = E/(2 (1 + nu))`) with `Fixed(I)` and `Fixed(rot((1, 2, 3), 0.5))`; `ISO_C` the same constants
  with `Cylindrical { origin: (-0.5, 0.2, 0), axis: (0, 0, 1) }`; `ORTHO` (`E = (1.4e11, 1e10, 1e10)`, `nu = (0.3, 0.3,
  0.45)`, `G = (5e9, 5e9, 3.5e9)`, `rho = 1600`, `alpha = 0`); `CYL` = `ORTHO` with `Cylindrical { origin: (0.5, -1, 0), axis:
  (0, 0, 1) }`; `TAB` = `Tabulated(ISO, [200, 600] K, e = [2e11, 1.6e11])` with the field `T = 293 + 250 x`; `TAB_U` the same
  with a uniform `T = 443`; `TAB_A` = `TAB_U` with the base's expansion `1.2e-5`; `ISO(E(443))` the isotropic material with
  `E = interpolate([200, 600], [2e11, 1.6e11], 443)`.
- **Block** `BLK` (ADR 0457 D7): Hex20 4 x 2 x 1 on `[0, 1] x [0, 0.4] x [0, 0.25]`, every component of `xmin` held; six modes,
  `ResidualTest::Backward`, tolerance 1e-12. **Transformation block** `TRB` (ADR 0457 D7): Hex8 5 x 2 x 1 on the same box,
  `u_x = u_y = 0` on `xmin` as displacement constraints, `u_z = 0` at each `xmin` node as a one-term linear constraint, a
  `NodalSpring` in z of `5e7` N/m on the `xmax` nodes. Rotation of the blocks about z through `O_B = (-0.5, 0.2, 0)`; speed
  `Omega_c = 0.5 sqrt(lambda_1)`, `lambda_1` the first eigenvalue at rest of the comparison model (`ISO`, `ISO(E(443))`, or
  the model itself for RQ1), computed in the test by this rule.
- **Blade** `BL` (RB checks): Hex20 20 x 2 x 2 (`upgrade_to_hex20` of `hex_box`) on `x in [1, 2]`, `y in [-0.01, 0.01]`, `z
  in [-0.025, 0.025]`; the support of beam theory (ADR 0457 D7, with the two middle lines of the 2 x 2 root face): `u_x = 0` on
  `xmin`, `u_y = 0` at the root nodes with `y = 0`, `u_z = 0` at the root nodes with `z = 0`; material `CYL_B` = orthotropic
  `E = (1.4e11, 1.4e10, 1.4e10)`, `nu = (0.3, 0.3, 0.4)`, `G = (7e9, 7e9, 5e9)`, `rho = 1600`, `alpha = 0`, `Cylindrical {
  origin: (0, 0, 0), axis: (0, 0, 1) }` (1 radial, 2 tangential, 3 axial); rotation about z through the origin; four modes,
  Backward 1e-12. The lag mode is the lowest of the four with a share of the kinetic energy in y above 0.5, the flap mode the
  lowest with a share in z above 0.5 (`share` of `si_rotating.rs`). Speeds 0, `Omega_a = 94.76`, `Omega_b = 379.0` rad/s.
  `BL_T`: `BL` turned by `phi = 0.7` rad about z (each node turned), the support in the turned directions as linear
  constraints (`c u_x + s u_y = 0` at each root node, `-s u_x + c u_y = 0` at the root nodes with `y' = 0`) and `u_z = 0` as
  displacement constraints: the transformation path.
- **Column** `COL` (BK1): the cantilever of ADR 0457 D7 (Hex20 20 x 1 x 1, `L = 4`, `h = 0.1`, the symmetric support of beam
  theory), the orthotropic constants of `CYL_B` with `Fixed(I)`; uniform `S_xx = -1e6` Pa; two factors.
- **Sector** `SEC` (R3): the disc sector D of ADR 0395 (`si_rotating_cyclic.rs`: 12 sectors, Hex20 3 x 2 x 1, `xmin`
  clamped, the cyclic constraint about z through the origin) with `ORTHO` and `Cylindrical { origin: (0, 0, 0), axis: (0, 0,
  1) }`: cylindrical axes about the constraint's own axis.
- Pre-stress of the buckling checks on `BLK` and `TRB`: uniform `S_xx = -2e8` Pa; three factors, `BucklingOptions::new(3)`.
- Transient set-ups: TR1 below; TR2, TR3: a nodal force `(0, 0, -1e4)` N on every `xmax` node (`BLK`) or `tip` node (`TRB`),
  `LoadHistory::model_loads` with `LoadTable::constant(1, 0, 1)`, average acceleration, `Rayleigh::from_ratio(0.02, 500,
  5000)`, `dt = 2e-4`, 25 steps from rest; the displacement and the velocity at the end.

### D8 Identities

- **RQ1** (the rotating stiffness against the pre-stressed modes of ADR 0457). With `spin_softening(false)`, `RotatingPart::modes
  (Omega)` solves `(K + Omega^2 K_sigma(P_1)) phi = omega^2 M phi`, and `prestressed_modes(model, P_1 scaled by Omega^2)`
  solves `(K + K_sigma(Omega^2 P_1)) phi = omega^2 M phi` with the per-point `K` verified by ADR 0457 (Q1, against the static
  solve). `K_sigma` is linear in the stress, so the two problems differ by rounding only; both are given `Shift::Value(0)`.
  A rotating stiffness taken otherwise (element-constant, at another temperature, without the supports) changes one and not
  the other.
- **BK4** (buckling against the static solve). For a pair `K x = -lambda K_sigma x` on the free unknowns (or on `q` of the
  transformation), the static solve of the same model under the nodal forces `f = K_sigma x` (`K_sigma` the public geometric
  stiffness of an isotropic twin on the same mesh: it takes no material) gives `u = -x/lambda`, and `lambda' = -x^T K_sigma
  x / x^T K_sigma u = lambda` up to the square of the error of `x` and rounding. The static solve is the path of ADR 0452
  (elimination) and ADR 0457 (transformation, with the supports).
- **TR1** (the integrator against the modal analysis). Average acceleration (Newmark `beta = 1/4`, `gamma = 1/2`) is the
  trapezoidal rule on `(u, v)`; for an undamped mode `omega` its one-step map has the eigenvalues `exp(+- i theta)`, `tan(theta/2)
  = omega dt/2`, with the eigenvectors `(1, +- i omega)`. From `u_0 = phi`, `v_0 = 0` (and `a_0 = -omega^2 phi` from the
  equilibrium the integrator solves), `u_n = cos(n theta) phi` exactly. With `phi` the first mode of the modal analysis
  (ADR 0457) and `q_n = phi^T M u_n / phi^T M phi`, a transient stiffness different by a relative `delta` from the modal one
  moves `q_n` by about `n theta delta/2`.

### D9 The blade reference (clause a2)

A Timoshenko beam along the radial span `x` (from the hub at `r = R_0 + x`), rotating at `Omega` about z, in the rotating frame
without the Coriolis term (as `RotatingPart::modes`), clamped at `x = 0` (`w = psi = 0`). The 3D strain energy restricted to
the kinematics `u_z = w(x)`, `u_x = -z psi(x)` (flap; the lag mode with `y`, `v`, `I_z`) and the operator of ADR 0403 give

`U = 1/2 int [E_1 I psi'^2 + kappa G A (w' - psi)^2 + N (w'^2 + (I/A) psi'^2)] dx - 1/2 Omega^2 int rho I psi^2 dx`
(flap), the lag mode with also `- 1/2 Omega^2 int rho A v^2 dx`, and `T = 1/2 omega^2 int (rho A w^2 + rho I psi^2) dx`:

the geometric stiffness of the axial tension on both displacement gradients, `N(x) = rho A Omega^2 (R_0 (L - x) + (L^2 -
x^2)/2)` (statically determinate, independent of the moduli), the spin softening of the in-plane motions (`M_perp` of a
rotation about z: the x and y components; the flap deflection `w` is along the axis and not softened). `kappa = 5/6` (the
rectangular section), `G = G_13` for the flap and `G_12` for the lag mode. Solved by Ritz (arithmetic 1). The first-order
Southwell coefficient of the Euler-Bernoulli mode, `Q` flap and `Q - 1` lag (arithmetic 1), is the closed form; the Ritz
values differ from it by Southwell's dependence on the speed and by the shear (-9.2e-3 flap, -8.6e-3 lag at `Omega_a`).

Discretisation and modelling error of the 3D blade against the reference (b1), estimated before any run:
- shear across the section: the Timoshenko shear term is -1.19 percent (flap, `L/h_z = 20`) and -0.19 percent (lag, `L/h_y =
  50`) of the frequency at rest (arithmetic 1: the Euler-Bernoulli column); the 3D section against `kappa = 5/6` is uncertain
  by about a tenth of it (ADR 0457 C1c measured +1.28e-3 with one Hex20 across; two here): flap +- 1.2e-3, lag +- 2e-4;
- the tilt of the cylindrical axes (arithmetic 2): flap -2.1e-5 to -2.1e-4, lag -3.8e-5 to -3.8e-4;
- the mesh along the span (20 Hex20, ADR 0385: 3.9e-4 on a deflection): +1e-4;
- the root and the 3D pre-stress (lateral stresses at the support, the Poisson coupling): +- 3e-4.

So about 1.8e-3 (flap) and 9e-4 (lag) on a frequency. On `S` the errors of `lambda(Omega)` and `lambda(0)` do not cancel fully
(the shear share falls as the geometric stiffness grows; ADR 0403 measured about 9e-4 on `S` of a solid beam): about 2e-3.

## Checks

Limits relative to the reference value unless stated. Frequencies in Hz from `ModalResult::frequencies`; `S_FE = (lambda(Omega)
- lambda(0))/Omega^2` with `lambda = (2 pi f)^2`.

| Check | Clause | Quantity | Limit | Estimate |
| --- | --- | --- | --- | --- |
| RB1 | a2, a1, b1 | `BL`: lag and flap `f` at 0, `Omega_a`, `Omega_b` against `f_ref` (arithmetic 1), each `abs(f/f_ref - 1)` | `<= 5e-3` | 1.8e-3 flap, 9e-4 lag (D9) |
| RB2 | a2, a1, b1 | `BL`: lag and flap `S_FE` at `Omega_a` and `Omega_b` against `S_ref`, each `abs(S/S_ref - 1)` | `<= 1e-2` | 2e-3 (D9) |
| RB3 | a2 | `BL`: `S_FE(Omega_a)` against the closed form, flap `Q = 2.764215`, lag `Q - 1 = 1.764215` | `<= 2e-2` | 1.1e-2 (-9e-3 of the reference, D9, and RB2's 2e-3) |
| RB4 | a2, a1 | `BL_T` (turned 0.7 rad, transformation path) against `BL`: the four frequencies at 0, `Omega_a`, `Omega_b`, max `abs(f_T/f - 1)` | `<= 1e-9` | 1e-12 (the same problem turned; rounding of the turned coordinates) |
| RC2 | a3 | `BLK`, rotating about z through `O_B` at `Omega_c` (from `ISO`): `ISO_O` with `Fixed(I)`, `Fixed(rot)`, and `ISO_C` against `ISO`, max over six modes `abs(f/f_ISO - 1)` | `<= 1e-12` | 1e-13 (ADR 0457 C2 to C2tp: at most 5.9e-14) |
| RC2t | a3 | `TRB`, the same | `<= 1e-12` | 1e-13 |
| RQ1 | a1 | `BLK` and `TRB` with `TAB` and with `CYL`: the identity of D8 at `Omega_c` of the model, max relative eigenvalue difference over six modes | `<= 1e-10` | 1e-14 |
| RP1 | a1, b2 | `BLK` with `TAB_U` and with `TAB_A` against `ISO(E(443))` (no expansion): frequencies at `Omega_c` (from `ISO(E(443))`), and the unit pre-stress, max abs difference over max abs | `<= 1e-12` each | 1e-15 (bits expected: the same operations at every point) |
| RP1t | a1, b2 | `TRB` with `TAB_U` against `ISO(E(443))`: frequencies at `Omega_c` | `<= 1e-12` | 1e-15 |
| W1 | a1 (D4) | `BLK`, `coriolis(true)`: `whirl_modes(Omega_c)`, the three lowest `omega`, `ISO_O` (`Fixed(rot)`) against `ISO` | `<= 1e-9` | 1e-11 (the dense solver's absolute accuracy, about `eps lambda_max / lambda_1`; not a clause-a3 check) |
| BK1 | a1, b1 | `COL`: factors 1 and 2 against `lambda_E = 17.99146636`, each `abs(lambda/lambda_E - 1)` | `<= 1e-2` | 2.4e-3 (Engesser -3.08e-3; the one-element depth stiffer in shear, about +4e-4 to +7e-4) |
| BK1b | a1, b1 | the same against Engesser `lambda_En = 17.93614677` | `<= 3e-3` | 7e-4 (twice ADR 0457 C1b's 3.5e-4 on a frequency) |
| BK2 | a3 | `BLK` under `S_xx = -2e8`: three factors, `ISO_O` (`Fixed(I)`, `Fixed(rot)`) against `ISO`, max relative | `<= 1e-12` | 1e-13 |
| BK2t | a3 | `TRB`, the same | `<= 1e-12` | 1e-13 |
| BK3 | a1, b2 | `BLK` and `TRB` with `TAB_U` against `ISO(E(443))`: three factors | `<= 1e-12` | 1e-15 |
| BK4 | a1 | `BLK` and `TRB` with `TAB` and with `CYL`: the identity of D8 for factors 1 and 2, max `abs(lambda'/lambda - 1)` | `<= 1e-9` | 1e-12 |
| TR1 | a1 | `BLK` and `TRB` with `TAB` and with `CYL`: D8 with the first mode, `dt = 2 pi/(20 omega_1)`, no damping, no load, 40 steps, max over n of `abs(q_n - cos(n theta))` | `<= 1e-8` | 1e-11 (the eigenvalue's error 1e-12 times `n theta`; rounding) |
| TR2 | a3 | `BLK` and `TRB`: `ISO_O` (`Fixed(I)`, `Fixed(rot)`) against `ISO` (D7 transient set-up): displacement and velocity at the end, max abs difference over max abs | `<= 1e-12` | 1e-14 |
| TR3 | a1, b2 | `BLK` and `TRB`: `TAB_U` against `ISO(E(443))`, the same | `<= 1e-12` | 1e-15 |
| R1 | D6 | `r1_r2_paths_after_f542` (both variants on the ADR 0452 cantilever): E1, E2, E3, E6, E7, E8, E9, E10 solve; E4, E5 refused `NotYetSupported { item: "material", variant, location: "material 0, element set 'all'", package: FOLLOW_UP }`; R2 (Shell4 static) refused as in ADR 0452 | as stated | as stated |
| R3 | a4, a5 | `r3_paths_not_opened_after_f542`, both variants on the ADR 0452 cantilever: `assemble::stiffness`, `geometric_stiffness`, `nonlinear::solve`, `damped_modes`, `DirectHarmonic::new`, `cyclic::modal_analysis_harmonic` refused naming the variant, `"material 0, element set 'all'"` and `FOLLOW_UP`; `buckling::factors`, `Integrator::new`, `RotatingPart::new` solve; Shell4 modal refused as in ADR 0457; `TRB` with `ORTHO` (`Fixed(I)`) and a cyclic constraint: `modal_analysis_with`, `buckling::factors`, `Integrator::new`, `RotatingPart::new`, `RotatingPart::with_unit_prestress` refused naming the variant, `"material 0, element set 'all', with the cyclic constraint {j}"` and `FOLLOW_UP`; `SEC` (cylindrical axes about the cyclic axis): `RotatingSector::new` and `RotatingSector::with_unit_prestress` refused the same way, `cyclic::modal_analysis_harmonic` refused naming the variant; `BLK` with `TAB` without a temperature field: `modal_analysis`, `RotatingPart::new`, `buckling::factors`, `Integrator::new` refused, key `temperature` | as stated | as stated |
| B1 | a6 | `sa_ortho_rotating_default_bits.rs` (20 digests of main 2504fd62: rotating modes on both paths with and without stress stiffening, a model with a temperature field and an expansion, the unit pre-stress and its geometric stiffness, the dense whirl modes, a cyclic sector at index 1, buckling on both paths, the transient integrator on both paths) against `BASE`; and the existing `sa_ortho_modal_default_bits`, `sa_ortho_default_bits`, `sa_ortho_modal` (with D6), `sa_ortho`, `sa_scaffold`, `sa_scaffold_bits` and the `si_rotating*`, `si_buckling*`, `si_transient*` tests pass | `==` | `==` |

### Negative controls

| Control | Quantity | Expected |
| --- | --- | --- |
| NB1 | `BL` with `Fixed(rot(z, pi/2))` axes (`E_2 = E_1/10` along the span): lag `f/f_ref` at rest | `< 0.5` (about `sqrt(0.1)`: RB1 sees `E_1`) |
| NB2 | `BL` with `spin_softening(false)`: lag `abs(S/S_ref - 1)` at `Omega_b` | `> 0.3` (about `1/1.66 = 0.6`) |
| NB3 | `BL` with `stress_stiffening(false)`: flap `abs(S/S_ref - 1)` at `Omega_b` | `> 0.5` (about 1) |
| NB4 | `BL_T` with `Fixed(I)` axes (the fibres 0.7 rad off the span) against `BL`: max relative difference of the four frequencies at rest | `> 1e-2` (the off-axis modulus is about 0.15 `E_1`) |
| NB5 | `COL` with the axes turned by 90 degrees about z: factor 1 over `lambda_E` | `< 0.5` (about 0.1) |
| NR1 | `BLK` with `TAB_A`: the stress of a static solve with the centrifugal load at 1 rad/s and the temperature field (thermal strain included) against the unit pre-stress of RP1, max abs difference over max abs of the unit pre-stress | `> 1` (the thermal stress, about `E alpha dT = 3e8` Pa near the root, against the centrifugal stress of about 1e4 Pa) |

### Planned mutations (scratch copy `w49i-work/f542-scratch/mut`, the test `sa_ortho_rotating`)

| Mutation | Expected to catch |
| --- | --- |
| Mu1 | the unit centrifugal model copies the temperature field but keeps the expansions | RP1 (`TAB_A`) |
| Mu2 | the unit centrifugal model as before (no field) | RP1, RQ1 (`TAB` refused), R1/R3 rows of `RotatingPart::new` |
| Mu3 | `RotatingPart` with a per-point material: `K_sigma,1` left out | RB1, RB2, RC2, RQ1 |
| Mu4 | `RotatingPart` with a per-point material: `M_perp` left out | RB2 (lag), RC2 |
| Mu5 | buckling on the transformation without `add_supports` | BK4 (`TRB`) |
| Mu6 | the transient integrator on the transformation without `add_supports` | TR1 (`TRB`) |
| Mu7 | `refuse_with_cyclic` returns `Ok` | R3 (the `with_unit_prestress` rows) |

## Decision rule

- **done**: every check RB1 to RB4, RC2, RC2t, RQ1, RP1, RP1t, W1, BK1, BK1b, BK2, BK2t, BK3, BK4, TR1, TR2, TR3, R1, R3,
  B1 met in its single registered run, NB1 to NB5 and NR1 as expected, and every mutation caught by at least one registered
  check.
- **partial**: any registered check missed (kept as a `*_known_miss` record with its limit), a mutation not caught, or a check
  met only after an amendment made after its run.
- **todo**: the rotating modes not opened.

## Limits (what is not covered)

- A cyclic constraint with either variant stays refused on every path, also with cylindrical axes about the constraint's own
  axis (D3); the cyclic harmonic paths, the rotating sector and its whirl analyses are not opened.
- Nonlinear, harmonic with the static correction, direct harmonic, damped modes, beams and shells refuse the variants.
- The blade reference is a Timoshenko beam with `kappa = 5/6`; the 3D section's shear and the tilt of the cylindrical axes are
  estimated, not computed (D9). A disc is not checked.
- The temperature of a tabulated material is the model's static field on every path; a field that changes in time (in the
  transient integrator) or with the speed is not provided; the pre-stress of a rotating part has no thermal stress (D2).

## Amendment 1 (2026-11-14, after the registered run of `sa_ortho_rotating`, before any corrected run)

The registered run (commit c911ca5c, log `f542-scratch/run-sa_ortho_rotating.log`) stopped the blade test
`rb1_to_rb4_nb1_to_nb4_rotating_blade` at NB3: with `stress_stiffening(false)` the part at `Omega_b = 379.0 rad/s` reported a
loss of stiffness (`negative: Some(1)`). The set-up was defective: `Omega_b` is twice `omega_lag,0`, and without the
centrifugal stiffening the spin softening `-Omega^2 M_perp` alone drives the lag eigenvalue, about `lambda_0 - Omega^2`,
below zero, so `K_eff` is not positive definite there; the registration overlooked that NB3 is evaluated on the whole part,
whose lowest mode is the lag mode. RB1 to RB4, NB1 and NB2 had run before it (their values are those of that run, addendum
1); NB4 was not reached.

Correction (no limit changed): **NB3b** = NB3 at `Omega_a = 94.76 rad/s` (half of `omega_lag,0`: without stiffening `K_eff`
stays positive definite, the lag eigenvalue about `0.75 lambda_0`): flap `abs(S/S_ref(Omega_a) - 1)` with `S_ref(Omega_a) =
2.73871527`, expected `> 0.5` (about 1: without stiffening the flap `S` is about `-f_x`, of order -1e-3). NB4 as registered.
The test of the registered run becomes the ignored record `rb1_to_rb4_nb1_to_nb4_rotating_blade` (its measured values in its
ignore text); the corrected test `rb1_to_rb4_nb1_nb2_nb3b_nb4_amended` repeats RB1 to RB4, NB1 and NB2 unchanged (a
deterministic computation: the registered values are those of the first run; a difference would be reported) and runs NB3b
and NB4 once.

Not an amendment: TR2 missed its limit on `BLK` (1.65e-12 to 9.25e-12 against 1e-12; `TRB` met). It is kept as the ignored
record `tr1_tr2_tr3_transient_known_miss` with the limit intact; a copy `tr1_tr2t_tr3_after_known_miss` keeps TR1, the `TRB`
rows of TR2 and TR3 (all met in the registered run, same limits) in the suite. Diagnosis (a probe after the run, a temporary
test file deleted afterwards, log `f542-scratch/probe-tr2.log`): the same `BLK` transient set-up with the isotropic material
against the isotropic material with `nu` one ulp above moves the end displacement by 5.1e-12 and the velocity by 5.6e-12, and
with `E` one ulp above by 1.5e-12 and 9.1e-12; on `TRB` by 6.6e-15 and 2.6e-13. The 1e-12 limit of TR2 on `BLK` lies below
the rounding sensitivity of the set-up itself (the step load excites the high Hex20 modes, and 25 steps of solves with `c_M M
+ c_K K` carry a change of `D` in the last bit to about 1e-11 of the response); the per-point path performs no operation the
isotropic one does not (TR3 bit-equal). The registration took its estimate (1e-14) from the eigenvalue checks of ADR 0457,
without a transient probe.

## Addendum 1: results (2026-11-14)

Registered runs at commit c911ca5c (library code and tests committed before any run), release profile, each file run once:
`sa_ortho_rotating_default_bits` (log `f542-scratch/run-b1.log`), `sa_ortho_rotating` (log
`f542-scratch/run-sa_ortho_rotating.log`); after amendment 1 (commit be16f7b6) the corrected test and the copy of the met
rows of the known miss, once (log `f542-scratch/run-amended.log`). Then the existing tests that cover the changed lines (the
`rcfd-fem` lib tests and 32 test files: `sa_damped`, `sa_ortho*`, `sa_scaffold*`, `sa_sparse*`, `sa_mat_table`,
`si_buckling*`, `si_geometric`, `si_harmonic*`, `si_knife_edge_closed_form`, `si_nonlinear`, `si_prestress_mpc`,
`si_rotating*`, `si_transient*`): 33 result blocks, 172 passed, 0 failed, 35 ignored (log `f542-scratch/run-fem-covering.log`).

| Check | Limit | Measured | Met |
| --- | --- | --- | --- |
| RB1 blade lag at 0, `Omega_a`, `Omega_b` against the Timoshenko reference | `<= 5e-3` | 7.62e-4, 6.09e-4, 3.25e-4 (`f` = 30.1400, 36.1393, 83.3504 Hz) | yes |
| RB1 blade flap, the same | `<= 5e-3` | 2.75e-4, 2.83e-4, 4.57e-4 (`f` = 74.6342, 78.6940, 124.1237 Hz) | yes |
| RB2 `S` lag at `Omega_a`, `Omega_b`; flap | `<= 1e-2` | 5.20e-4, 5.19e-4; 7.02e-4, 1.12e-3 | yes |
| RB3 `S(Omega_a)` against the first-order Southwell closed form, lag; flap | `<= 2e-2` | 9.09e-3; 9.92e-3 | yes |
| RB4 the turned blade (transformation) against the blade | `<= 1e-9` | 9.84e-11 | yes |
| RC2 rotating block, `Fixed(I)`; `Fixed(rot)`; cylindrical | `<= 1e-12` | 4.5e-14; 6.4e-14; 5.1e-14 | yes |
| RC2t the same on the transformation | `<= 1e-12` | 5.7e-15; 1.1e-14; 9.3e-15 | yes |
| RQ1 rotating against the pre-stressed modes, `TAB` BLK, TRB; `CYL` BLK, TRB | `<= 1e-10` | 1.7e-14, 7.8e-16; 1.1e-14, 2.7e-15 | yes |
| RP1 `TAB_U`, `TAB_A` against `ISO(E(443))`: frequencies; unit pre-stress | `<= 1e-12` | 0, 0; 0, 0 | yes |
| RP1t `TAB_U` transformation | `<= 1e-12` | 0 | yes |
| W1 dense whirl, `Fixed(rot)` against `ISO` | `<= 1e-9` | 1.97e-13 | yes |
| BK1 column factors 1, 2 against Euler with `E_1` | `<= 1e-2` | 2.671e-3 both (`lambda` = 17.9434138) | yes |
| BK1b the same against Engesser | `<= 3e-3` | 4.05e-4 | yes |
| BK2 buckling block, `Fixed(I)`; `Fixed(rot)` | `<= 1e-12` | 6.3e-14; 6.8e-14 | yes |
| BK2t the same on the transformation | `<= 1e-12` | 3.6e-15; 1.6e-14 | yes |
| BK3 `TAB_U` against `ISO(E(443))`, BLK; TRB | `<= 1e-12` | 0; 0 | yes |
| BK4 buckling against the static solve, `TAB` BLK, TRB; `CYL` BLK, TRB | `<= 1e-9` | 2.2e-16, 2.2e-16; 1.3e-15, 6.7e-16 | yes |
| TR1 transient against the modal analysis, `TAB` BLK, TRB; `CYL` BLK, TRB | `<= 1e-8` | 9.1e-13, 9.5e-15; 3.0e-13, 1.9e-13 | yes |
| TR2 BLK, `Fixed(I)`: displacement; velocity | `<= 1e-12` | 1.65e-12; 9.25e-12 | **no** (known miss) |
| TR2 BLK, `Fixed(rot)`: displacement; velocity | `<= 1e-12` | 3.85e-12; 6.24e-12 | **no** (known miss) |
| TR2 TRB, `Fixed(I)`; `Fixed(rot)` (displacement, velocity) | `<= 1e-12` | 1.1e-14, 3.4e-13; 1.5e-14, 2.3e-13 | yes |
| TR3 `TAB_U` against `ISO(E(443))`, BLK and TRB | `<= 1e-12` | 0 (all four) | yes |
| R1 (copy) E1, E2, E3, E6 to E10 solve; E4, E5 refused; R2 | as stated | 21 of 21 | yes |
| R3 (copy with its rows) | as stated | 31 of 31 | yes |
| B1 20 digests of main 2504fd62 | `==` | 20 of 20 | yes |
| NB1 lag `f/f_ref` with `E_2` along the span | `< 0.5` | 0.318 | as expected |
| NB2 lag `S` without spin softening | `> 0.3` | 0.602 | as expected |
| NB3 flap `S` without stress stiffening at `Omega_b` | `> 0.5` | set-up defective (loss of stiffness), amendment 1 | not run |
| NB3b (amendment 1) the same at `Omega_a` | `> 0.5` | 1.0003 (`S = -9.2e-4`) | as expected |
| NB4 turned blade with `Fixed(I)` (run after amendment 1) | `> 1e-2` | 0.612 | as expected |
| NB5 column with the axes turned 90 degrees | `< 0.5` | 0.0999 | as expected |
| NR1 the thermal stress included against the unit pre-stress | `> 1` | 4.49e4 | as expected |

Reading. The orthotropic blade with cylindrical axes follows the rotating Timoshenko beam within 7.6e-4 at rest and at both
speeds, below the registered estimate (1.8e-3 flap, 9e-4 lag): two Hex20 across the section carry the shear of `kappa = 5/6`
closely. The stiffening coefficients agree within 1.1e-3, so the centrifugal pre-stress of the orthotropic part, its geometric
stiffness and its spin softening are those of the beam; the first-order Southwell closed form is 0.9 to 1.0 percent above
them, as D9 predicted (Southwell's dependence on the speed and the shear). Recorded without a limit: at this slenderness the
Euler-Bernoulli flap frequency with `E_1` is 1.2 percent high. Turning the whole blade by 0.7 rad changes nothing beyond 1e-10
(RB4), while the same turn with fixed axes changes the frequencies by 61 percent (NB4): the per-point cylindrical axes act on
the rotating path, the pre-stress solve included. The isotropic equivalence of the rotating and buckling paths and of the
transient on the transformation block sits at rounding (at most 6.8e-14); the tabulated material at a uniform temperature
gives the bits of the isotropic material at `E(443)` on every path, also with an expansion (RP1 `TAB_A`): the pre-stress at 1
rad/s carries no thermal strain (NR1: with it the stress would differ by a factor 4.5e4). The orthotropic column buckles 4.05e-4
above Engesser's value with `G_13`.

TR2 on `BLK` missed: the step response of the Hex20 block after 25 steps differs by up to 9.25e-12 between the isotropic
material and the isotropic constants in an orthotropic material. Diagnosis (amendment 1): a one-ulp change of `nu` or `E` of
the isotropic material itself moves the same set-up by 5.1e-12 to 9.1e-12; the 1e-12 limit lies below the rounding sensitivity
of that set-up, not of the per-point path (TR3, the tabulated material at a uniform temperature against the isotropic one, is
bit-equal; TR1 matches the closed form of the trapezoidal rule to 9e-13). The limit stays; the test is the ignored record
`tr1_tr2_tr3_transient_known_miss`.

### Mutations (scratch copy `f542-scratch/mut` of commit be16f7b6, `sa_ortho_rotating` per mutation, logs `f542-scratch/mut_Mu*.log`)

| Mutation | Caught by |
| --- | --- |
| Mu1 the unit model keeps the expansions | RP1 (`TAB_A`: the thermal pre-stress makes `K_eff` lose stiffness at `Omega_c`) |
| Mu2 the unit model without the temperature field | RP1/RQ1 (`TAB` refused, key `temperature`), R3 (`RotatingPart::new` with `Tabulated`) |
| Mu3 `K_sigma,1` left out for a per-point material | RB (loss of stiffness at `Omega_b`), RC2 and RC2t (0.18, 0.19), W1 (0.18), RQ1 (0.33) |
| Mu4 `M_perp` left out for a per-point material | RB1 to RB3 lag (8.3e-2 to 0.60), RC2 and RC2t (5.0e-2, 5.4e-2) |
| Mu5 buckling on the transformation without the supports | BK4 TRB (0.73, 9.8) |
| Mu6 the integrator on the transformation without the supports | TR1 TRB (1.99, 1.8e-5) |
| Mu7 `refuse_with_cyclic` returns `Ok` | R3 (buckling, integrator and `RotatingPart::with_unit_prestress` reach the cyclic geometry; `RotatingSector::with_unit_prestress` solves) |

Seven of seven caught. ADR 0457's equivalent mutant Mu7 (the entry check of `buckling::factors`) no longer exists: the path is
opened, and its entry check is now the only check of the variants there (R3 rows of `buckling::factors`).

### Deviations and what was not run

- Amendment 1: the NB3 set-up was defective (loss of stiffness); NB3b replaced it and NB4 ran after the amendment. RB1 to RB4,
  NB1 and NB2 repeated in the corrected test gave the same values as the registered run.
- TR2 missed on `BLK` (known miss, above). A probe after the run (disclosed in amendment 1) diagnosed it.
- After the registered runs rustfmt changed line breaks in `geometric.rs` and the two new test files (commit 77c38687); the
  tests were not run again for it except inside the covering run. `cargo fmt -p rcfd-fem --check` and `cargo clippy -p
  rcfd-fem --all-targets -- -D warnings` pass.
- Not run: the whole workspace (the lead's gate); `baseline_hashes` of `rcfd-api` (no `rcfd-fv` or `rcfd-api` source
  changed; the default paths of `rcfd-fem` are covered by B1 and the ADR 0457 and ADR 0452 bit records, which pass); the
  `docs_site` test (no documentation page changed).

### Decision by the registered rule

**Partial**: TR2 missed its limit on `BLK` (a registered check missed, kept as `*_known_miss` with the limit intact), and the
negative control NB3 was evaluated only after an amendment. Every other check was met in its single registered run, and every
mutation was caught. Clause (a3) is met on the rotating and the buckling paths and on the transient transformation block, and
missed on the transient Hex20 block, where the limit is below the set-up's own rounding sensitivity.

### Remaining questions

1. F-537's question 3, a cyclic constraint with cylindrical axes about its own axis, stays refused by name (D3). Options: (a) a
   follow-up opens the cyclic harmonic paths, the rotating sector and its whirl analyses when every material with its own axes
   has cylindrical axes about the constraint's axis and origin, checked sector against full rotor; (b) keep the refusal.
   Recommendation (a): fan blades are analysed on a sector.
2. TR2, the known miss. Options: (a) keep F-542 partial and register the transient isotropic equivalence again in a follow-up
   with a limit taken from a rounding probe of the set-up made before the run; (b) the maintainer accepts the miss as the
   set-up's rounding and sets F-542 done; (c) leave it. Recommendation (a): a small, honest closure of clause (a3).
3. The paths still refused (nonlinear, harmonic with the static correction, direct harmonic, damped modes, beams and shells).
   Options: (a) follow-ups through `point_stiffness`; (b) leave them. Recommendation (a), damped modes and the harmonic static
   correction first (the same assembly, the harmonic response of a fan blade).

## For the lead: roadmap entries

Proposed status of F-542: **partial** (TR2 known miss on the Hex20 block; NB3 amended).

> ADR 0459, 2026-11-14: the orthotropic and tabulated materials of ADR 0452 on the rotating modes (stress stiffening, spin softening; whirl analyses of a part), the buckling factors and the transient integrator (both paths), through the per-point stiffness; the pre-stress at 1 rad/s of a rotating part takes a table's stiffness at the model's temperature field and no thermal strain; a cyclic constraint with either material (also cylindrical axes about its own axis, F-537 question 3) is refused by name on every path, as are nonlinear, harmonic with the static correction, direct harmonic, damped modes, beams and shells. Orthotropic Hex20 blade with cylindrical axes (E2 = E1/10, G = E1/20, hub ratio 1, Omega up to twice the lag frequency): lag and flap within 7.6e-4 of a rotating Timoshenko beam, stiffening coefficients within 1.1e-3, first-order Southwell within 1.0e-2; the blade turned by 0.7 rad unchanged to 1e-10. Orthotropic column 4.05e-4 above Engesser. Isotropic constants in an orthotropic material: rotating and buckling within 6.8e-14 (both paths), transient on the transformation within 3.4e-13; missed on the transient Hex20 block (9.25e-12 against 1e-12, a one-ulp change of the isotropic constants moves that set-up by 9e-12; known miss). A tabulated material at a uniform temperature gives the bits of the isotropic material on every path. Default paths bit-identical (20 digests of main 2504fd62). Seven of seven mutations caught; ADR 0457's equivalent mutant is gone. Files rotating.rs, buckling.rs, transient.rs, geometric.rs, mat_ortho.rs; tests sa_ortho_rotating.rs, sa_ortho_rotating_default_bits.rs; sa_ortho_modal r1_r2_paths_after_f537 and r3_paths_not_opened recorded and copied.

Note of F-537 (sentence to append):

> Follow-up F-542 (ADR 0459, 2026-11-14) opened the rotating modes, the buckling factors and the transient integrator to both variants; question 3 (cyclic with cylindrical axes) stays refused by name; the equivalent mutant Mu7 no longer exists.

Follow-up item (id by the lead), title "Cyclic symmetry with cylindrical material axes", deps F-542:

> Every limit not stated here is registered in the ADR before the run. The cyclic harmonic modal analysis, the static harmonic solve and the rotating sector (modes and whirl) take the orthotropic and tabulated materials of ADR 0452 when every material with its own axes has cylindrical axes about the cyclic constraint's axis and origin; other axes stay refused by name; the harmonics of a sector match the full rotor of the same material within registered limits; isotropic constants in an orthotropic material give the isotropic results within 1e-12 on stocky models; default paths bit-identical.

Follow-up item (id by the lead), title "Transient isotropic equivalence with a probed limit", deps F-542:

> Every limit not stated here is registered in the ADR before the run, after a rounding probe of each set-up (a one-ulp change of the isotropic constants) made and disclosed before the registration. The transient integrator with isotropic constants in an orthotropic material matches the isotropic material on a Hex20 and a Hex8 model on both paths within limits derived from the probe; default paths bit-identical.
