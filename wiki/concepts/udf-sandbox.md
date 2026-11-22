---
type: concept
tags: [interface, udf, sandbox]
sources: [doc-udf-abi, doc-case-schema-udf]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# User functions: the WebAssembly sandbox

User code loaded by a case as a WebAssembly module (`udf.<name>`), run in the `wasmi` interpreter (pure Rust, no JIT) so that "a module cannot touch the host: there is no file, network, clock, process or host-memory access, and no WASI" ([[doc-udf-abi]]).

## How rcfd uses it

- ABI version 1: the module exports `rcfd_abi_version() -> i32` = 1 and optional hooks (`momentum_source`, `velocity_profile`, `kinematic_viscosity`, `monitor`, `temperature_profile`, `wall_heat_flux`, `thermal_conductivity`, `energy_source`, `scalar_source`, `initial_value`, `execute_at_end`), all `f64` SI ([[doc-udf-abi]]).
- Case keys select roles (`momentum_source`, `viscosity`, `monitor`, catalogue v2 `conductivity`, `energy_source`, `initialise`, `at_end`) and boundary hooks (`velocity_udf`, `temperature_udf`, `heat_flux_udf`, `source_udf`) ([[doc-case-schema-udf]]).
- Limits: no imports; `fuel_per_call` (default 1e6) renewed every call; 16 MiB linear memory; one table of 10 000 entries; module at most 4 MiB ([[doc-udf-abi]]).
- Failure: a trap, exhausted fuel, a non-finite result or a non-positive viscosity stops the run, state `failed`, exit 1; "Nothing panics"; loading problems are located case diagnostics under `udf.<name>.*` ([[doc-udf-abi]]).
- Calls are deterministic, so a run is reproducible ([[doc-udf-abi]]).
- Scope: source and property hooks in steady SIMPLE/SIMPLEC only; cost "a few microseconds per call"; a batch interface would be ABI version 2 ([[doc-udf-abi]]).
- Under the service sandbox, module files must lie inside the root ([[doc-udf-abi]]).

## Related

- [[exit-codes]], [[located-error]], [[case-file]]
