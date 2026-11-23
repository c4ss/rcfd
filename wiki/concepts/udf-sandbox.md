---
type: concept
tags: [interface, udf, sandbox]
sources: [doc-udf-abi, doc-case-schema-udf, adr-thread-early-workflow-and-partials]
created: 2026-11-22
updated: 2026-11-23
status: draft
---

# User functions: the WebAssembly sandbox

User code loaded by a case as a WebAssembly module (`udf.<name>`), run in the `wasmi` interpreter (pure Rust, no JIT; version 1.1.0 since ADR 0782, F-856, 2026-11-23) so that "a module cannot touch the host: there is no file, network, clock, process or host-memory access, and no WASI" ([[doc-udf-abi]]).

## How rcfd uses it

- ABI version 1: the module exports `rcfd_abi_version() -> i32` = 1 and optional hooks (`momentum_source`, `velocity_profile`, `kinematic_viscosity`, `monitor`, `temperature_profile`, `wall_heat_flux`, `thermal_conductivity`, `energy_source`, `scalar_source`, `initial_value`, `execute_at_end`), all `f64` SI ([[doc-udf-abi]]).
- Case keys select roles (`momentum_source`, `viscosity`, `monitor`, catalogue v2 `conductivity`, `energy_source`, `initialise`, `at_end`) and boundary hooks (`velocity_udf`, `temperature_udf`, `heat_flux_udf`, `source_udf`) ([[doc-case-schema-udf]]).
- Limits: no imports; `fuel_per_call` (default 1e6) renewed every call; 16 MiB linear memory; one table of 10 000 entries; module at most 4 MiB ([[doc-udf-abi]]).
- Failure: a trap, exhausted fuel, a non-finite result or a non-positive viscosity stops the run, state `failed`, exit 1; "Nothing panics"; loading problems are located case diagnostics under `udf.<name>.*` ([[doc-udf-abi]]).
- Calls are deterministic, so a run is reproducible ([[doc-udf-abi]]).
- Scope: source and property hooks in steady SIMPLE/SIMPLEC only; cost "a few microseconds per call"; a batch interface would be ABI version 2 ([[doc-udf-abi]]).
- Under the service sandbox, module files must lie inside the root ([[doc-udf-abi]]).
- **Fuel after the move to wasmi 1.1.0 (ADR 0782):** "The unit of fuel is the interpreter's own count of executed instructions"; a loop of 100 turns that cost 915 units under wasmi 0.31 costs 407 now, "so the same budget buys about 2.2 times as many turns of that loop. Other code was not measured." The default `fuel_per_call` is unchanged ([[doc-udf-abi]], [[adr-thread-early-workflow-and-partials]]).
- **Limits held by tests since ADR 0782** (`crates/rcfd-api/tests/udf_limits_adr0782.rs`): growth to exactly 16 MiB succeeds and one page more fails the call; refused at load are a declared memory of 257 pages, a start function that never ends, any import (function, memory, global or table), a second memory, a 64-bit memory, a tail call, an extended constant expression and a module in the text format ([[doc-udf-abi]]). The eight tests pass on wasmi 0.31.2 and 1.1.0 alike ([[adr-thread-early-workflow-and-partials]]).

## Related

- [[exit-codes]], [[located-error]], [[case-file]]
