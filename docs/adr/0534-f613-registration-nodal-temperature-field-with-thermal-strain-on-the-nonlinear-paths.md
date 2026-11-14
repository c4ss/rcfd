# ADR 0534: F-613 registration: a nodal temperature field against time on the nonlinear paths, with thermal strain

Status: **REGISTERED 2026-11-14, written before any code of F-613 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-613 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the scalar arithmetic below was done by hand and evaluated with one plain python script in the scratch
folder (`w49i-work/f613-scratch/probe.py`, log `probe.log`; closed forms only, no rcfd code; disclosed, its numbers quoted
below). The bit record B1 of the paths F-613 touches is a test file written and run on the unchanged base 679081a1 after this
commit and before any library code (D8). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-613 (deps F-592), the follow-up proposed by [ADR 0526](0526-f592-registration-temperature-history-on-the-quasi-static-driver-and-the-nonlinear-transient-integrator.md)
(its remaining question 3). Number 0534 given by the lead. Branch `f613-work` from 679081a1 (which contains F-592), worktree
`w49i-work/f613`, scratch `w49i-work/f613-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no
edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the
crate graph unchanged; no existing test edited; clean-room. The brief asks for the F-613 entry of `docs/ROADMAP.md` and
`docs/roadmap.json` to be updated by this branch in a separate last commit (the svgs not regenerated). Areas of other agents not
touched: the eigensolver ordering (F-595), the Craig-Bampton enrichment (F-603), the supernodal factor in the contact driver
(F-615). `lib.rs` is not edited (the new type lives in `thermal.rs`).

## What F-613 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. The quasi-static driver and the nonlinear transient integrator take a nodal temperature field against time (a table of fields, the step-end value) with the thermal strain of the material's expansion; a uniform field gives the bits of ADR 0526 without expansion, and a free thermal expansion of an unconstrained elastic cube under a temperature step equals alpha dT within 1e-12. Models without a temperature keep their bits.

The brief adds: register (b1) the field interpolation (nodal to integration point by shape functions), (b2) the thermal strain
form at finite strain (additive Green-Lagrange or multiplicative, justified against "equals alpha dT within 1e-12"), (b3) the
reference temperature, (b4) how "without expansion" is obtained for the uniform-field bits check, (b5) which materials take it
(refuse the rest); (b6) opt-in, a default-bits record first, `baseline_hashes` unchanged; (b7) no case-file key (an open
question); (b8) mutations the checks catch; (b9) every aggregated test error required finite.

Clauses: (a0) every limit registered here; (a1) the quasi-static driver (`nonlinear::solve_in_time_with`) takes a nodal
temperature field against time, a table of fields evaluated at the end of every attempt, with the thermal strain; (a2) the
nonlinear transient integrator (`nl_transient::Integrator`) the same, at the end of every sub-step; (a3) a uniform field gives
the bits of ADR 0526 without expansion; (a4) a free thermal expansion of an unconstrained elastic cube under a temperature step
equals `alpha dT` within `1e-12`; (a5) models without a temperature keep their bits.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0526 (whole: D1 to D7, the checks, the addendum); ADR 0281 (the linear thermal strain: the
  interpolation of the nodal differences `T_b - T_ref` with the element's own functions at the points of the stiffness rule,
  arithmetic 2 and 3).
- `crates/rcfd-fem/src/thermal.rs` (whole: `temperature_field`, `thermal_strain`, `point_temperature_change`,
  `element_thermal_load`, `add_thermal_load`), `material.rs` (`PointContext`, `Material::stress`), `model.rs` (`Material`, its
  `expansion()`: the coefficient of the isotropic base of a wrapping variant, NaN without one; `check_elements`: the nonlinear
  paths refuse the orthotropic, tabulated and pending variants), `transient.rs` (`LoadTable::value`), `nonlinear.rs` (the
  kinematics, `evaluate`, `point_update`, `evaluate_fbar`, `check_driver_model` with the refusal of a temperature load,
  `internal_force_at`, `tangent_at`, `force_and_tangent`, `Problem`, `solve_from_at`, `TimeSteppingOptions`,
  `solve_in_time_with`), `nl_transient.rs` (`NlTransientOptions`, `Integrator::new`, `check_coverage`, `temperature_at`,
  `damping_at`, `start`, `substep`, `strain_energy`), `nl_transient/emc.rs` (its use of the temperature).
- Tests: `sa_temp_history.rs` (the cube, the rod, `strain_step`, `tight`, `flat_history`, `vibrate`, `advance`, S2, V2T),
  `sa_temp_history_default_bits.rs` (the form of a bit record), `sb_thermal.rs` (the 3-2-1 support, `solve_static_with`).

## Arithmetic done before the code (disclosed)

1. **The thermal strain at finite strain: the multiplicative split.** `F = F_e F_th` with the isotropic thermal stretch `F_th =
   theta I`, `theta = 1 + alpha dT`, `dT = T - T_ref` (Lu and Pister 1975, the decomposition of the deformation of an isotropic
   thermoelastic solid, cited from memory for orientation; everything used is derived here). Then `C_e = F_e^T F_e = C/theta^2`
   and the elastic Green-Lagrange strain `E_e = (C_e - I)/2 = (E - E_th)/theta^2` with `E = (C - I)/2` and `E_th = (theta^2 -
   1)/2 I = (a + a^2/2) I`, `a = alpha dT` (computed as `a (1 + a/2)`, no cancellation). The material is called at `E_e`
   (with its state) and gives `S_e` and `D_e = dS_e/dE_e`. With the stored energy per unit volume of the stress-free
   (intermediate) configuration, `W(C) = J_th W_e(E_e)`, `J_th = theta^3`: `S = 2 dW/dC = theta^3 S_e / theta^2 = theta S_e`
   and the tangent `dS/dE = theta D_e / theta^2 = D_e / theta`. For a material with history (J2, viscoelastic, creep) the same
   push-back is taken: the material's state lives in the intermediate configuration (its strains are elastic-configuration
   strains). **Why multiplicative** (b2): a free body at a uniform `dT` is stress-free at `F = theta I`, a stretch of exactly
   `1 + alpha dT`: `u = alpha dT (X - X_A)`, so "equals alpha dT" holds for the displacement gradient, at any `dT`. The additive
   Green-Lagrange form `E_e = E - alpha dT I` is stress-free at `(lambda^2 - 1)/2 = alpha dT`, `lambda = sqrt(1 + 2 a)`: at `a =
   1e-2` the stretch minus one is `9.950494e-3` (probe), `4.95e-5` from `alpha dT`, which no 1e-12 limit can take. At small
   `a` the two agree to first order (`E_e = E - a I + O(a^2, a |E|)`), so the linear thermal path of ADR 0281 is the
   small-strain limit of this one (check LIN).
2. **The field interpolation** (b1). A table of nodal fields against time (D1) gives at the end time `t1` of an attempt the
   nodal temperatures `T_b(t1)`, each by the rule of `LoadTable::value` (the point value at a point time, `v_i + s (v_j - v_i)`
   between). At a point `q` of the stiffness rule of an element, with the shape function values `N_b(q)`: the point
   temperature `T_q = sum_b N_b T_b` (the temperature of the point context, read by the shift of a viscoelastic material) and the
   point change `dT_q = sum_b N_b (T_b - T_ref)` (the nodal differences formed first, as `thermal::point_temperature_change`,
   ADR 0281 arithmetic 2). When all nodal values of the element have the same bits `T_0`, the interpolant is the constant `T_0`
   exactly (the functions sum to one), and it is taken as such: `T_q = T_0`, `dT_q = T_0 - T_ref`, without the rounding of the
   sum. This is what makes a uniform field reproduce the bits of ADR 0526 (arithmetic 4).
3. **The reference temperature** (b3). The field history carries one reference temperature `T_ref` (the stress-free
   temperature, as `Load::Temperature { reference }` of ADR 0281), finite. `dT` is measured from it at every point and time; the
   start of a run need not be at `T_ref` (a field different from `T_ref` at the start gives a thermal strain from the start).
4. **The uniform field without expansion** (a3, b4). "Without expansion" is a material with `alpha = 0` (the expansion of the
   isotropic base): then `a = 0 * dT = 0` exactly at every point (also `-0`, which compares equal), and the point evaluation is
   then the call of the base (`point_update` at `E` with the point context), with no scaling: the thermal path is not entered
   (`a == 0` is tested, the arithmetic of `theta = 1` is not relied on). The models of ADR 0526 S2 and V2T have `alpha = 0`
   (`base(nu)` of `sa_temp_history.rs`). With a uniform field whose values at the table's times are the values of the ADR 0526
   table, the nodal values at any `t` have the bits of `table.value(t)` (the same formula, arithmetic 2), every element is
   uniform, so every point context is `with_time(t1, h)` with the temperature `table.value(t1)`: the contexts of ADR 0526 D2,
   D3, bit for bit. The damping tangent of the integrator (D5) is then formed at the same contexts and re-formed at the same
   sub-steps. Expected: bits equal (UB1 on the driver, UB2 on the integrator).
5. **Free expansion of an unconstrained cube** (a4). The cube `[0, 1]^3` of `2 x 2 x 2` Hex8, isotropic elastic `E = 2e11`,
   `nu = 0.3`, `alpha = 1e-4 1/K`, `T_ref = 300 K`, held statically determinately (3-2-1, as ADR 0281 arithmetic 3: corner A =
   (0, 0, 0) all three components, B = (1, 0, 0) y and z, C = (0, 1, 0) z). The field steps from `T_ref` to `T_ref + 100 K` at
   every node: the table `(0, T_ref), (h, T_ref + dT), (3 h, T_ref + dT)`, step ends `h, 2 h, 3 h`, `h = 1 s` (the elastic
   material does not read the time). `a = 1e-2`. The exact solution at every step end is `u = a (X - X_A)`: `F = theta I` is in
   every isoparametric element space, gives `E = E_th` (to rounding), `E_e = 0`, `S = 0`, satisfies the supports (`u(B) = (a, 0,
   0)`, `u_z(C) = 0`). Measure FE1: `max |u_i - a (X_i - X_A,i)| / L` over the step ends, nodes and components, `L = 1`.
   Rounding: `E - E_th` is a difference of numbers `1e-2` of relative rounding `1e-16`, so `E_e ~ 1e-18`, residual forces `~ E
   1e-18 L^2`, `u` error `~ 1e-17` to `1e-15` after the Newton iteration with the tight tolerance (`1e-13`, `tight()` of
   `sa_temp_history.rs`). Limit `1e-12` (acceptance). The stresses: the internal force at every equation (free and supported),
   `max |f_int| / (E a L^2)` (FE1s): rounding `~ 1e-15`, limit `1e-10` (the form of ADR 0281 c2). Control MU1 (additive form):
   `4.95e-5` (arithmetic 1).
6. **The step-end rule** (FE2). The same cube, the ramp `(0, T_ref), (4 h, T_ref + dT)`, step ends `h, 2 h, 3 h, 4 h`: at the
   step end `t_n` the field is `T_ref + dT t_n/(4 h)` and the elastic solution is `u = a_n (X - X_A)`, `a_n = alpha (T(t_n) -
   T_ref)` with `T(t_n)` from the same table formula (computed in the test with `LoadTable::value`). Measure as FE1, limit
   `1e-12`. Mutation MU5 (the attempt at the step-start field): the solution lags one step, `a/4 = 2.5e-3` (probe).
7. **The stress form at finite strain** (HB1). The cube held on all six faces (rollers: the normal component zero on `xmin`,
   `xmax`, `ymin`, `ymax`, `zmin`, `zmax`) under the step of 5. `u = 0` is exact: `F = I`, `E_e = -E_th/theta^2 = -(theta^2 -
   1)/(2 theta^2) I`, the St. Venant-Kirchhoff `S_e = (3 lambda + 2 mu) e I = E/(1 - 2 nu) e I`, `S = theta S_e`, uniform and
   isotropic, so the rollers carry it with no tangential traction (as ADR 0281 arithmetic 4): `S_xx = -E/(1 - 2 nu) (theta^2 -
   1)/(2 theta)`. Probe: `-4.975247524752e9 Pa` (the linear value `-E a/(1 - 2 nu) = -5e9 Pa`, ratio `0.99505`). The sum of the
   x internal forces at the nodes of `xmax` is `S_xx` times the face area 1 (with `F = I` the first Piola stress is `S`; the
   nodes of `xmax` are not on `xmin`; the faces `y`, `z` carry no `xy`, `xz` traction). Measure HB1: `|sum f_x(xmax) - S_xx| /
   |S_xx|`, rounding `~ 1e-15`, limit `1e-10`; and `max |u| / (a L)`, limit `1e-12` (HB1u). Mutations: `S = S_e` (the factor
   `theta` dropped, MU2): `9.90e-3` relative (probe); `S = S_e/theta^2` (the energy per reference volume) `2.94e-2`.
8. **The consistent tangent** (TG1). The element tangent of the thermal path is `B^T (D_e/theta) B + K_sigma(S)`. Check against
   the central difference of the assembled internal force with the field, at a random displacement of size `1e-2` of a bar
   `[0, 2] x [0, 0.5] x [0, 0.5]` of `2 x 1 x 1` Hex8 (36 equations), a non-uniform field with `a` up to `1e-2`, step `h_fd =
   1e-7`: truncation `~ h_fd^2 |f'''| / |K| ~ 1e-14`, rounding `~ 1e-16 |f| / (h_fd |K|) ~ 1e-16 1e-2 / 1e-7 = 1e-11`. Measure
   `max_ij |K_ij - K_fd,ij| / max_ij |K_ij|`, limit `1e-6`. Mutation MU3 (`D_e` unscaled): `~ a = 1e-2` relative on the material
   part.
9. **The interpolation of a non-uniform field, the small-strain limit** (LIN). A beam `[0, 2] x [0, 0.5] x [0, 0.5]` held 3-2-1,
   a deterministic non-uniform nodal field `T_b = T_ref + A r_b`, `r_b = 2 frac(0.618034 b) - 1` in `[-1, 1)` (node index `b`),
   `alpha = 1e-6`, `A = 1 K`: point strains `~ 1e-6`. The nonlinear driver (one attempt at the constant field, `tight()`) against
   the linear static solve with `Load::Temperature` of the same nodal field and reference (ADR 0281: the same interpolation of
   the nodal differences, the same points). They differ by the geometric nonlinearity and the multiplicative split, both
   relative `O(|H| + a) ~ 1e-6` (times the slenderness `L/h = 4` for the rotations of a thermal bending). Measure `e(A) = max
   |u_nl - u_lin| / max |u_lin|`; limit `1e-4` (margin about 10 over `4e-6`). Order: the difference is quadratic in the
   amplitude (relative linear), `e(A)/e(A/2)` in `[1.8, 2.2]` (an interpolation defect would give a ratio near 1). Two meshes:
   Hex8 `[8, 2, 2]` (LIN-h) and Tet10 from `tet_box [4, 1, 1]` (LIN-t). Mutation MU4 (the point temperature at the element mean
   of the nodal values): the thermal strain within each element changes by the order of the field's variation (here of the
   order of `A` itself, a random field): `e` of order `1e-1`, many decades above `1e-4`.
10. **The integrator at a thermal equilibrium** (FEi). The cube of 5 held 3-2-1, the constant field `T_ref + dT`, Rayleigh
    damping (mass `10`, stiffness `1e-6`, so that the damping tangent at the field is formed), started at `u0 = a (X - X_A)`
    (zero at the supported components), `v0 = 0`, 50 steps of `1e-5 s`, average acceleration: `f_int(u0) ~` rounding, so `a0 ~`
    rounding and the state stays at `u0`. Measure as FE1 over the 50 step ends, limit `1e-12`.
11. **The integrator under a field step** (V3T, the step-end rule in dynamics, as ADR 0526 arithmetic 7). The elastic bar
    `[1, 0.2, 0.2]` of `[3, 1, 1]` Hex8 with three symmetry planes (S3e of ADR 0526), `alpha = 1e-5`, `T_ref = 300 K`, two
    non-uniform fields `F1_b = 300 + 50 + 20 r_b`, `F2_b = 300 + 120 - 30 r_b` (`a` up to `1.5e-3`), Rayleigh mass `20` and
    stiffness `2e-8`, the start `u0 = 0`, `v0 = 40 m/s` axial at `xmax`, `dt = 2e-6`. Run A: the table `(0, F1), (20 dt, F1),
    (21 dt, F2), (80 dt, F2)` (inner times written `20.0 * dt`, `21.0 * dt`), 40 steps. Run B: 20 steps with the constant field
    `F1`, its state restored into an integrator with the constant field `F2`, 20 more. Every sub-step of A has the field of the
    same sub-step of B (step ends at or before `20 dt` take `F1`, after it `F2`); the damping of A is formed at `F1` and re-formed
    at `F2` with the arithmetic of B's formation at `F2`. Expected: `u`, `v`, `a`, internal force bit-equal at all 40 step ends
    and the state bytes equal. Control NC3: run A against the constant `F1` for 40 steps, `max |u_A - u_F1| / max |u_F1|`: the
    thermal strain changes by `~1e-3` over a bar of length 1, a displacement change `~1e-3 m` against a vibration of amplitude
    `v0/omega = 5.0e-3 m` (probe, `omega = (pi/2) c/L`): order `1e-1`; limit `>= 1e-2`.

## Design

### D1. The field history (`thermal.rs`)

`thermal::TemperatureFieldHistory` (non-exhaustive, `Clone`, `PartialEq`): a reference temperature and points `(t_k, field_k)`,
each field one temperature in K per node of the mesh. `new(reference, points)`: at least two points, the times finite and
strictly increasing, every field of the same non-zero length, every value and the reference finite (key `"temperature"`).
`constant(reference, field, t0, t1)`; `reference()`, `points()`, `range()`, `covers(t_a, t_b)` (exact comparison, as
`LoadTable`), `value(t) -> Option<Vec<f64>>` (each node by the formula of `LoadTable::value`, `None` outside). Internally
`thermal` also gets the point rule of arithmetic 2 and the multiplicative evaluation of arithmetic 1 (a `pub(crate)` function
that wraps the material call of `nonlinear`), and the model checks of D4.

### D2. The element evaluation (`nonlinear.rs`)

`evaluate` becomes `evaluate_with(..., thermal: Option<ElementTemperatures>)` (the element's nodal temperatures in its node
order, the reference, and whether the expansion acts); `evaluate` calls it with `None`, and with `None` every operation of the
base is executed as before. With `Some`, every point of the stiffness rule takes the point context of the attempt with the
temperature `T_q` (arithmetic 2), and when the expansion acts and `a = alpha dT_q != 0` the multiplicative evaluation of
arithmetic 1 (a point with `theta <= 0` refused, key `"temperature"`). `force_and_tangent` and the assembled internal force take
an optional nodal field (the values at the attempt's time, the reference, the expansion flag) and pass each element its slice;
`Problem` (the attempt) carries it; `solve_from_at` passes none. Public entries for the checks: `internal_force_with_field` and
`tangent_with_field` (model, `u`, states, context, the nodal temperatures, the reference; the tangent with a thread count).

### D3. On the quasi-static driver

`TimeSteppingOptions` gains `temperature_field: Option<TemperatureFieldHistory>` (`None` by `new`) and the builder
`temperature_field(..)`; `check()` refuses it together with `temperature` or `temperature_history` (key `"temperature"`).
`solve_in_time_with` refuses a field that does not cover `[0, t_last]` (key `"temperature"`) and the models of D4, and every
attempt (the instantaneous one at `t = 0`, every sub-step to `t1`, halved or not) takes the field's value at `t1` with the
expansion. Without a field nothing changes (B1).

### D4. Which models take it (b5); the refusals

A field is taken by solid elements of the `Full` formulation (Tet4, Tet10, Hex8, Wedge, Hex20, the kinds of `evaluate`) with a
material that has a single finite expansion coefficient: the isotropic elastic material and J2, viscoelastic and creep on an
isotropic base (the variants the nonlinear paths accept; the orthotropic and tabulated variants are refused there already).
Refused with a field (key `"temperature"` unless stated): a Shell4 or Shell3 block (the corotated shell has no thermal strain);
a Hex8 block of the `BBar` formulation (the F-bar element is not given the thermal path here); a material whose `expansion()` is
not finite (key `"expansion"`, defensive: no variant that the nonlinear paths accept has one); a field whose length is not the
number of nodes (`FieldLength`, key `"temperature"`); a nodal value at a table time with `1 + alpha (T_b - T_ref) <= 0` for the
material of an element of the node (checked before any attempt); on the integrator also the energy-momentum step and a
neo-Hookean model (`emc`, the discrete gradient has no thermal form), and the strain energy (`Integrator::strain_energy`, key
`"strain energy"`: the stored energy of the multiplicative split is not registered). The model's `Load::Temperature` stays
refused on both paths (the field is an option, as the temperature of ADR 0499 and the history of ADR 0526).

### D5. On the nonlinear transient integrator

`NlTransientOptions` gains `temperature_field` (`None` by `new`) and the builder; `Integrator::new` refuses it together with
`temperature` or `temperature_history` and the models of D4, and stores it. The field's value at the start time (`start`), at
the end of every sub-step (`substep`) is passed with the expansion; coverage at `start` and before a run (key
`"temperature"`), as the history of ADR 0526 D3. The damping (b-free choice, open question 2): the stiffness part `K_0` is the
tangent at `u = 0`, the empty states and the point temperatures of the field **without the thermal strain** (the stiffness of
the stress-free material at the temperature, the rule of ADR 0526 D4 carried to a field): formed in `new` at the field's first
values and re-formed at a sub-step whose nodal values differ in bits from those of the stored matrix. The saved state does not
carry the field; a state restored into an integrator with another field continues with that one (used by V3T).

### D6. What does not change

The kinematics, the material calls, the Newton attempt, the halvings, the creep limit, the integrator's step and saved state,
the energy-momentum step, the linear thermal path of ADR 0281: without a field every path executes its operations as before. No
case-file key (b7).

### D7. The checks (`crates/rcfd-fem/tests/sa_temp_field.rs`, no library code)

The helpers of `sa_temp_history.rs` (copied: `Checks`, `worse`, `refused_with`, `bits`, `tight`, `flat_history`, the cube, the
rod, `strain_step`, `rod_modes`, `vibrate`, `advance`; that file is not edited). Every aggregated value is required finite.

### D8. The bit record of the paths touched (B1)

`crates/rcfd-fem/tests/sa_temp_field_default_bits.rs`, written after this commit and run on the base 679081a1 before any
library code; FNV-1a 64 digests written into the file then. Content: `solve_in_time_with` of the shifted cube with the
temperature history of ADR 0526 (20 steps, a step of the table inside); of an elastic cube with a non-zero expansion and no
temperature (3 steps under a load history); `nonlinear::solve` of an elastic bar with a non-zero expansion; `internal_force_at`
and `tangent_at` of a J2 bar at a displacement and a temperature; `nl_transient` of the shifted rod with Rayleigh mass and
stiffness damping and the history of ADR 0526 (60 steps, the damping re-formed, with the state bytes); of an elastic bar with a
non-zero expansion and damping, no temperature (30 steps, with the bytes); of a BBar Hex8 bar (20 steps); the messages of the
refusal of a `Load::Temperature` by `solve_in_time_with` and by `Integrator::new`. Together with the default-bits files of the
crate (among them `sa_temp_history_default_bits.rs`, `sa_qs_visco_default_bits.rs`, `sa_nl_transient_default_bits.rs`,
`sa_emc_default_bits.rs`) and `baseline_hashes`, these are the bits of (a5).

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| FE1 | a1, a4 | free expansion of the 3-2-1 cube under the temperature step (arithmetic 5), driver: `max |u_i - a (X_i - X_A,i)| / L` over the 3 step ends, nodes, components | `<= 1e-12` | rounding `1e-17` to `1e-15` |
| FE1s | a4 | its internal forces `max |f_int| / (E a L^2)` at every equation and step end | `<= 1e-10` | rounding `~1e-15` |
| FE1c | set-up | the field at `t = 0` gives `u = 0` (`max |u| <= 1e-12 a L`); no halving | exact / as stated | arithmetic 5 |
| FE2 | a1 | the ramp of arithmetic 6: `max |u_i - a_n (X_i - X_A,i)| / L` over the 4 step ends | `<= 1e-12` | rounding |
| HB1 | a1 | the held cube (arithmetic 7): `|sum f_x(xmax) - S_xx| / |S_xx|`, `S_xx = -E/(1 - 2 nu) (theta^2 - 1)/(2 theta)` | `<= 1e-10` | rounding `~1e-15` |
| HB1u | a1 | its displacements `max |u| / (a L)` | `<= 1e-12` | `u = 0` exact |
| TG1 | a1 | the tangent with a field against the central difference of the internal force (arithmetic 8) | `<= 1e-6` | `~1e-11` |
| LIN-h | a1 | Hex8 beam, non-uniform field: `e(A)` against the linear thermal solve (arithmetic 9) | `<= 1e-4` | `O(1e-6)` times 4 |
| LIN-t | a1 | the same on Tet10 | `<= 1e-4` | the same |
| LINo-h, LINo-t | a1 | `e(A)/e(A/2)` on each mesh | in `[1.8, 2.2]` | 2 (relative difference linear in the amplitude) |
| UB1 | a3 | the driver: the S2 set-up of ADR 0526 (the shifted cube, `alpha = 0`, the table step `T_1 -> T_2`, the strain step, 100 steps, `tight()`) with the uniform field history of the table's values against `temperature_history(table)`: every `u`, state, internal force, halving and sub-step count | bits equal | arithmetic 4 |
| NCU | control of UB1 | the same field with a base of `alpha = 1e-5`: `max |u - u_0526| / max |u_0526|` | `>= 1e-3` | `alpha (T_1 - T_ref) = 1e-4` against the strain `1e-3` of the step: order `1e-1` |
| UB2 | a3 | the integrator: run A of V2T of ADR 0526 (the shifted rod, Rayleigh mass and stiffness damping, the table step at step 400, 800 steps) with the uniform field against the history: `u`, `v`, `a`, internal force at every step end and the state bytes | bits equal | arithmetic 4 |
| FEi | a2, a4 | the integrator at the thermal equilibrium (arithmetic 10), 50 steps: as FE1 | `<= 1e-12` | rounding |
| V3T | a2 | the integrator under a non-uniform field step (arithmetic 11), 40 steps against the restart; no halving | bits equal | arithmetic 11 |
| NC3 | control of V3T | run A against the constant `F1` | `>= 1e-2` | order `1e-1` |
| RF4 | a1, a2 | refusals, each with its key: the driver with a field and a temperature, with a field and a history (`"temperature"`), a field not covering `[0, t_last]`, a field of the wrong length (`FieldLength` `"temperature"`), a BBar block, a nodal `theta <= 0`; `TemperatureFieldHistory::new` with one point, with times not increasing, with fields of unequal length, with a non-finite value (`"temperature"`); `Integrator::new` with a field and a temperature, with the energy-momentum step; `start` outside the field; `run` beyond the field (`"temperature"`, the time of the state unchanged); `strain_energy` with a field (`"strain energy"`) | each refused with its key | D3 to D5 |
| N1 | a5 | B1: every digest of the base 679081a1 | equal | D8 |
| N5 | a3, a5 | `cargo test -p rcfd-fem --release`: every default-bits file of the crate and the tests of `sa_temp_history.rs`, `sa_qs_visco.rs`, `sa_nl_transient.rs`, `sa_emc.rs`, `sb_thermal.rs` pass unchanged | pass | D6 |
| N4 | a5 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no output path of a case changed |

Every aggregated error of a check is required finite (a NaN is a miss, b9). A check that takes more than about two minutes in
release is an ignored heavy record run once, with a fast test on the same code path.

## Planned mutations (on a scratch copy of the tip, its own target directory, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the additive Green-Lagrange thermal strain `E_e = E - a I`, `S = S_e`, `D = D_e` | FE1 (`4.95e-5`), FE2, HB1, FEi |
| MU2 | `S = S_e` (the factor `theta` dropped; the tangent left `D_e/theta`) | HB1 (`9.9e-3`), TG1 |
| MU3 | the tangent unscaled, `D = D_e` | TG1 (`~1e-2`) |
| MU4 | the point temperature and change at the element mean of the nodal values | LIN-h, LIN-t |
| MU5 | the driver's attempt at the step-start field `T(t)` | FE2 (`2.5e-3`) |
| MU6 | the integrator's sub-step at the step-start field `T(t_a)` | V3T (bits) |
| MU7 | the reference ignored (`dT_q = T_q`) | FE1 (`alpha T = 4e-2` at `T_ref + dT` against `1e-2`: `3e-2`), FE1c (`3e-2` at `t = 0`), HB1 |
| MU8 | the uniform-element shortcut removed (always `sum N_b T_b`) | UB1 or UB2 (bits) if a sum of the shape functions at a point is not exactly 1 in floating point; otherwise equivalent (recorded either way) |
| MU9 | the driver's field coverage check removed | RF4 (a panic of `value` instead of the refusal) |
| MU10 | the field length check removed | RF4 (an index panic or an acceptance) |
| MU11 | the integrator's run coverage of the field removed | RF4 |
| MU12 | the integrator's sub-step without the field (the history's context only) | V3T, FEi |

Equivalent mutation, registered so that its outcome is not read as a defect (not counted by the decision rule; run and
recorded):

| Id | Mutation | Why no registered run can catch it |
|---|---|---|
| E1 | the damping tangent `K_0` with the thermal strain (the expansion flag on) | V3T compares two runs with the same rule; FEi has `v ~ 0`, the damping force is rounding; UB2 has `alpha = 0` |

## Decision rule

- **done**: every check of the table met, MU1 to MU7 and MU9 to MU12 each caught by at least one check (MU8 recorded either
  way), fmt and clippy clean, B1, the default-bits files and the baseline hashes unchanged.
- **partial**: B1, the default-bits files and the baseline hashes unchanged, and FE1, UB1, UB2 met, with a miss of any other
  check diagnosed and recorded as a `*_known_miss` test, or a counted mutation not caught.
- **todo**: otherwise (in particular any change of the default bits).

## Limits (not covered)

- The thermal strain is isotropic (`F_th = (1 + alpha dT) I`, a constant `alpha`); no temperature-dependent expansion, no
  secant-versus-instantaneous coefficient, no orthotropic expansion. Shells, the F-bar Hex8, the energy-momentum step and the
  neo-Hookean material are refused with a field; the strain energy with a field is refused.
- The field is given, not solved: no heat equation, no coupling of the deformation into the temperature. It is piecewise linear
  in time per node and taken at the end of each increment (the reduced time of a shifted material is the right-endpoint sum of
  ADR 0526 arithmetic 2 at each point).
- The damping tangent is taken without the thermal strain (D5, E1).
- No case-file key, no output of the point temperatures.
- One mesh per check except LIN; the distortion of the elements is not exercised by FE1 (the regular cube; the exactness of
  arithmetic 5 holds on any isoparametric mesh, not checked here).

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **The thermal strain form.** Multiplicative `F_th = theta I` with `S = theta S_e` (the energy per stress-free volume), taken
   (arithmetic 1): the acceptance's "equals alpha dT" is exact for the stretch. The alternative additive Green-Lagrange form
   fails the acceptance at `a = 1e-2` by `5e-5`.
2. **The damping tangent with a field.** At the point temperatures without the thermal strain, taken (D5): the damping is a
   property of the stress-free material at its temperature and it keeps the rule of ADR 0526 D4. The alternative, the tangent
   at the thermally strained state `u = 0` (with its prestress), is the E1 mutation.
3. **The form of the input.** A separate option `temperature_field` beside `temperature` and `temperature_history` (each pair
   refused together), taken: the options of ADR 0499 and ADR 0526 and their bits stay. The model's `Load::Temperature` stays
   refused on the nonlinear paths. Alternative: take the model's `Load::Temperature` as a constant field (would change a
   refusal recorded by B1).
4. **Case-file key.** Not added (b7). Recommendation for the lead: `temperature_field = { reference, times, fields }` (or a file
   reference) in the quasi-static time study and the nonlinear transient study, when the case file is opened to the nonlinear
   studies.
