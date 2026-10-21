# Sandboxed user functions: WebAssembly ABI version 1 (F-122, hook catalogue v2 F-239)

A case can load user code as WebAssembly modules (`udf.<name>`). The code runs inside the `wasmi` interpreter
(pure Rust, no JIT), so a module cannot touch the host: there is no file, network, clock, process or host-memory
access, and no WASI. Any language that compiles to plain `wasm32` works (Rust `no_std`, C, Zig, WAT).

## Case settings

```toml
[udf.heating]
file = "heating.wasm"        # relative to the case file; inside the root under the service sandbox
fuel_per_call = 1000000      # instruction budget of one call (default 1e6)
momentum_source = true       # use the module's momentum_source export (steady SIMPLE/SIMPLEC)
viscosity = false            # use kinematic_viscosity (steady, no turbulence model)
monitor = true               # record the monitor export as udf_heating
conductivity = false         # thermal_conductivity export replaces physics.conductivity (steady, energy)
energy_source = false        # add the energy_source export to the energy equation (steady, energy)
initialise = false           # initial velocity, pressure and temperature from initial_value
at_end = false               # evaluate execute_at_end once when the run ends, as udf_end_heating

[boundaries.inlet]
velocity = "fixed"
velocity_udf = "inlet_profile"   # velocity_profile export of udf.inlet_profile
temperature = "fixed"
temperature_udf = "inlet_t"      # temperature_profile export of udf.inlet_t

[boundaries.heater]
temperature = "heat_flux"
heat_flux_udf = "heater"         # wall_heat_flux export of udf.heater

[scalars.dye]
diffusivity = 1e-3
source_udf = "dye_src"           # scalar_source export of udf.dye_src
```

At most one module provides each of the momentum source, the viscosity, the conductivity, the energy source and the
initialisation. Explicit `velocity_expression_*` components of a patch win over `velocity_udf`, `temperature_expression`
wins over `temperature_udf` and `heat_flux_expression` over `heat_flux_udf`. The time variable passed is 0 in steady
runs and the new time level in transient runs (`velocity_profile`); `temperature_profile` and `wall_heat_flux` are
evaluated once at t = 0, like the temperature and heat-flux expressions.

## Entry points

All numbers are SI, all reals are `f64`. Every module must export `rcfd_abi_version() -> i32` returning `1`; all
other exports are optional (a role set in the case needs its export).

| Export | Signature | Called |
| --- | --- | --- |
| `momentum_source` | `(x, y, z, t, ux, uy, uz: f64, comp: i32) -> f64` | per cell and iteration, three times (`comp` 0, 1, 2); acceleration in m/s^2 at the cell centre with the current velocity |
| `velocity_profile` | `(x, y, z, t: f64, comp: i32) -> f64` | per boundary face centre, once at build and (transient) every step; velocity component |
| `kinematic_viscosity` | `(x, y, z, t, temperature, speed: f64) -> f64` | per face and iteration; m^2/s, must be positive; `temperature` is 0 without an energy equation, `speed` is the interpolated velocity magnitude |
| `monitor` | `(x, y, z, volume, ux, uy, uz, p, temperature: f64) -> f64` | per cell and iteration; the monitor is the sum of `value * volume` (kinematic pressure `p`) |
| `temperature_profile` | `(x, y, z, t: f64) -> f64` | per face of a `temperature_udf` patch, once at build; boundary temperature in K |
| `wall_heat_flux` | `(x, y, z, t: f64) -> f64` | per face of a `heat_flux_udf` patch, once at build; heat flux into the domain in W/m^2 (also wired into the conjugate heat transfer heat-flux path, which is not tested) |
| `thermal_conductivity` | `(x, y, z, t, temperature: f64) -> f64` | per face and iteration; W/(m K), must be positive; `temperature` is the face-interpolated value (boundary value on boundary faces); the face diffusivity is `k / (rho cp)` |
| `energy_source` | `(x, y, z, t, temperature: f64) -> f64` | per cell and iteration; W/m^3, divided by `rho cp` and added to the other heat sources of the iteration (zone sources, components, chemistry, particles; only the zone-source combination is tested) |
| `scalar_source` | `(x, y, z, t, value: f64) -> f64` | per cell and iteration for each passive scalar whose `source_udf` names the module; value per second, added to `source`, `source_expression` and zone sources; `value` is the current cell value |
| `initial_value` | `(x, y, z: f64, comp: i32) -> f64` | per cell, once at build: velocity components (`comp` 0, 1, 2) in m/s, kinematic pressure (3), temperature (4, only with the energy equation); applied after `initial.velocity` and `initial.velocity_expression_*`, before register patches, which still win |
| `execute_at_end` | same as `monitor` | per cell, once, in the step that ends the run (converged, iteration limit or end time); the sum of `value * volume` is the monitor `udf_end_<name>` |

The momentum source is interpolated to the faces like the Boussinesq force and enters the face flux and the
cell momentum balance together with `physics.body_force` and buoyancy. Results must be finite.

## Limits and failure behaviour

* The module must import nothing; a module with any import is refused at load time (message names the import).
* Fuel (instructions) per call is `fuel_per_call` and is renewed before every call; running out traps.
* Linear memory is capped at 16 MiB (growth beyond it traps), at most one memory, one table of 10 000 entries,
  module size at most 4 MiB.
* A trap, exhausted fuel, a non-finite result or a non-positive viscosity stops the run with the reason in
  `stop_reason`; the run-control state is `failed` (exit code 1). Nothing panics. Loading problems (unreadable
  file, invalid module, wrong ABI, missing export, wrong signature, sandbox escape) are case diagnostics under
  `udf.<name>.*`.
* Calls are deterministic (the module has no clock or randomness), so a run is reproducible.

Scope of the source and property hooks: `momentum_source`, `kinematic_viscosity`, `thermal_conductivity`,
`energy_source` and `scalar_source` act in steady SIMPLE/SIMPLEC runs only (any other algorithm is refused with a
diagnostic). `thermal_conductivity` is refused together with `properties.conductivity` / `properties.specific_heat`
laws, conjugate heat transfer, radiation and `heat_flux` boundaries (which convert the flux with
`physics.conductivity`); `energy_source` is refused with conjugate heat transfer and radiation. Both need
`energy.enabled`. The `heat_flow` report keeps its own `conductivity` setting. `initialise` needs
`initial.method = "uniform"`. `execute_at_end` is not called when a run is ended from outside the step: a
`Simulation::run(n)` cap below the iteration limit, a run-control stop, a wall-clock limit or the divergence guard.

## Not implemented

* Momentum source, viscosity, conductivity, energy and scalar sources in transient, coupled or frozen-flow runs
  (refused with an error) and anything in the compressible, variable-density or VOF solvers (refused).
* Hooks of the F-239 catalogue not offered: turbulence sources, species (multicomponent) sources, an adjust hook
  that changes fields between iterations, an exit hook separate from `execute_at_end`, reaction rates, dynamic-zone
  motion, specific heat and density. Particle hooks are gated on the particle model (Lagrangian tracking, F-056 and
  F-128, is partial) and none is offered.
* No host functions (for example logging); no shared memory buffers, so every cell is an individual call. A batch
  interface would be ABI version 2.
* Only the interpreter: expect a few microseconds per call, which makes per-cell momentum sources and per-face
  viscosities cost noticeable time on million-cell meshes.

## Tests

`crates/rcfd-api/tests/udf_hooks_w38a.rs` (F-239, ADR 0088): each new hook set to a constant or analytic function
reproduces the matching built-in setting (relative difference below 1e-9, measured 0 in every case; all runs steady
SIMPLEC):
`temperature_profile` = `temperature_expression`; `wall_heat_flux` = `heat_flux_value` and the linear closed form
(3.5e-13); `thermal_conductivity` = the constant-property run (`physics.conductivity` set differently in the hook
run); `energy_source` = the uniform zone source, the slab profile identity (1.1e-10 of the rise) and, for the linear
source 2e4 x / L, the closed form (4.7e-4 at 64 cells); hook 5e3 plus zone source 5e3 = zone source 1e4; `scalar_source` = the uniform scalar `source` and its slab
identity (9.9e-15); `initial_value` = `initial.velocity_expression_x` plus uniform pressure and temperature (at build
and after 50 iterations); `execute_at_end` runs once on the final fields (the independent sum to 1e-12). Missing
exports, undefined modules, wrong boundary conditions, two providers, transient algorithms, property-law, heat-flux
and initial-method conflicts are refused; a negative conductivity and a non-finite energy source stop the run.

F-122 tests:

`crates/rcfd-api/tests/udf.rs` (modules written in WAT): constant momentum source equals `physics.body_force`
(relative difference below 1e-9); a curl-free source a = (3x, 0, 0) in a closed box is balanced by p = 1.5 x^2
with the spurious velocity falling from 4.4e-2 (10x10) to 1.1e-2 (20x20) and the pressure error from 3.9e-2 to
1.9e-2; a constant viscosity module and a parabolic `velocity_profile` module reproduce the plain
`physics.nu` / expression runs (below 1e-9); the monitor equals the independent sum to 1e-12; imports, wrong ABI,
wrong signature, missing ABI export, infinite loop, memory growth, NaN, negative viscosity are rejected or stop
the run; 20 000 calls of a 1000-fuel budget succeed (budget renewal); the sandbox keeps module files inside the
root.
