# ADR 0037: Wave 21C, particle and chemistry wiring into `rcfd run` and the service (F-056, F-054), non-iterative fractional-step time advance (F-210), F-209 remainder

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave existed.
Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion moves to an
ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up defect gets a
NEW corrected test and the old one stays as a record. Addenda (file names, mesh sizes not fixed here, measured numbers) may be
appended at the end without touching a registered limit.

Scope. F-056 and F-054 are `partial`; F-210 is `todo`. This wave stops each of them at `partial` with named gaps (end of file).
F-209 (adaptive stepping, accelerators) is picked up only if it is cheap; the decision is recorded at the end (D6). Clean-room:
published methods and first principles only; no vendor code, text, data or names. Every registered expectation below is derived
from definitions, closed forms or invariants, or is an equivalence between two code paths of this repository, or reuses a
published-data acceptance band that already exists in the repository (Ghia et al. cavity data, band 0.02, F-028).

Baseline hashes (`crates/rcfd-api/tests/baseline_hashes.rs`) stay bit-identical and are not re-recorded. All additions are
additive schema keys with explicit unset/off sentinels; every new kernel is behind an option that is off by default.

Concurrency. Waves 21A (ADR 0035) and 21B (ADR 0036) edit the same shared files (`schema.rs`, `sim.rs`, `case.rs`, `lib.rs`,
docs). Edits to shared files are small and targeted, re-read immediately before editing, committed path-limited.

## Decisions

### D1 Particles in the run loop (F-056)

New module `rcfd-api/src/particle_run.rs` with `ParticleRun` (tracker, step counter, optional track log, optional output
directory). `Simulation` gets one field `particles: Option<ParticleRun>`, `None` unless `particles.enabled`.

* Construction in `Simulation::new` (end): `build_tracker` on the simulation mesh, so every mesh-dependent located error of the
  library (unknown patch, point outside the mesh, periodic patch) is now reported by `Simulation::new`, `rcfd run --check-only`
  and `rcfd validate`, not only when a hook is called.
* Transient algorithms (`piso`, `pimple`): the hook is the last statement of `Simulation::step()` for `Engine::Transient`: it
  takes the flow step `dt = time_after - time_before`, `h = min(dt, end - tracker.time)`, builds the `FlowData` of the current
  field and calls `Tracker::advance(&flow, h)` while `tracker.time < end (1 - 1e-12)`. Arithmetic and call order are those of the
  library hook `track_transient`, so the two agree bit for bit (check W1). Because the hook sits in `step()`, every driver
  (`runctl::drive`, hence `rcfd run` and the service `run.headless`/`workflow`, and the interactive service worker of
  `run.start`/`run.step`) advances the particles. `track_transient` itself is unchanged (an independent implementation) and
  detaches the built-in hook of the simulation it is given (`Simulation::detach_particles`) so that it does not track twice.
* Steady algorithms (`simple`, `simplec`, `coupled`, frozen flow): the flow is not time resolved, so the particles are tracked on
  the final field once, at result-writing time (`runctl::write_results`), exactly as `track_frozen` does (W2). This is a
  documented choice: injection times are tracking times, not iteration counts.
* Not supported, each a located error at `particles.enabled` in `Simulation::new`: `dynamic_mesh.enabled` (the mesh moves under
  the tracker), the density-based and variable-density solvers (already rejected by `particles::check`). Restart/checkpoint:
  `supports_restart()` is false while particles are enabled (the tracker state is not saved), so the existing run-control warning
  is emitted instead of writing a checkpoint that would silently drop the particles.
* The hook never writes to the flow. Reading only `velocity()`, the geometry and `case` constants.

Outputs of the run path, written into the run's output directory by `runctl::write_results` (and, for `output_every`,
during the run when the driver has an output directory, set by `drive` through `Simulation::set_output_dir`):

* `particles.vtp`: final state (`write_vtp`, same bytes as the library function writes for the same tracker);
  `particles_<n>.vtp` every `particles.output_every` tracking steps (0 = only the final file).
* `particle_report.json`: `{time, injected, counts {active, deposited, escaped, stuck}, mean_diameter, mass_by_state, steps}`
  (mass = sum of parcel count x density x pi d^3 / 6).
* `particle_tracks.csv` (only with the new key `particles.tracks = true`): rows `id,time,x,y,z,state` for every parcel at
  injection and after every `particles.track_every`-th tracking step in which it was active, plus the step at which it leaves.
* `particles.output_file` keeps its library meaning (path of the library hooks `track_frozen`/`track_transient`, relative to the
  working directory); the run path does not write it. Documented in `docs/case-schema.md` and `docs/particles.md`.

New keys (all inert when `particles.enabled = false`): `particles.tracks` (bool, default false), `particles.track_every` (int,
default 1, at least 1; located error `particles.track_every`).

Service: a new session method `particles.report` (optional `with_tracks`) returns the report JSON of the live simulation (the
tracks as arrays when asked). It is added to `METHODS`; no MCP tool is added (the MCP catalogue test pins the tool list).

### D2 Chemistry in the run loop (F-054)

Reading of the code before the wave: `Simulation::step` already calls `chemistry::apply_split` (transient, after the scalar
sweeps) and `chemistry::apply_steady` (steady, before the scalar solve), and the species are user scalars that `export_vtu`
writes. So the library IS driven by the loop and by `rcfd run`; the roadmap note "not driven" is true only for particles. What is
missing and is delivered here: (a) proof through the real `rcfd` binary and the service, (b) chemistry outputs of their own.

* `chemistry.json` in the output directory whenever `chemistry.model != off`: model, mechanism name counts (species, reactions,
  elements), per transported species `{min, max, volume_mean}`, mass-fraction sum range over the cells, temperature range (with
  energy), and the element mass totals (volume integral of rho times the element mass fractions computed from the species
  compositions) so conservation can be audited from the run output.
* Opt-in cell arrays (`chemistry.write_rates = false` default) in `solution.vtu`: `wdot_<species>` (net molar production rate
  from `cell_rates`, units of the mechanism reader, documented) and, when `chemistry.heat_release`, `heat_release_rate`
  [K/s] = the temperature source of the energy balance, evaluated on the final fields.
* New key: `chemistry.write_rates` (bool, default false). No other schema change; no change to a numerical path.

### D3 Fractional-step time advance (F-210)

Option, not a new algorithm: `solver.time_advance` = `iterative` (default, today's PISO/PIMPLE loop) | `fractional_step`. With
`fractional_step` the `piso` algorithm advances by one non-iterative projection step; `pimple` with `fractional_step` is a located
error (`solver.time_advance`: outer iterations contradict a non-iterative step). Further keys: `solver.fractional_form` =
`incremental` (default) | `chorin`; `solver.fractional_boundary` = `kim_moin` (default) | `plain`. Library: `PimpleSettings`
gets `fractional: Option<FractionalSettings>` (`None` default), implemented in the child module `pimple/fractional.rs`;
`PimpleSolver::step` dispatches to it when set and no moving mesh is active. Collocated variables, constant viscosity (or the
face viscosity array `nu_face` if set), the existing convection schemes, `ImplicitEuler` and `Bdf2` time schemes with the same
history objects as PISO, so `snapshot/restore` and the tree restart of laminar PISO keep working.

Per step (constants `c0, c1, c2` of the time scheme, `dt`, velocity `u^n`, flux `phi^n`, pressure `p^n`, previous increment `q_prev`):

1. Predictor, per component, implicit in `u*`: `(c0 u* - c1 u^n + c2 u^{n-1})/dt + conv(phi_conv, u*) - lap(nu, u*) =
   -grad(p^n)` (incremental form) or `= 0` (Chorin form, no pressure in the predictor). `phi_conv = phi^n`, extrapolated in time
   on the first step of BDF2 as the PISO code does. Boundary values of `u*` on faces where the velocity is prescribed:
   `u_b + s dt (g - (g . n) n)` with `g` the owner-cell least-squares gradient of `q_prev` (incremental: the previous pressure
   increment; Chorin: `p^n`) and `s = 1` for `fractional_boundary = kim_moin`, `s = 0` for `plain`. This is the Kim and Moin
   (1985) idea of replacing the physical velocity condition of the intermediate field by `u^{n+1}_b + dt grad(phi)_b`, with the
   unknown `phi` lagged by one step and only its tangential part (its normal part is zero by the Neumann condition below).
2. Face flux without pressure: `phi*_f = (interp(u*) . S)_f + k dt (interp(grad p^n) . S - snGrad(p^n) |S|)_f` (`k = 1`
   incremental, `k = 0` Chorin), the collocated-grid fractional-step form (Zang, Street and Koseff 1994: replace the
   cell-gradient pressure force carried by the interpolated velocity with the compact face gradient). Prescribed-flux boundary
   faces carry `u_b . S`. No transient Rhie-Chow (Choi) term: with `rAU = dt` Choi's correction degenerates and no fixed point
   exists (derived in the implementation notes below); the price is a weak dt dependence of the steady state through the
   pressure smoothing term, which check F8 measures.
3. One Poisson solve (existing `solve_pressure`, uniform diffusivity `dt`): `lap_h(q) = div(phi*)/dt` times `dt`, i.e.
   `div(dt grad q) = div(phi*)`. Incremental form: `q` is the increment, boundary conditions homogenised (Dirichlet 0 where the
   pressure is fixed, Neumann 0 where the normal velocity is prescribed). Chorin form: `q = p^{n+1}` with the real pressure
   conditions. A pure-Neumann problem is pinned by the existing `p_ref` mechanism.
4. Projection: `phi^{n+1} = phi* - dt snGrad(q) |S|` (existing `corrected_flux`), `u^{n+1} = u* - dt grad(q)` (cell
   Green-Gauss gradient with the same boundary values), `p^{n+1} = p^n + q` (incremental) or `q` (Chorin), `q_prev = q`.
   The Poisson problem and the flux correction are the same discrete operators, so `div(phi^{n+1})` equals the linear-solver
   residual of the pressure equation (check F4).

Not delivered in D3 (named gaps): rotational (Timmermans) pressure correction, pressure extrapolation, Crank-Nicolson
momentum, Choi-type transient Rhie-Chow, body force/buoyancy (a located error when `energy.enabled`, `physics.body_force`,
gravity-driven terms), turbulence and rheology (located errors), moving meshes (located error), `pimple` outer loops.

Tree restrictions, each a located error at `solver.time_advance` (checked in `Simulation::new`): algorithm not `piso`; any of
`turbulence.model != laminar`, `energy.enabled`, `rheology`, non-zero `physics.body_force`, `dynamic_mesh.enabled`,
`solver.viscous_stress = full`, compressible or variable-density fluid. User scalars, species, chemistry and particles run
unchanged on top of `phi` and `u`.

### D4 Documentation and status

`docs/case-schema.md` (particles/chemistry additions, solver keys), `docs/particles.md` (run-loop section replaces the
"not called" statement), `docs/reference-data.md` (references with verified/unverified marks), `docs/roadmap.json`: F-056,
F-054 stay `partial`; F-210 becomes `partial` (not `done`) with the unmet parts named; F-209 unchanged unless D6 applies.

### D5 Shared-file discipline

New files: `particle_run.rs`, `chem_output.rs`, `rcfd-fv/src/pimple/fractional.rs`, tests. Shared files get small edits:
`sim.rs` (field, hook, accessors, construction/validation, fractional settings), `schema.rs`/`particles.rs`/`chemistry.rs`
(keys), `runctl.rs` (`write_results`, `set_output_dir`), `service.rs` (one method), `lib.rs` (modules), `case.rs` only if needed.

### D6 F-209 remainder

Not attempted unless the other items finish with margin; the decision is recorded in the results addendum. The wired item that
exists (`solver.max_courant` Courant-controlled step) is not claimed as F-209 progress.

## Derivations and constants used by the checks

* Taylor-Green vortex on the doubly periodic square `[0, 2 pi]^2`: `u = sin x cos y e^{-2 nu t}`, `v = -cos x sin y e^{-2 nu t}`,
  `p = (cos 2x + cos 2y)/4 e^{-4 nu t}`; mean kinetic energy per unit mass `1/4 e^{-4 nu t}`.
* Observed order from three step sizes `dt, dt/2, dt/4` against a reference with step `dt/32` of the SAME method: errors
  `e1 = |u_dt - u_ref|`, `e2 = |u_{dt/2} - u_ref|`; order `log2(e1/e2)`. The reference error is `(dt/32)^p` relative to
  `dt^p`, i.e. at most 3 percent of `e1`'s scale, so the estimate carries at most ~0.1 error in `p`; the bands account for it.
* Fixed-point argument for the Choi term (why it is omitted): with `rAU = dt` the face flux of the corrected predictor would be
  `phi* = interp(u*).S + (phi^n - interp(u^n).S) - ...`; at a fixed point `u* = u^n` cell-wise and `phi*` must equal `phi^n`,
  which would require `interp(grad p).S = snGrad(p)|S|` on every face, not true in general; so such a scheme has no steady
  state. The scheme above (no Choi term) has the fixed point `phi = interp(u).S + dt (interp(grad p).S - snGrad(p)|S|)`.
* Divergence measure: `continuity` of `StepInfo` is `max|div phi| / max|S|`; with a relative pressure tolerance `r` the
  discrete divergence after the correction is at most `r` times the initial Poisson residual, hence the limits below scale with
  the tolerance.

## Registered checks

All "bit" checks compare `f64` bit patterns (or `Tracker::fingerprint`) and have limit 0 differing bits. Case geometries are
fixed in the tests (sizes not fixed here are recorded in the addendum).

### Wiring: particles (W)

* **W1 transient equivalence.** A `piso` case with a flow (lid-driven cavity 20 x 20, `nu` 0.01, 40 steps of `dt` 0.01) and a
  point injection. Run A: `runctl::drive` + `write_results` into a temp directory (the run path). Run B: a second `Simulation`
  from the same text, `particles::track_transient` with `particles.output_file` set. Limits: tracker fingerprint of A and B
  equal (0 bits); `particles.vtp` of A is byte-identical to the file of B.
* **W2 steady equivalence.** Converged `simple` cavity (same mesh) with a surface or point injection; run path vs
  `track_frozen`: fingerprints equal (0 bits), `particles.vtp` byte-identical.
* **W3 flow untouched.** The cavity of W1 with `particles.enabled = true` vs `false`: `ux, uy, p` and the face flux equal in all
  bits after 40 steps (and after 60 SIMPLE iterations for W2's case). `baseline_hashes.rs` passes unchanged.
* **W4 defaults inert.** With `particles.enabled = false` and `chemistry.model = off` the output directory of a run holds
  exactly `monitors.csv, monitors.json, report.html, solution.vtu` (no particle or chemistry file) and the vtu array name list is
  unchanged by `chemistry.write_rates`.
* **W5 located errors** (path asserted, message non-empty): `particles.enabled` with `dynamic_mesh.enabled`; an injection point
  outside the mesh (`particles.injection.<n>.position`, now raised by `Simulation::new` and by `--check-only`); unknown
  surface patch (`particles.injection.<n>.patch`); unknown `particles.walls.<n>`; `particles.track_every = 0`
  (`particles.track_every`). `rcfd run --check-only` exits 3 for each.
* **W6 outputs exact.** In `particle_report.json`: `active + deposited + escaped + stuck = injected` (exact) and `injected`
  equals `floor(rate (t_inj_end - t_start))` of the registered injection rule; with `particles.tracks`, every parcel has at
  least one row, the first row time equals the parcel's injection step end and the last row position equals the final position
  in `particles.vtp` (bits of the printed shortest-round-trip numbers, 0 differences), row count equals the sum over parcels of
  recorded steps (exact).
* **W7 real binary and service.** `rcfd run <case.toml> --out <dir>` (the built binary, as in the existing CLI tests) exits 0
  and its `particles.vtp` is byte-identical to the in-process run path of W1; the service `run.headless` with `out_dir` writes
  the same bytes; the interactive path (`run.start` to completion, then `particles.report`) gives the same `counts` and `time`
  as W1's report.
* **W8 checkpoint honesty.** With particles enabled `Simulation::supports_restart()` is false and `drive` with a checkpoint path
  emits the existing "cannot be restarted" warning event.

### Wiring: chemistry (C)

* **C1 binary vs library vs cell integral.** The closed-box case of ADR 0027 K2b (4 x 4 `box_2d`, `piso`, `dt` 1e-5,
  `energy.enabled`, heat release, isomerisation or the K2b mechanism as in `tests/chemistry.rs`) run with the built `rcfd run`:
  species and `T` read back from `solution.vtu` equal the in-process `Simulation` fields to `1e-9 DeltaT_ad` (temperature) and
  `1e-9` (mass fractions) [the K2b cell-spread limit]; and equal the single-cell integration of the toolkit to `1e-5 DeltaT_ad`
  [the K2b equality limit].
* **C2 steady through the binary.** The K4 plug-flow decay case (steady, `nx` as in `tests/chemistry.rs`) run with `rcfd run`:
  outlet species profile equals the in-process `Simulation` to `1e-9` relative to the inlet value and the discrete solution of
  K4 to its registered `5e-6`.
* **C3 report.** `chemistry.json` exists and is valid JSON; `species` count equals the mechanism; the mass-fraction sum range
  satisfies `|sum - 1| <= 1e-12` in the closed-box case; element mass totals change by at most `1e-12` relative between the first
  and last step (K3 invariant, `3e-14` registered there) (the report is produced at the end, the initial totals come from the
  initial fields in the test).
* **C4 rate arrays.** With `chemistry.write_rates = true` the arrays `wdot_<species>` equal `cell_rates` evaluated on the
  final fields (relative `1e-12`) and, with heat release, `heat_release_rate` equals `-sum h_k wdot_k/(rho cp)` of the same call
  (relative `1e-12`); with the key false no such array exists (W4).
* **C5 service.** `run.headless` with `out_dir` for the C1 case writes `chemistry.json` identical to the CLI's.

### Fractional step (F)

* **F1 Taylor-Green decay (library).** `n = 32`, central convection, `nu = 0.05`, `dt = 0.02`, `t = 2` (the PISO test set-up),
  exact initial `u` and `p`, form incremental, `kim_moin`, `ImplicitEuler`: relative error of the kinetic energy against
  `e^{-4 nu t}` at most 0.02 (the roadmap acceptance, 2 percent); relative L2 error of the velocity field against the closed
  form at most 0.02.
* **F2 temporal order.** `n = 32`, `nu = 0.1`, `t = 1`, central, exact initial data; reference `dt/32` of the same method
  and form with `dt = 0.1, 0.05`: observed order in `[0.8, 1.3]` for `ImplicitEuler`, `[1.7, 2.3]` for `Bdf2`, both measured with
  the incremental form; the L2 difference to the reference is used.
* **F3 spatial order.** `dt = 0.005`, `nu = 0.1`, `t = 0.5`, `Bdf2`: error against the closed form at `n = 16` over `n = 32` at
  least 3.0 (the existing PISO criterion).
* **F4 divergence free to solver tolerance.** Over every step of F1 and of the cavity of F6: `max continuity <= 1e-6` with the
  default linear controls (relative tolerance 1e-7); with `pressure_solver.tol_rel = 1e-12`, `tol_abs = 1e-16` the maximum is at
  most `1e-9`.
* **F5 agreement with PISO.** The F1 mesh and data, `nu = 0.05`, `dt = 0.01`, `t = 1`, central, `ImplicitEuler`, both forms:
  the relative L2 difference of the velocity fields between the fractional-step solver and the PISO solver (2 correctors) is at
  most `5e-3`.
* **F6 lid-driven cavity steady state.** Re = 100, 40 x 40 `cavity_2d`, `linear_upwind`, `dt = 0.05`, run until
  `max |u^{n+1} - u^n| / dt < 1e-7` or `t = 200`: (a) it reaches the stop rule; (b) centreline `u` and `v` against the Ghia et al.
  data of `cases/cavity/reference` (verified in `docs/reference-data.md`): maximum error at most 0.02 of the lid speed (the F-028
  band); (c) maximum centreline difference to the steady SIMPLEC solution of the same mesh and scheme at most 0.01.
* **F7 step sensitivity.** F6 with `dt = 0.05` and `dt = 0.025`: maximum centreline difference at most 0.005.
* **F8 Kim-Moin effect (Chorin form).** The F6 cavity with `fractional_form = chorin` at `dt = 0.05` and `0.025`: the
  centreline difference between the two step sizes with `kim_moin` is at most 0.5 times that with `plain`.
* **F9 Chorin form decays.** Chorin form, F1 set-up with `dt = 0.01`: kinetic-energy relative error at most 0.02.
* **F10 momentum.** F1: the mean velocity stays below `1e-8` in magnitude.
* **F11 tree equivalence.** F1 through the case tree (`box_2d` periodic, initial `velocity_expression`, `solver.time_advance =
  fractional_step`): `u` and `p` equal the library run built from the same data to `1e-12` relative (bits expected equal; the
  limit is 1e-12 to allow a different but equivalent initial-field evaluation); the tree run through `rcfd run` writes a
  `solution.vtu`.
* **F12 located errors.** `solver.time_advance` is the path for: `pimple`, steady algorithms, `energy.enabled`, a turbulence
  model, `rheology`, `physics.body_force`, `dynamic_mesh.enabled`; an unknown enum value is a schema error.
* **F13 defaults unchanged.** `baseline_hashes.rs` passes unchanged (not re-recorded); a `piso` run with
  `time_advance = "iterative"` explicitly set is bit-identical to one without the key.

## Not delivered / named gaps (registered before the work)

Cyclone separation case, droplet/combusting/two-way particles, tracking through unjoined interfaces, pollutant models,
compressible chemistry, tabulation, published flame comparison (the F-054 acceptance), thickened-flame model, rotational
pressure correction, Crank-Nicolson, variable-density fractional step, F-209 items (error-estimator stepping, residual smoothing,
Aitken acceleration, flat-plate acceleration, Taylor-Green step-count comparison).

## References

| Quantity | Source | Status (set after the web check, see `docs/reference-data.md`) |
| --- | --- | --- |
| Projection method | Chorin (1968), Math. Comp. 22, 745-762 | verified citation (AMS record); the paper itself was not read |
| Fractional step with intermediate-velocity boundary conditions | Kim and Moin (1985), J. Comput. Phys. 59, 308-323 | abstract and NASA TM-85898 precursor found; volume and pages not confirmed against a publisher record; the precise lagged-increment form used here is the author's reading |
| Incremental pressure-correction, splitting error order of the non-incremental scheme | Guermond, Minev and Shen (2006), CMAME 195, 6011-6045; Guermond and Quartapelle (1997) | verified citation and abstract; theorems not re-derived |
| Collocated-grid fractional step | Zang, Street and Koseff (1994), J. Comput. Phys. 114, 18-33 | UNVERIFIED, from memory |
| Taylor-Green closed form | docs/reference-data.md | verified (earlier wave) |
| Ghia et al. cavity data | docs/reference-data.md | verified, two secondary copies |

## Results addendum (written after the runs; no registered limit was changed)

Measured values are from release builds on the final code; tests are in `crates/rcfd-fv/tests/fractional_step.rs` (F1-F10),
`crates/rcfd-api/tests/fractional_tree.rs` (F11-F13), `crates/rcfd-api/tests/particle_run.rs`, `crates/rcfd-cli/tests/particle_run.rs`
(W1-W8, F11 binary part) and `crates/rcfd-cli/tests/chemistry_run.rs` (C1-C5).

### Particles (W)

| Check | Measured | Limit | Result |
| --- | --- | --- | --- |
| W1 transient run path vs `track_transient` (cavity 20 x 20, 40 steps, 10 parcels, `output_every` 7) | fingerprint (132 words), `particles.vtp` and all 5 numbered files equal; flow bits equal | 0 bits | met |
| W2 steady run path vs `track_frozen` (60 SIMPLE iterations) | fingerprint and `particles.vtp` equal | 0 bits | met |
| W3 flow untouched, piso and simple | `U`, `p`, face flux bits equal with particles on and off; `baseline_hashes.rs` passes unchanged | 0 bits | met |
| W4 defaults inert | default file list exactly `monitors.csv, monitors.json, report.html, solution.vtu`; `chemistry.write_rates = true` without chemistry changes nothing | exact | met |
| W5 located errors | all paths found, non-empty messages, `rcfd run --check-only` exit 3 for four of them (the fifth, dynamic mesh, see deviation 1) | path + exit 3 | met (with deviation 1) |
| W6 outputs exact | `active + deposited + escaped + stuck = injected = 10 = floor(50 x 0.2)`; every parcel has a first row at its injection step end (within 1e-9), the last row equals the final position bit for bit, the row count equals 1 + every 3rd later step + closing row for each parcel | exact | met |
| W7 binary and service | `rcfd run` bytes of `particles.vtp`, `particle_report.json`, `particle_tracks.csv` equal the in-process run path; `run.headless` writes the same `particles.vtp`; `run.start` then `particles.report` equals the run path report | exact | met |
| W8 checkpoint honesty | `supports_restart()` false with particles (transient), true without; `drive` with a checkpoint path emits the "cannot be restarted" warning and writes no file; the binary writes no `restart.rcfd` | exact | met |

### Chemistry (C)

| Check | Measured | Limit | Result |
| --- | --- | --- | --- |
| C1 K2b through `rcfd run` | binary vs in-process: T 0, Y 0 (bits equal); binary vs single-cell integration 1.8e-15 DeltaT_ad; T_end 2800 K | 1e-9 DeltaT_ad, 1e-9, 1e-5 DeltaT_ad | met |
| C2 K4 plug flow through `rcfd run` | binary vs in-process 0 (bits equal); discrete-solution deviation 5.248e-6 | 1e-9; 5e-6 as printed in this ADR | binary vs in-process met; the printed 5e-6 MISSED (known miss, deviation 2) |
| C3 report | `abs(1 - sum Y)` 1.1e-16; element moles change 0 | 1e-12; 1e-12 | met |
| C4 rate arrays | `wdot_A`, `wdot_B`, `heat_release_rate` relative difference to `cell_rates` / the enthalpy formula 0; absent without the key | 1e-12 | met (caveat: same code path as the reference function, so this checks the output plumbing and the units, not the rates) |
| C5 service | `chemistry.json` bytes equal the CLI file (490 bytes) | identical | met |

### Fractional step (F)

| Check | Measured | Limit | Result |
| --- | --- | --- | --- |
| F1 Taylor-Green KE error / L2 error (n 32, nu 0.05, dt 0.02, t 2) | 2.303e-3 / 1.485e-3 | 0.02 / 0.02 | met |
| F2 temporal order, Euler | 1.034 | [0.8, 1.3] | met |
| F2 temporal order, BDF2 | 1.040 | [1.7, 2.3] | MISSED, `f2_known_miss_temporal_order_bdf2` (ignored) |
| F3 spatial ratio e16/e32 (dt 0.005) | 2.479 | >= 3.0 | MISSED, `f3_known_miss_spatial_order` (ignored) |
| F4 continuity, Taylor-Green default / tight; cavity default / tight | 1.831e-11 / 2.753e-15; 5.776e-9 / 4.490e-14 | 1e-6 / 1e-9 | met |
| F5 difference to PISO (incremental / Chorin) | 3.016e-3 / 3.970e-3 | 5e-3 | met |
| F6 cavity Re 100, 40 x 40, dt 0.05 | stop rule reached (540 steps); Ghia error u 3.0027e-3, v 8.9796e-3; difference to SIMPLEC 4.7294e-4 | 0.02; 0.01 | met |
| F7 step sensitivity (0.05 vs 0.025) | 2.4513e-4 | 5e-3 | met |
| F8 Kim-Moin effect (Chorin form) | ratio km/plain 1.130 (1.1671e-2 vs 1.0331e-2) | <= 0.5 | MISSED, `f8_known_miss_kim_moin_effect` (ignored) |
| F9 Chorin Taylor-Green KE error | 1.388e-2 | 0.02 | met |
| F10 mean velocity | 4.1e-11 | 1e-8 | met |
| F11 tree vs library | bits equal (worst relative difference 0); `rcfd run` writes `solution.vtu`, no checkpoint | 1e-12 | met |
| F12 located errors | all paths found (`physics.body_force` is refused at parse by the existing `physics.body_force` error for `piso`) | path | met |
| F13 defaults | explicit `iterative` bit-identical to the default; `baseline_hashes.rs` unchanged | 0 bits | met |

Diagnoses of the three F misses (none tuned):

* F2 BDF2 (1.040 against [1.7, 2.3]). Not a defect of the projection. The baseline PISO solver with periodic faces and one
  corrector shows the same first-order behaviour (orders 1.03 and 1.09 for BDF2 with `n_outer = 1`; 2.17 and 1.75 only with three
  outer iterations), a pure periodic shear-diffusion problem gives about 1.08 for both solvers, and a wall-bounded decaying mode
  gives BDF2 orders 2.074 / 2.045 with the fractional step identical to PISO. This points at the lagged coupling of periodic faces
  in the shared momentum assembly (a hypothesis consistent with these runs, not proven by an isolating change). The collocated
  pressure-smoothing term `dt (interp grad p - snGrad p)` adds an O(dt) steady/inviscid error (with the term removed at
  `nu = 1e-6` the order is 1.98). A first implementation used `dt` instead of the effective step `dt/c0` for the pressure
  (c0 = 3/2 for BDF2); that bug was found and fixed during the diagnosis (first run: F2 BDF2 1.052, F3 ratio 2.502, Euler results
  unchanged); the numbers in the table are after the fix.
* F3 (2.479 against 3.0). At `dt = 0.005` the temporal error from the periodic-face lag is comparable to the spatial error.
* F8 (ratio 1.130 against 0.5). The Chorin form has an O(dt) steady-state error from the bulk (the momentum operator is applied to
  `u* = u + dt grad p`), which the Kim-Moin boundary treatment does not remove; error to SIMPLEC at dt 0.1 / 0.05 / 0.025:
  Kim-Moin 4.351e-2 / 2.741e-2 / 1.574e-2, plain 3.863e-2 / 2.452e-2 / 1.438e-2. The incremental form has 8.07e-4 / 4.73e-4 /
  2.49e-4 and is identical with and without Kim-Moin, because the lagged increment tends to zero at steady state. So in this
  implementation the Kim-Moin condition is a no-op where it is harmless and does not beat the plain condition where it matters;
  the claimed benefit (reduced boundary splitting error in transients) is NOT shown by any registered check.

Post-hoc tests (NOT registered; limits chosen after seeing the diagnostics above; they do not replace the misses): `f2c`
wall-bounded decaying mode BDF2 order 2.074 in [1.7, 2.3] (errors 4.451e-3 and 1.057e-3); `f3c` the F3 ratio at `dt = 0.00125`
equals 3.539 against the registered 3.0; `f8b` the Chorin finer-pair step-halving order lies in [0.7, 1.3] (measured 0.800 Kim-Moin /
0.770 plain) and the incremental form is at least 10 times closer to the steady state. The diagnostic code is not in the
repository.

### Deviations from the registered text

1. W5, dynamic mesh: the conflict of particles with `dynamic_mesh.enabled` was already a located error at `dynamic_mesh.enabled`
   (ADR 0033 check, raised by `semantic_errors`), so the test asserts that path; the guard added at `particles.enabled` in
   `Simulation::new` is unreachable through the tree and untested.
2. C2: the "K4 registered 5e-6" in the C2 text is a misquote of the K4a limit of ADR 0027, which is 1e-5 (5.2e-6 was the measured
   value there). The measured 5.248e-6 reproduces the in-process K4 result exactly. The 5e-6 criterion is kept as the ignored test
   `c2_known_miss_discrete_solution_5e_6`; the corrected test `c2_binary_k4_plug_flow_matches_in_process_and_the_k4a_limit`
   asserts the K4a limit 1e-5. The K4 case also needed a convergence condition (`convergence.a`: `res_A` below 1e-11, 40 minimum
   iterations): the normalised momentum residual of the uniform plug flow does not fall below `tolerance_u` (0.79 after 800
   iterations, continuity 1e-16), a property of the case unrelated to the chemistry.
3. D1 said `supports_restart()` is false "while particles are enabled"; implemented for transient algorithms only. Steady runs are
   tracked once from the final field, so a steady restart stays valid.
4. D5 named `chem_output.rs`; the chemistry report and rate arrays live in `chemistry.rs` (`ChemState::report`, `rate_fields`)
   with thin accessors in `sim.rs`. `particles.output_file` keeps its library meaning (as registered).
5. The old test `pt14_mesh_dependent_errors_are_located` (ADR 0028) built its simulation with particles enabled and expected
   `Simulation::new` to succeed; since D1 reports those errors in `Simulation::new`, its set-up now builds the simulation from the
   same case with the section switched off. Its assertions are unchanged; the new behaviour is checked by W5.
6. A particle-tracking failure during a run (tracker error) stops the simulation with `failure` set (like a user-function error).
7. The first commit of the fractional-step schema keys (56d4fd4) contained the three keys twice (a mistake of my commit tooling);
   it was corrected in the next commit.
8. F-210 stops at `partial`: the registered BDF2 temporal-order, spatial-order and Kim-Moin checks are misses (above). The roadmap
   acceptance (Taylor-Green decay within 2 percent, cavity against published data) is met by F1 and F6.
9. Fractional-step runs are not restartable (`supports_restart()` false); a restart would drop the lagged pressure increment.

### Decision D6 (F-209)

Not attempted. The remainder needs error-estimator stepping wired to the tree, residual smoothing and Aitken acceleration with a
Taylor-Green step-count comparison, which is not cheap; the status stays `todo`.
