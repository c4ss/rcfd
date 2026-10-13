# Sandboxed user functions: WebAssembly ABI version 1 (F-122)

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

[boundaries.inlet]
velocity = "fixed"
velocity_udf = "inlet_profile"   # velocity_profile export of udf.inlet_profile
```

At most one module provides the momentum source and at most one the viscosity. Explicit
`velocity_expression_*` components of a patch win over `velocity_udf`. The time variable passed is 0 in steady
runs and the new time level in transient runs.

## Entry points

All numbers are SI, all reals are `f64`. Every module must export `rcfd_abi_version() -> i32` returning `1`; all
other exports are optional (a role set in the case needs its export).

| Export | Signature | Called |
| --- | --- | --- |
| `momentum_source` | `(x, y, z, t, ux, uy, uz: f64, comp: i32) -> f64` | per cell and iteration, three times (`comp` 0, 1, 2); acceleration in m/s^2 at the cell centre with the current velocity |
| `velocity_profile` | `(x, y, z, t: f64, comp: i32) -> f64` | per boundary face centre, once at build and (transient) every step; velocity component |
| `kinematic_viscosity` | `(x, y, z, t, temperature, speed: f64) -> f64` | per face and iteration; m^2/s, must be positive; `temperature` is 0 without an energy equation, `speed` is the interpolated velocity magnitude |
| `monitor` | `(x, y, z, volume, ux, uy, uz, p, temperature: f64) -> f64` | per cell and iteration; the monitor is the sum of `value * volume` (kinematic pressure `p`) |

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

## Not implemented

* Momentum source and viscosity in transient runs (refused with an error) and anything in the compressible
  solver (refused); no scalar sources, no energy sources, no property tables other than viscosity.
* No host functions (for example logging); no shared memory buffers, so every cell is an individual call. A batch
  interface would be ABI version 2.
* Only the interpreter: expect a few microseconds per call, which makes per-cell momentum sources and per-face
  viscosities cost noticeable time on million-cell meshes.

## Tests

`crates/rcfd-api/tests/udf.rs` (modules written in WAT): constant momentum source equals `physics.body_force`
(relative difference below 1e-9); a curl-free source a = (3x, 0, 0) in a closed box is balanced by p = 1.5 x^2
with the spurious velocity falling from 4.4e-2 (10x10) to 1.1e-2 (20x20) and the pressure error from 3.9e-2 to
1.9e-2; a constant viscosity module and a parabolic `velocity_profile` module reproduce the plain
`physics.nu` / expression runs (below 1e-9); the monitor equals the independent sum to 1e-12; imports, wrong ABI,
wrong signature, missing ABI export, infinite loop, memory growth, NaN, negative viscosity are rejected or stop
the run; 20 000 calls of a 1000-fuel budget succeed (budget renewal); the sandbox keeps module files inside the
root.
