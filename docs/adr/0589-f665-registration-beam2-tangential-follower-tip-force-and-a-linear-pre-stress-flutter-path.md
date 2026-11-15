# ADR 0589: F-665 registration: Beam2 tangential follower tip force and a linear pre-stress flutter path

Status: **REGISTERED 2026-11-15, written before any code of F-665 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-665 was written in the repository. No rcfd code was run for this registration: the
reference numbers below come from one plain Python script (numpy only) of the reference alone,
`w49i-work/f665-scratch/beam2_beck_reference.py`, log `beam2_beck_reference.log`, disclosed (arithmetic 1 to 3). The bit
record N1 (D5) is a test file written and run on the unchanged base 06b66e35 after this commit and before any library line.
Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-665 (dep F-653): the follow-up proposed by
[ADR 0580](0580-f653-registration-flutter-of-follower-loads-the-unsymmetric-eigenproblem.md) (its open question 2 and
remaining question Q2). Number 0589 given by the lead. Branch `f665-work` from 06b66e35 (which holds F-653's
`eigen_pencil.rs`, `follower::tangent_modes_full`, `buckling_factors_full`, and F-664's `backward_errors`, ADR 0587), worktree
`w49i-work/f665`, scratch `w49i-work/f665-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice the maintainer would normally be asked
about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and listed at the end.

## What F-665 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. A follower tip force on a Beam2 node with its load stiffness, and the modes of K + lambda (K_sigma - K_f) with the mass as complex eigenvalues; Beck's column of Beam2 elements has its flutter load within a registered limit of the derived 20.0509536 E I/L^2 of docs/reference-data.md; existing beam results keep their bits.

Clauses: (a0) every limit registered here; (a1) a follower tip force on a Beam2 node with its load stiffness `K_f`; (a2) the
modes of `(K + lambda (K_sigma - K_f)) phi = omega^2 M phi` as complex eigenvalues; (a3) Beck's column of Beam2 elements has
its flutter load within a registered limit of `20.0509536 E I/L^2` (`docs/reference-data.md`, the entry of ADR 0580); (a4)
existing beam results keep their bits.

Readings, registered:

- **"Linear pre-stress"** (the brief): `K_sigma` is the geometric stiffness of the pre-stress of the linear static solve with
  the dead reference load (`Prestress::from_static`), no Newton iteration, so the beam needs no Newton tangent (the driver
  refuses beams, `check_supported_nonlinear`; ADR 0580 open question 2). `lambda` scales both `K_sigma` and `K_f`, which are
  those of the reference load (load factor 1).
- **(a1), "rotating with the end slope".** The Beam2 node carries the section rotation `theta` (small rotations, ADR 0417); the
  force is taken as fixed in the end section, so it turns with the node rotation: `f = lambda (F + theta x F)` (D1). For the
  shear-flexible beam the node rotation is the section rotation `psi`, not the axis slope `w'`; at the free end of the
  Timoshenko continuum both choices give the same tip condition `w' = psi` (arithmetic 2), and the element is checked against
  that continuum (T3). Open question 2.
- **(a3), the limit.** The Beam2 element is the exact prismatic Timoshenko beam (ADR 0417), so a column of finite slenderness
  departs from the Euler-Bernoulli `20.0509536` by its shear and rotary inertia (ADR 0580 arithmetic 3: `-7.0e-4` at `L/h = 100`,
  `nu = 0`). The limit against `20.0509536` is set from that departure (T1); two tighter checks pin the implementation: against
  the same discretisation computed independently (T2) and against the Timoshenko continuum (T3).
- **(a4).** No existing function body changes (D4): every call of the existing API takes the same operations. Checked by the
  new record N1 (written and run on the base before any library line), the existing beam bit records of `rcfd-fem` and
  `baseline_hashes` of `rcfd-api`.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0580 (whole: arithmetic 1 to 5, D1 to D7, amendment 2, the follow-up text); ADR 0587 (D1,
  D2, the limit `2 tolerance` of the backward error and its estimate, "For the lead"); ADR 0572 and 0573 (the form of "For the
  lead").
- `crates/rcfd-fem/src/follower.rs` (whole, in particular `tangent_modes_full`, `buckling_factors_full`, `ComplexModes`),
  `geometric.rs` (`Prestress`, `assemble_geometric`, `add_on_pattern`, `prestressed_modes`), `geometric_struct.rs` (the
  header, `prestress_from_static`), `beam.rs` (the header, `plane_shape_coefficients`, `plane_stiffness`, `plane_functions`,
  `plane_mass`, `local_stiffness`, `local_mass`, `to_global`, `plane_slopes`, `plane_geometric`, `local_geometric_stiffness`),
  `nonlinear.rs` (`add_triplets`), `model.rs` (`Load::NodalForce`, `DofMap`), `mesh.rs` (the API list), `error.rs`
  (`MissingComponent`).
- Tests: `sa_geometric_struct.rs` (the beam model, `planar`, `clamped`, `static_prestress`, B1 to B3), `sa_f653_default_bits.rs`
  (the digest form).

## Arithmetic done before the code (disclosed)

1. **The Beam2 element in one plane, re-implemented** (`beam2_beck_reference.py`, method A; numpy, from the formulas of
   `beam.rs` as read). Units `E I = 1`, `rho A = 1`, `L = 1`; per node `(u, v, psi)`; square section of side `h`, `I/A =
   h^2/12`; axial `E A/l` and the consistent axial mass; in the bending plane the stiffness of `plane_stiffness` (shear
   parameter `Phi = 12 E I/(kappa G A l^2)`, `kappa = 5/6`, `nu = 0.3`), the mass of `plane_mass` (deflection and rotation
   functions of `plane_shape_coefficients`, four-point Gauss, rotary inertia `rho I`), the geometric stiffness of
   `local_geometric_stiffness` in the plane (`N/l` on `u`, `N int d'^T d'` with the deflection functions). The follower term
   (D1) in the plane: `F = -P e_x` and `theta = psi e_z` give `f_v = -P psi` at the tip, so `K_f[v_tip, psi_tip] = -P`. The
   pencil `(K + p (K_sigma - K_f)) x = W M x`, `K_sigma = -P G_1` (axial force `N = -P`), clamped at `x = 0`; flutter: the
   onset of a complex pair among the 4 eigenvalues nearest 0 (`|Im| > 1e-6 |W|`), bisection on `[16, 24]`, 30 halvings,
   `p_cr` the midpoint of the final bracket, `W` the real part of the pair at its upper end. `L/h = 100`:

   | N | method A (Beam2) `p_cr` | dev. from `20.0509536` | method B (the Hermite limit, `Phi = 0`, no rotary inertia) |
   |---|---|---|---|
   | 10 | 20.0347377844 | `-8.087e-4` | 20.0523361750 (`+6.9e-5`) |
   | 20 | 20.0332858078 | `-8.811e-4` | 20.0510415696 (`+4.4e-6`) |
   | 40 | 20.0331625901 | `-8.873e-4` | 20.0509591363 (`+2.8e-7`) |
   | 80 | 20.0331471600 | `-8.881e-4` | 20.0509539805 (`+1.9e-8`) |

   Method B reproduces the Hermite numbers of ADR 0580 arithmetic 2 to the printed digits (a check of the script). The
   coalescence at `N = 40`: `W = 121.189706` (`-1.259e-3` against `121.342510`). Registered model: `N = 40`, full value
   `p_A = 20.033162590116262`, `W_A = 121.18970557932914`.
2. **The Timoshenko continuum** (method C; the transfer-matrix determinant of ADR 0580 arithmetic 3, copied): `kappa G A
   L^2/(E I) = (5/6)/(2.6)/(h^2/12)`, rotary inertia `h^2/12`, tip conditions `psi'(L) = 0` and `w'(L) = psi(L)`: `p_T =
   20.03314308` (`-8.883e-4`). The tip condition is the same for a force following the section and one following the axis: the
   transverse balance at the tip is `kappa G A (w' - psi) - P w' + P t = 0` with `t = psi` (section) or `t = w'` (axis), which
   gives `w' = psi` in both cases (`kappa G A != P`). Method A converges to method C: `N = 40` within `9.7e-7`, `N = 80`
   within `2.0e-7` (the error ratio about 4.8, `O(h^2)` here because the mass and geometric stiffness of the interdependent
   element are not exact for the continuum; the stiffness is). So the Beam2 element with the follower of D1 approximates the
   Timoshenko Beck column, and its departure from the EB value is physical (shear and rotary inertia), `-8.88e-4` at `L/h =
   100`.
3. **Spectra and controls** (method A, `N = 40`, `L/h = 100`; `W` in `E I/(rho A L^4)`): at `p = 16` the 4 nearest 0 are real,
   `49.48, 245.38, 3061.2, 12964`; at `p = 24` a pair `94.97 +/- 93.76 i` and `2693.4, 12173`. The dead tip force (no `K_f`) at
   `1.01 p_A`: real, `14.14, -422.12, 2230.2, 11627` (the dead column has buckled: Euler `pi^2/4` and `9 pi^2/4`). The
   symmetric part of `K_f` at `1.01 p_A`: real, `-54.40, 192.06, 2866.2, 12545` (spurious divergence, as on the solid). At `p_A`:
   `||K + p (K_sigma - K_f)||_F = 7.4e7`, `||M||_F = 0.16` (units above).
4. **The 3-D form of D1** (first principles). `(theta x F)_i = eps_ijk theta_j F_k`, so `K_f[(a, i), (a, 3 + j)] = d f_i/d
   theta_j = lambda eps_ijk F_k` at each loaded node `a`, the only non-zero entries (rows of translations, columns of
   rotations; the force does not depend on the translations). For `F = -P e_x`: `K_f[y, theta_z] = eps_{yzx} (-P) = -P`,
   `K_f[z, theta_y] = eps_{zyx} (-P) = +P`, the in-plane entry of item 1 and its mirror. The torsion rotation gives `theta_x e_x x
   e_x = 0`: a force along the axis does not couple torsion. For a straight column the two bending planes are uncoupled by
   `K`, `M`, `K_sigma` and `K_f`; the flutter load in the weak plane depends on the section only through `I/A` (the shear
   parameter `12 E I/(kappa G A L^2)` and the rotary inertia `I/(A L^2)`), so a rectangle `b x h` with `h` the weak height
   has, in `E I_weak/L^2`, the flutter load of the square `h x h` exactly (in exact arithmetic), at any orientation of the axis.
5. **The estimates of the remaining checks.** The bisection resolution: the bracket `8/2^30 = 7.5e-9`, `3.7e-10` relative; the
   classification is exact down to a distance from `p_cr` far below that (the split of the pair `~3.6 sqrt(delta) W_cr`,
   ADR 0580 arithmetic 4), so two independent computations of the same discretisation agree to one bracket unless their
   eigenvalues differ by more than `~1e-9` relative near `p_cr`: T2 `<= 4e-10`, limit `1e-8`; T7 likewise. The backward
   error of each pair: at most `rho est <= tolerance = 1e-10` (ADR 0587 D1, `rho < 1` at shift 0), measured `3.8e-14` on the
   solid Beck pencils: limit `2 tolerance = 2e-10` (the limit of ADR 0587). T10 (the full solver against the symmetric
   `geometric::prestressed_modes` without follower): as ADR 0580 F5 (`1.3e-9` measured): limit `1e-8`.

## Design

### D1. `follower::FollowerTipForce` (`follower.rs`, new type)

`FollowerTipForce::new(model: &SolidModel, set: &str, force: Vec3) -> Result<FollowerTipForce>`: the force `force` (N, global
axes, of the reference load, load factor 1) on each node of the node set `set`, fixed in the end section so that it turns
with the node's rotation. Refusals: an unknown node set (`UnknownSet`, kind node); a non-finite component (`InvalidValue`, key
`"force"`); a node that is not a node of a Beam2 element (`InvalidValue`, key `"follower force"`; shell rotations are not a
section rotation); a node without six components (`MissingComponent`, role `"follower force"`).

- `force(&self, u: &[f64], load_factor: f64) -> Result<Vec<f64>>`: `f_a = lambda (F + theta_a x F)` on the translations of
  each loaded node `a`, `theta_a` the node's rotation unknowns of `u`, zero elsewhere; linear in `theta` (the small-rotation
  kinematics of Beam2): at `u = 0` exactly `lambda F` on the translations (the dead `Load::nodal_force(set, F)`). A `u` of
  the wrong length is refused (`FieldLength`, key `"displacement"`).
- `load_stiffness(&self, load_factor: f64) -> Vec<(usize, usize, f64)>`: `K_f = d f/d u`, the entries of arithmetic 4
  (`lambda eps_ijk F_k` at (translation `i`, rotation `j`) of each loaded node), unsymmetric; independent of `u`.
- `force_vector(&self) -> Vec3`, `name(&self)`: accessors.

### D2. `follower::prestressed_modes_full` (`follower.rs`, new function)

`prestressed_modes_full(model: &SolidModel, prestress: &Prestress, forces: &[&FollowerTipForce], load_factor: f64, options:
&ModalOptions) -> Result<ComplexModes>`: the pencil `(K + lambda (K_sigma - sum K_f)) phi = omega^2 M phi`, `K` the stiffness
(`assemble::point_stiffness`), `K_sigma` of `prestress` (`geometric::assemble_geometric`; the pre-stress of the reference
load, for instance `Prestress::from_static` of the linear static solve with the dead forces), `K_f` of each force at load
factor 1 (D1), `M` the consistent mass; assembled as `K + lambda K_sigma` on the common pattern (`add_on_pattern`, the
geometric values scaled by `lambda` first) and `-lambda K_f` by `nonlinear::add_triplets`. Reduced on the model's prescribed
equations (zero values only; another value refused, key `"constraint"`), solved by `nearest_pencil_eigenpairs` with `count =
options.modes`, `tolerance = options.tolerance`, the shift rule of `tangent_modes_full` (`None`: 0 when every rigid motion is
held, refused otherwise, key `"shift"`; `Some(Value(s))`: `s`; `Some(Auto)`: refused, key `"shift"`). Refused besides: a
non-finite `load_factor` (key `"load factor"`), the transformation path (key `"load stiffness"`, as `buckling_factors_full`),
the model checks of `geometric::prestressed_modes` (`validate`, `check_supported_with_materials`), the mode count and the
mass diagonal as `tangent_modes_full`. The result `ComplexModes` (ADR 0580 D3, with the backward errors of ADR 0587).

### D3. The tests (`crates/rcfd-fem/tests/sa_f665.rs`)

- **Planar column**: `L = 1`, 40 Beam2 elements along `x`, `rectangle_section(b, b, 0.1406 b^4, 5/6)`, `b = 0.01` (`L/h =
  100`), orientation `[0, 1, 0]` (bending in `x-y` with `theta_z`, `I_z`), `E = 2e11`, `nu = 0.3`, `rho = 7850`; node `0`
  held in all six components, every node held in `u_z, theta_x, theta_y` (the `planar` constraint of `sa_geometric_struct.rs`).
  `P_ref = 20 E I_z/L^2`; the follower `FollowerTipForce::new(m, "xl", [-P_ref, 0, 0])`; the pre-stress `Prestress::from_static`
  of the model with `Load::nodal_force("xl", [-P_ref, 0, 0])` solved by `solve_static`. At `p`: `lambda = p/20`,
  `prestressed_modes_full(m, pre, [fp], lambda, ModalOptions::new(4))`.
- **Oblique 3-D column**: the axis `a = (1, 2, 2)/3`, `L = 1`, 40 elements, `rectangle_section(0.02, 0.01, 0.229 * 0.02 *
  0.01^3, 5/6)` (the weak height `h = 0.01` along `e_3`, `I_y = b h^3/12`), orientation `[0, 0, 1]`, node 0 held in all six,
  nothing else held; `P_ref = 20 E I_y/L^2`, `F = -P_ref a`, 6 modes.
- **Classification** `flutter(p)`: `Ok` and some returned eigenvalue with `|Im| > 1e-6 |omega^2|` (as ADR 0580 D5).
  Bisection on `[16, 24]`, both ends checked first (T5), 30 halvings; `p_meas` the midpoint of the final bracket; `W_meas =
  Re(omega^2) rho A L^4/(E I)` of the complex pair at the final upper end.

### D4. What changes

New: in `follower.rs` the type `FollowerTipForce` with its methods, the function `prestressed_modes_full`, their imports and
lines of the module documentation. Nothing else; no line of an existing function, of `beam.rs`, `geometric.rs`,
`geometric_struct.rs`, `eigen_pencil.rs`. A visibility change only if the compiler requires one (recorded then).

### D5. The bit record N1 (`crates/rcfd-fem/tests/sa_f665_default_bits.rs`)

Written after this commit and run on the unchanged base 06b66e35 before any library line; FNV-1a 64 digests (the form of
`sa_f653_default_bits.rs`) of, on the planar and the oblique models of D3: `solve_static` with the dead tip force (the
displacement); `modal_analysis`, 4 modes, `ResidualTest::Backward` (eigenvalues, modes, residuals, iterations);
`geometric::prestressed_modes` with `pre.scaled(0.05)`, 4 modes, `Backward`; `buckling::factors`, 2 factors (factors, modes,
residuals, iterations, Sturm count).

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| T1 | a3 | planar column: `|p_meas/20.0509536 - 1|` | `<= 2e-3` | `8.87e-4` (arithmetic 1; Timoshenko continuum `8.88e-4`) |
| T2 | a3 | planar: `|p_meas/p_A - 1|`, `p_A = 20.033162590116262` (the same discretisation, arithmetic 1) | `<= 1e-8` | `<= 4e-10` |
| T3 | a3 | planar: `|p_meas/p_T - 1|`, `p_T = 20.03314308` (Timoshenko continuum) | `<= 1e-5` | `9.7e-7` |
| T4 | a3 | planar coalescence: `|W_meas/121.342510 - 1|`; `|W_meas/W_A - 1|`, `W_A = 121.189706` | `<= 3e-3`; `<= 1e-6` | `1.26e-3`; `~1e-9` |
| T5 | a3 | the bracket ends: `flutter(16)` false, `flutter(24)` true (planar and oblique) | hold | arithmetic 3 |
| T6 | a2 | every pair of every evaluation of T1 and T7: `backward_errors` | `<= 2e-10` | `<= 1e-10`, measured `~1e-13` elsewhere |
| T7 | a1, a2 | oblique 3-D column: `|p_3D/p_meas - 1|` (against the planar T1 value) | `<= 1e-8` | `<= 4e-10` (arithmetic 4) |
| T8 | a1 | load stiffness: (i) on the oblique model for three vectors `u` (SplitMix64, seed `0x0589_F665`, entries in `(-1, 1)`): `max_r |f(u) - f(0) - K_f u|_r / (|F| max |u|)` at `lambda = 0.7`; (ii) `force(0, 1)` equals `F` on the translations of the tip node and 0 elsewhere, exactly; (iii) planar: `K_f[y, theta_z] = -P_ref` and the oblique model's `K_f` has exactly 6 entries | `<= 1e-14`; exact; exact | `~1e-16`; 0; arithmetic 4 |
| T9 | a2 (control) | planar, no follower (`forces = []`, the dead tip force) at `lambda = 1.01 p_meas/20`: every returned `|Im| <= 1e-8 |omega^2|` and the lowest `Re omega^2 < 0` | hold | real, `-422` (arithmetic 3) |
| T10 | a2, a4 | planar, no follower at `p = 1` (`lambda = 0.05`): 4 eigenvalues of `prestressed_modes_full` against `geometric::prestressed_modes(m, pre.scaled(0.05), ModalOptions::new(4).residual_test(Backward))`: `max |w_f/w_s - 1|`; `max |Im|/|w|` | `<= 1e-8`; `<= 1e-8` | `~1e-9`; 0 |
| T11 | D1, D2 | refusals: `FollowerTipForce::new` with an unknown set, a NaN force (key `"force"`), a node of a Hex8 model (key `"follower force"`); `prestressed_modes_full` with `Shift::Auto` and without the clamp (shift `None`) (key `"shift"`), a NaN load factor (key `"load factor"`), a linear constraint (key `"load stiffness"`), a non-zero prescribed value (key `"constraint"`), `modes = 0` (key `"modes"`) | hold | |
| N1 | a4 | the digests of D5 at the base | exact | no existing line changed |
| N2 | a4 | the existing beam and follower bit records: `sa_geometric_struct_bits`, `sa_geometric_struct_f558_bits`, `sa_beam_warping_f598_bits`, `sa_beam_warping_f626_bits`, `sa_beam_warping_f635_bits`, `sa_beam_warping_f661_bits`, `sa_follower_default_bits`, `sa_f653_default_bits`, `sa_f664_default_bits` (release) | pass | |
| N3 | a4 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | |

Files: `crates/rcfd-fem/tests/sa_f665.rs` (T1 to T11), `crates/rcfd-fem/tests/sa_f665_default_bits.rs` (N1). A check that
would take more than about two minutes in release is an ignored heavy record run once, with a fast test on the same code path.

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the sign of `K_f` flipped (`-eps_ijk F_k`) | T1, T2, T8 |
| MU2 | `K_f` not scaled by `lambda` in `prestressed_modes_full` | T1, T2 |
| MU3 | `K_f` transposed (rows of rotations, columns of translations) | T1 or T5, T8 |
| MU4 | the columns of `K_f` at the translations (`j` in place of `3 + j`) | T1 or T5, T8 |
| MU5 | `prestressed_modes_full` ignores the forces | T5 (no flutter at `p = 24`) |
| MU6 | `K_sigma` not scaled by `lambda` | T1, T2 |

## Decision rule

- **done**: every check of the table met, every planned mutation caught by at least one check, no check met only after a
  post-run amendment, fmt and clippy clean, N1 to N3 unchanged.
- **partial**: N1 to N3 unchanged and fmt and clippy clean, but a registered check missed (kept as a `*_known_miss` test with
  its limit and the measured number), or a mutation not caught (none is declared equivalent), or a check met only after a
  post-run amendment.
- **todo**: otherwise, in particular any change of the default bits.

## Limits (not covered)

- The flutter load is found by a bisection in the test, not by a library routine; the library returns the complex spectrum at
  a given load factor.
- Linear pre-stress only: `K_sigma` of the linear static state scaled by `lambda`, no Newton equilibrium, no shortening, no
  initial-displacement terms; the follower linear in the rotation (small rotations). No `Contribution` for the Newton driver
  (it refuses beams).
- The buckling path: `buckling_factors_full` keeps its `FollowerPressure` argument; a tip force there is not added (a
  static `K x = mu (K_f - K_sigma) x` of Beck's column has no real factor, ADR 0580 arithmetic 3).
- A follower moment, a force on a shell node, the opt-in moment and Wagner terms of ADR 0508 with a follower (the pre-stress
  is taken as given; not checked), the transformation path (refused), no case-file key.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **The entry point.** Taken: a new function `follower::prestressed_modes_full` beside `geometric::prestressed_modes`, with the
   load factor as an argument (the acceptance's `lambda`), returning `ComplexModes`; the existing functions are not touched.
2. **What the force follows.** Taken: the node rotation (the section rotation of Beam2), the natural unknown at a node; the
   axis slope would need the element's interpolation at the end. Both give the same Timoshenko tip condition (arithmetic 2).
3. **The column.** Taken: `L/h = 100`, 40 elements; the shear and rotary-inertia departure `-8.9e-4` is inside the limit
   `2e-3` of T1, and T3 checks it against the Timoshenko continuum. A more slender column would bring the EB value closer but
   widens the spread of the spectrum (`E A L^2/(E I) = 12 (L/h)^2`).
4. **No `Contribution`, no buckling-path change, no case-file key** (limits above; a schema golden would change).

## Addendum 1 (2026-11-15): results

Runs: `sa_f665` (`rcfd-fem`) once at 1245ee40 (`--release -- --nocapture --test-threads=1`; `f665-scratch/run1-f665.log`). N1
recorded at the base 06b66e35 before any library line (`n1-base.log`, `n1-base2.log`, commit 8bc1af34) and passed at the tip.
At the tip, release: the beam and follower bit records of N2 and the tests that cover `follower.rs` (`sa_follower`, `sa_f572`,
`sa_f653`, `sa_f664`, `sa_shell_follower`): all pass (`n-tests.log`); `baseline_hashes` of `rcfd-api` (3 passed, 1 ignored;
`baseline.log`). `cargo fmt -p rcfd-fem -- --check` and `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` clean.
No limit was changed after a run; no amendment.

| Id | Limit | Measured | Met |
|---|---|---|---|
| T1 `|p_meas/20.0509536 - 1|` | `<= 2e-3` | `8.873e-4` (`p_meas = 20.033162590116`, below the EB value: shear and rotary inertia) | yes |
| T2 `|p_meas/p_A - 1|` (same discretisation, Python) | `<= 1e-8` | `0` (the same final bracket) | yes |
| T3 `|p_meas/p_T - 1|` (Timoshenko continuum) | `<= 1e-5` | `9.74e-7` | yes |
| T4 `|W_meas/121.342510 - 1|`; `|W_meas/W_A - 1|` | `<= 3e-3`; `<= 1e-6` | `1.259e-3`; `3.5e-9` (`W_meas = 121.189706`) | yes |
| T5 brackets (planar, oblique) | hold | `p = 16`: all real; `p = 24`: a pair `W = 94.97 +/- 93.77 i` (both models) | yes |
| T6 backward errors, every pair of every evaluation | `<= 2e-10` | planar `5.3e-14`, oblique `1.9e-13` | yes |
| T7 `|p_3D/p_meas - 1|` (oblique axis `(1, 2, 2)/3`, rectangle 2:1) | `<= 1e-8` | `0` (the same final bracket) | yes |
| T8 (i) linearity of `force` against `K_f`; (ii) `force(0, 1)`; (iii) entries | `<= 1e-14`; exact; exact | `1.4e-16`; bitwise; `K_f[y, theta_z] = -P_ref` exactly, 6 entries | yes |
| T9 dead tip force at `1.01 p_meas`: real, lowest `Re < 0` | hold | real (`Im = 0`); `W = 12.27, -380.72, 2255.1, 11666` | yes |
| T10 against `geometric::prestressed_modes`; `|Im|/|w|` | `<= 1e-8`; `<= 1e-8` | `2.97e-11`; `0` | yes |
| T11 refusals | hold | all nine with the registered key | yes |
| N1 default bits | exact | the 8 digests equal | yes |
| N2 existing beam and follower bit records | pass | pass (nine files) | yes |
| N3 `baseline_hashes` | pass | pass | yes |

Remarks. The Beam2 column flutters at `20.0332 E I/L^2` against the EB `20.0509536` (`-8.87e-4`), within `9.7e-7` of the
Timoshenko continuum of the same section: the departure is physical, not a defect of the load stiffness. T9: arithmetic 3
printed `14.14, -422.12, ...` for the dead tip force; that number came from a defect of the script (zeroing the entry
`B[v_tip, psi_tip]` removed the geometric term together with the follower term). A diagnostic after the run
(`f665-scratch/dead_load_fix.py`, `dead_load_fix.log`, removing only the follower term) gives `12.2654, -380.718, 2255.13,
11665.86`, the rcfd numbers to the printed digits. The registered check (real, a negative eigenvalue) was not affected.

### Mutations (scratch copy of 1245ee40 by `git archive | tar -x -m`, one at a time, `f665-scratch/mutate.py`, `mutations.log`)

| Id | Caught by |
|---|---|
| MU1 sign of `K_f` flipped | T1 to T5 (no flutter up to `p = 24`), T8 |
| MU2 `K_f` not scaled by `lambda` | T2 (`7.2e-4`), T3, T4; not by T1 (the shift `7.2e-4` stays inside its `2e-3`) |
| MU3 `K_f` transposed | T8 only: the spectrum does not change, because `(K + lambda K_sigma) - lambda K_f^T` is the transpose of the registered matrix (`K`, `K_sigma`, `M` symmetric), and a pencil and its transpose have the same eigenvalues |
| MU4 columns of `K_f` at the translations | T1 to T5, T8 |
| MU5 forces ignored | T1 to T5 (no flutter at `p = 24`) |
| MU6 `K_sigma` not scaled by `lambda` | T1 to T5, T10 (`1.8`) |

Six of six caught. The registration expected MU2 to be caught by T1 and MU3 by T1 or T5 as well; both were caught by other
registered checks.

### Deviations

- None in the checks. The expected numbers of the dead-load control in arithmetic 3 were wrong (script defect, above); the
  check did not depend on them.
- T7 and T9 call the bisection of T1 again to get `p_meas` (the same deterministic computation, the same numbers); no check
  was re-run to pick a result.

### Not run

- The whole workspace suite and the whole `rcfd-fem` suite (the brief); no existing line of `rcfd-fem` changed (D4: only
  additions at the end of `follower.rs` and in its module documentation). No case-file key.

### Decision

**done** by the registered rule: every check met in the one registered run, six of six mutations caught, no amendment, fmt
and clippy clean, N1 to N3 unchanged. Every clause of the acceptance is met: a follower tip force on Beam2 nodes with its load
stiffness (a1: T8, T11), the complex modes of `K + lambda (K_sigma - K_f)` with the mass (a2: T5, T6, T9, T10), Beck's column
of Beam2 elements within `8.87e-4` of the derived `20.0509536` (a3; registered limit `2e-3`, and within `9.7e-7` of the
Timoshenko continuum), existing beam results unchanged (a4: N1 to N3).

### Remaining questions (recommendations for the lead)

- **Q1** (the buckling path with a tip force): options (a) let `buckling_factors_full` take tip forces (a new argument or
  entry point); (b) leave it, since Beck's column has no static factor. Recommendation (b) until a beam follower case with
  divergence is needed.
- **Q2** (a `Contribution` for the Newton driver): only when the driver takes beams (a geometrically nonlinear beam); not now.
- **Q3** (case-file keys for the follower tip force and the flutter modes): with the keys of ADR 0560 Q1 and ADR 0580 Q3,
  when those are added (a schema golden changes; maintainer approval).
- **Q4** (MU3 leaves the spectrum unchanged): no eigenvalue check can tell the transpose of `K_f` apart; the element check T8
  guards it. Recommendation: keep T8; no action.

## For the lead: roadmap entries

> Proposed status: done.

> Structural solver, advanced tier. Follow-up of F-653 (ADR 0580, 2026-11-15). DONE. ADR 0589, 2026-11-15: new rcfd_fem::follower::FollowerTipForce (a tangential follower force on Beam2 nodes, fixed in the end section so that it turns with the node rotation, f = lambda (F + theta x F); its load stiffness K_f with the entries lambda eps_ijk F_k at (translation i, rotation j), unsymmetric; non-beam nodes refused) and follower::prestressed_modes_full (the complex modes of (K + lambda (K_sigma - sum K_f)) phi = omega^2 M phi about the linear pre-stress of the reference load, no Newton tangent, so beams take it; shift-invert Arnoldi of ADR 0580 with the backward errors of ADR 0587; transformation path refused). Beck's column of 40 Beam2 elements, L/h = 100, bisection on the onset of a complex pair: p_cr = 20.03316 E I/L^2 (-8.87e-4 from the derived 20.0509536, registered limit 2e-3; the shear and rotary inertia of the Timoshenko element: within 9.7e-7 of the Timoshenko continuum and equal to an independent re-implementation of the same discretisation), coalescence Omega^2 = 121.19 (-1.26e-3); an oblique 3-D column of a 2:1 rectangle gives the same load to the last bit; the dead tip force gives a real spectrum with a negative eigenvalue; without followers the full path agrees with geometric::prestressed_modes within 3e-11; backward errors at most 1.9e-13. Six of six mutations caught (the transpose of K_f leaves the spectrum unchanged and is caught by the element check); existing beam results (new record N1, the existing beam and follower records) and baseline hashes unchanged.

> Note for F-653 (append to its note): Its open question 2 (a Beam2 follower tip force with a linear pre-stress flutter path) is answered by F-665 (ADR 0589, 2026-11-15): Beck's column of Beam2 elements within 8.9e-4 of 20.0509536 (the Timoshenko departure).
