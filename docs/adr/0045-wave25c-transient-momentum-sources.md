# ADR 0045: Wave 25C, momentum sources and lumped pressure components in the transient PISO / PIMPLE algorithms (F-214, F-217, F-218)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any solver code of this wave was changed and
before any run of this wave. Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and
the assertion moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A
test set-up defect gets a NEW corrected test and the old one stays as a record. Addenda (exact numbers, deviations) may be appended
at the end without touching a registered limit.

Scope. F-214 (Lorentz force), F-217 (fan / porous-jump / loss closures, heat-exchanger macro models) and F-218 (actuator disk) are all
`partial`, and one reason in each of their roadmap notes is the same: the transient pressure-velocity algorithms have no
momentum-source hook. This wave removes that reason for `piso` and `pimple`. No item changes status unless every acceptance point of
the roadmap entry is genuinely met; none is expected to be (F-218 still lacks the rotor data comparison, F-214 the varying B and
induced field, F-217 the resolved-exchanger validation). Clean-room: first principles and published literature only; no vendor code,
text, data or names; nothing from vendor material. Other waves (24 duct/solver diagnosis, 25A fractional step and
interface code, 25B heat-exchanger effectiveness functions and waves) work in the same tree: this wave does not edit
`fractional.rs`, the SIMPLE/SIMPLEC duct boundary treatment, `hx.rs` effectiveness functions or `waves.rs`; shared files
(`sim.rs`, `schema.rs`, `case-schema.md`, `roadmap.json`) get small targeted edits committed through a temporary index.

## Audit of what exists (HEAD 78fcadd)

Where the `sources.<n>` path is applied (measured by reading, not assumed):

* `Sources::evaluate(geom, u)` (`rcfd-api/src/sources.rs`) returns an explicit kinematic `BodyForce` (cell and balanced-force face
  values) and an implicit isotropic sink [1/s]. Only the `Engine::Steady` branch of `Simulation::step` calls it, once per
  SIMPLE/SIMPLEC iteration. The SIMPLE kernel (`simple.rs`) applies the force in four places: the cell momentum RHS `sys.b += V f_c`,
  the face flux `phi_HbyA += rAU_f f_f` (balanced force), the fixed-flux wall pressure gradient `dp/dn = f_f . n` where the normal
  velocity is prescribed, and the cell velocity `U = HbyA + rAU f_c - rAU grad p`. The sink goes on `m.a.diag` before relaxation, so
  it is in `a_p` and `rAU`. `Sources::wall_pressure_trapezoid` and `frame_flux` (`conv_flux_offset`) are further SIMPLE-only
  constructions used by the rotating frame, MRF zones and the body-force wall-pressure option.
* Gravity / Boussinesq buoyancy is the existing face-consistent body force: `boussinesq_force` builds a `BodyForce` and both the
  steady kernel and `PimpleSolver` (`pimple.rs` lines 311, 389, 429, 460: fixed-flux wall pressure, `sys.b`, `gamma_p bf.face`,
  `r_au bf.cell`) use the same four places. So the transient solver already has the face-consistent explicit force; it lacks the
  implicit sink, the per-outer-iteration re-evaluation, `FaceJump` (fan, porous jump, loss closures) and the patch-pressure update
  (`network_outlet`, `fan_boundary`).
* `physics.body_force` is `base_body`, set only for the steady and coupled engines and refused for `piso`/`pimple` in `case.rs`.
  This wave does not change that rule; the constant body force of the transient checks is a `sources.<n>` `acceleration` source.
* Coupled: `CoupledSolver` has a uniform `body_force` but no sink, no jump and no re-evaluation hook; `sim.rs` already refuses
  `sources` and `components` with a located error. Compressible and variable-density: `sim.rs` builds the sources as `None`, but
  `comp.rs` / `vd.rs` / `sources::check` already diagnose them (located). `dynmesh.rs` diagnoses `sources` (not `components`).
* Steady-only gates to be replaced by a per-algorithm table: `sources::check` (`simple`, `simplec` only), `lumped::check` (same),
  `case-schema.md` lines 587 and 655, schema docs.

## Decisions

### D1 One mechanism: an outer-iteration hook plus a sink in `PimpleSolver`

The existing path is reused, not duplicated. `Sources::evaluate` and `Components::{face_jump, network_pressures, heat_sources}` are
called unchanged. `PimpleSolver` gains, all `None`/off by default so that every existing run is bit-identical:

* fields `sink: Option<Vec<f64>>` (implicit linear sink, added to `m.a.diag` before relaxation exactly as in SIMPLE; therefore
  in `a_p`, `rAU`, `HbyA`), `source_force: Option<BodyForce>` (explicit source acceleration, summed with `body_force` into the
  effective body force of the outer iteration; kept separate from `body_force` so that the energy / RSM resets of `s.body_force` in
  `sim.rs` cannot accumulate it, and `body_force` itself is never replaced by a zero `Some`), and `face_jump: Option<FaceJump>`;
* `step_with(dt, hook)` where `hook: Option<&mut dyn FnMut(&VectorField, &[f64]) -> OuterUpdate>`; `step(dt)` is `step_with(dt, None)`.
  The hook is called at the start of every outer iteration with the current iterate `u` and flux `phi`; its `OuterUpdate` replaces
  `source_force`, `sink`, `face_jump` and carries `patch_pressures` (fixed kinematic pressures applied to named pressure patches).
  Solution-dependent sources (Lorentz force with a solved potential, fan curves, network pressures, `|u|` of the Forchheimer term)
  are therefore re-evaluated every outer iteration; PISO (one outer iteration) evaluates them once per step from the old-time
  velocity (first-order lagged explicit part, implicit sink part consistent).
* Face consistency: the effective body force goes through exactly the four places listed above (fixed-flux wall pressure,
  `sys.b`, `gamma_p f_f` in `phi_HbyA`, `rAU f_c` in the cell velocity). The SIMPLE "wall pressure trapezoid" and its SIMPLEC wall
  correction are not ported: the transient solver keeps the buoyancy pattern. For a force parallel to a wall or zero at the wall
  (every check below) the constructions coincide.
* `FaceJump` (fan, porous jump, loss closure) is ported with the algebra of the SIMPLE kernel: the Green-Gauss gradient of the
  momentum predictor and of the velocity correction get `gradient_fix` of the jump totals (from the current flux), and in every
  corrector `phi_HbyA_f <- (phi_HbyA_f + G rise_f)/(1 + G resist_f)`, `gamma_f <- gamma_f/(1 + G resist_f)`, `G = gamma_f lap_f`.
  The lagged relaxation of the fan dp (`components.<n>.relax`) is applied once per hook call, i.e. per outer iteration.
* Heat-exchanger heat sources: `Components::heat_sources` is evaluated in the transient energy branch of `sim.rs` before
  `EnergySolver::advance`, with the same base-source merge rule as the steady branch (`zone_sources` merged only when components are
  present, so runs without components are unchanged). Monitors of the components are pushed per step.

### D2 Supported-algorithm table and located errors (schema-visible, `semantic_errors()` and `Simulation::new`)

| Algorithm | `sources` (porous, acceleration, actuator_disk, lorentz) | `sources` rotating_frame, mrf | `components` |
| --- | --- | --- | --- |
| simple, simplec | yes (unchanged) | yes (unchanged) | yes (unchanged) |
| piso, pimple | yes (this wave) | located error at `sources.<n>.type` | yes (this wave) |
| piso, pimple with `solver.time_advance = fractional_step` | located error at `sources.<n>.type` | same | located error at `components.<n>.type` |
| piso, pimple with `dynamic_mesh.enabled` | located error (existing at `dynamic_mesh.enabled`) | same | located error at `components.<n>.type` (new) |
| coupled | located error (existing `sources`, message now names the algorithm) | same | located error (existing `components`) |
| compressible, variable density | located error (existing) | same | located error (existing) |

The generic "available with the steady algorithms (simple, simplec) only" wording of `sources::check` and `lumped::check` is
replaced by this table; unsupported combinations keep a path of the form `sources.<n>.type` / `components.<n>.type`.
MRF and the rotating frame need the frame-flux offset and the component-diagonal correction, which the transient kernel does not
have; they are refused rather than approximated.

### D3 Superseded assertions (listed before coding; not edited quietly)

Three existing assertions test that the limitation exists. They are superseded by the goal of this wave and are replaced in the
same commit as the code, each keeping its intent (an unsupported algorithm gives the located error) and each accompanied by a
positive build test of the now-supported algorithm:

1. `tests/actuator_disk.rs`, `a3_diagnostics_are_located`, the "transient algorithm" block (`piso` gives `sources.disk.type`):
   the algorithm of the block becomes `coupled`; positive test `piso`/`pimple` build is in the new test file.
2. `tests/mhd.rs`, `a4_invalid_settings_get_located_errors`, "unsupported algorithms" loop over `["piso", "pimple"]`: the loop
   becomes `["coupled"]` (the separate `coupled` assertion below it stays).
3. `tests/lumped.rs`, `z5_invalid_settings_get_located_errors`, "transient algorithm" (`piso` gives `components.f.type`): the
   algorithm becomes `coupled`.

No limit of any other test is touched.

### D4 Scope not attempted

The coupled (monolithic) solver and the compressible solver stay unsupported (located errors). `physics.body_force` stays
steady-only. The MRF and rotating-frame sources, `dynamic_mesh`, and fractional-step time advance stay unsupported in the transient
algorithms. Joule heating with a magnetic field and the induced field remain outside the model (ADR 0039).

## Registered checks (limits fixed before any run of this wave)

Set-ups. Walls are never `symmetry` (known solver defect, wave 24). Drivers are `sources.drive.type = "acceleration"` sources.
`linear` tolerances: `momentum_rel_tol = 1e-12`, `pressure_rel_tol = 1e-12`. Transient settings stated per check; `n_correctors = 2`
(the default) unless stated.

**T1 (uniform acceleration, doubly periodic box).** `box_2d`, `periodic_x = periodic_y = true`, 8x8 cells, unit square, `rho = nu = 1`
(kinematic), initial uniform `u = (0.3, 0, 0)`, `sources.drive` acceleration `a = (2, 0, 0)`, `dt = 0.01`, 100 steps (t = 1),
for `piso` (Euler), `pimple` (n_outer = 3) and `piso` with BDF2. The exact solution is `u(t) = 0.3 + 2 t`, linear in t, which
Euler and BDF2 reproduce exactly.
 * T1a: `max |u_x - (0.3 + 2 t)| <= 1e-9` at every step, `max |u_y| <= 1e-12`, `max |p - p_0| <= 1e-9`.
 * T1b (implicit sink): add `sources.bed` porous darcy_forchheimer `permeability = 0.5` (so `s = nu/K = 2`, `inertial_resistance = 0`).
   Euler discrete recursion `u_{n+1} = (u_n + a dt)/(1 + s dt)`: agreement `<= 1e-9`. Against the analytic
   `u = a/s + (0.3 - a/s) e^{-s t}`: Euler `<= 1e-2 a/s`, BDF2 `<= 2e-3 a/s` (maximum over the steps).

**T2 (Poiseuille start-up, series).** `box_2d` periodic in x (nx = 4, ny = 40), unit square, no-slip plates at `ymin`/`ymax`,
`nu = rho = 1`, rest initial, acceleration `G = 8` (steady centre value `G h^2/(8 nu) = 1`), `dt = 1e-3`, `piso` (Euler), `pimple`
(n_outer = 2), `piso` BDF2. Reference: `u(y,t) = G y(h-y)/(2 nu) - (4 G h^2/(nu pi^3)) sum_{n odd} sin(n pi y/h) exp(-n^2 pi^2 nu t/h^2)/n^3`
(derivation in `docs/reference-data.md`; 199 terms).
 * T2a: at `t = 0.02, 0.05, 0.1, 0.5`: `max_c |u_c - u_series(y_c, t)| / 1 <= 1e-2` (Euler, pimple), `<= 1e-2` (BDF2).
 * T2b: at `t = 1.5`: `max_c |u_c - G y_c (h - y_c)/(2 nu)| <= 2e-3` (the steady parabola; the estimated discrete wall-cell
   error is about 3e-4).

**T3 (transient Lorentz force on Hartmann flow).** The A1 geometry and parameters of `tests/mhd.rs` (periodic box_2d nx = 4, plates at
y = 0, 1, `a = 0.5`, `sigma = rho = nu = 1`, `B = (0, 2 Ha, 0)`, `G = 400` acting as `sources.drive` acceleration instead of
`physics.body_force`), `(Ha, ny) in {(2, 40), (10, 80), (20, 160)}`, rest initial, `piso` and `pimple` (n_outer = 3, `alpha_u = 0.9`,
`alpha_p = 0.5`), `dt = 0.02`, `t_end = 2`.
 * T3a (stationarity, registered "long time"): `max |u(t_end) - u(t_end - dt)| / u_max <= 1e-8`.
 * T3b (A1 limits in the transient algorithm): profile `max |u - u_Hartmann|/u_max <= 2e-2`, flow rate `|U/U_exact - 1| <= 1e-2`.
 * T3c (against the steady result): for `Ha = 10` the steady SIMPLEC run of the identical case (A1 settings) has
   `max |u_transient - u_SIMPLEC| / u_max <= 1e-6`.
 * T3d (solved potential, A5 analogue): `B = (0, 0, 2 Ha)` with `potential = "solve"`, `Ha = 10`, `ny = 80`, `pimple`: compared with the
   same transient run without the source at `t = 0.1` and `t = 1.0`: `max |u - u_plain| / u_max <= 1e-6` and
   `max |J| / (sigma B u_max) <= 1e-6`.

**T4 (transient actuator disk, A4b set-up).** The A4b duct of `tests/actuator_disk.rs` (channel_2d 60x10, 2 m x 0.2 m x 0.05 m,
`nu = 1e-3`, `rho = 1.2`, inlet 1 m/s, outlet pressure, side walls `fixed (1,0,0)`, upwind, disk box x in [0.9, 1.1],
thrust 0.6 N, smooth distribution), `piso` and `pimple` (n_outer = 3), `dt = 0.01`, 600 steps.
 Limits are those of A4b: kinematic `dp = 50 +- 0.5`, `max |ux - 1| <= 1e-3`, `max |uy| <= 1e-3`, plus stationarity
 `max |p(t) - p(t - dt)| / 50 <= 1e-6` over the last step.

**T5 (momentum balance, impulse = integral of F dt).** Discrete identity of the summed x-momentum equation (periodic faces and interior
faces telescope; pressure uniform in these 1-D flows): for each step `sum_c V (u^{n+1} - u^n) = dt [ sum_c V F^{n+1} - tau^{n+1} ]`,
`tau` the wall shear force `sum_wall nu u_c/(dy/2) A` of the new level, `F` the force actually applied (the acceleration; for the
Lorentz case `-s u_x^{n+1}`, the implicit damping that the solver applies, with `u` perpendicular to `B`). Accumulated over the run:
`| sum_c V u(T) - sum_c V u(0) - sum_n dt (sum V F^{n+1} - tau^{n+1}) | / (sum_n dt sum V |F|) <= 1e-8`
 * T5a: T2 set-up (Euler, `piso`) over 0.1 s; T5b: T3 set-up `Ha = 10` (Euler, `piso`) over 0.2 s; T5c: T1b set-up (porous sink,
   doubly periodic, no walls) over 1 s with `<= 1e-9`.

**T6 (located errors and positive builds).** For each row of the D2 table marked "located error" a test asserts that
`semantic_errors()` or `Simulation::new` reports a diagnostic whose path is `sources.<n>.type` / `components.<n>.type` (or the
existing path of the row). Positive: every supported source type builds and runs one step under `piso` and `pimple`.

**T7 (components in the transient algorithm, registered limits of the steady checks).** Cases are those of `tests/lumped.rs` and
`tests/heat_exchanger.rs` with `solver.algorithm = "pimple"` (n_outer = 3), `dt = 0.01`, run until stationary
(`max` relative change of ux and p over the last 200 steps `< 1e-6`, cap 4000 steps), the registered stationarity criterion
of the transient runs. The side walls of those cases are as registered (`symmetry`; a miss attributable to the symmetry defect is
reported as such, not tuned).
 * T7a (Z1): porous-jump sheet `dp` against Darcy-Forchheimer, worst relative error `<= 1e-3`.
 * T7b (Z2): fan + porous jump operating point: flow `<= 5e-3`, fan rise `<= 1e-3`, jump drop `<= 1e-3` of analytic.
 * T7c (X2): loss closure `dp` (`loss_coefficient = 5`, `loss_linear = 2`) `<= 1e-3` of prescribed (needs the transient heat-source
   path; fixed duty 0).
 * T7d (X4, distributed-bed set-up): fan_boundary flow `<= 5e-3` of the analytic operating point and patch pressure on the curve
   `<= 2e-3`.
 * T7e (NTU exchanger, transient vs steady): the reported `hx_duty` of the NTU counterflow case (`ua = 40`, secondary 350 K,
   capacity 20 W/K) at stationarity equals the duty of the steady SIMPLEC run within `1e-4` relative.

**T8 (steady results unchanged).** (i) `tests/baseline_hashes.rs`: 3 passed / 1 ignored identically before and after, hashes not
re-recorded. (ii) Bit-identity beyond the baseline file: a scratch test (not committed) prints the IEEE-754 bit hash of `ux, uy, p`
after 150 iterations of steady SIMPLEC cases with a porous source, an acceleration, an actuator disk, a Lorentz source, a fan plus
porous jump and an NTU heat exchanger with loss closure; the hashes from a `git archive` of HEAD 78fcadd and from the final tree
must be equal bit for bit. (iii) the existing test files `sources.rs`, `actuator_disk.rs`, `mrf.rs`, `component_diagonal.rs`,
`lumped.rs`, `heat_exchanger.rs`, `mhd.rs` pass with the three assertions of D3 as the only edited lines.

Baseline on HEAD before any change: see the Addendum (run with `CARGO_TARGET_DIR=<target dir>`).

## Acceptance bookkeeping

The roadmap notes of F-214 (transient Lorentz), F-218 (transient actuator disk) and F-217 (transient fan / porous jump / loss
closure / exchanger) are updated only with what the measured addendum supports. Statuses stay `partial`.

## Addendum A: baseline before any change

`cargo test --release -p rcfd-api --test baseline_hashes` on a `git archive` of HEAD 78fcadd: 3 passed, 0 failed, 1 ignored
(`rotating_channel_matches_pre_adr0005_hash`, known). Recorded before any solver edit of this wave.
