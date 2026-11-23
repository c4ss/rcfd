---
type: source
tags: [interface, udf, sandbox]
sources: []
origin: docs/udf-abi.md
origin_commit: 09647baa
created: 2026-11-22
updated: 2026-11-23
status: draft
---

# Sandboxed user functions: WebAssembly ABI version 1

**Origin:** [docs/udf-abi.md](../../docs/udf-abi.md) at `origin/main` 09647baa (first read at 2e94e5ef; F-122, hook catalogue v2 F-239, interpreter wasmi 1.1.0 by F-856)
**Kind:** ABI contract

## Summary

The contract between rcfd and a user-function module: case keys, exports with signatures and units, sandbox limits and failure behaviour. Modules run in the `wasmi` interpreter (pure Rust, no JIT), with no file, network, clock, process or host-memory access and no WASI. See [[udf-sandbox]].

## Key points

- Every module exports `rcfd_abi_version() -> i32` returning `1`; other exports optional. All numbers SI, reals `f64`.
- Exports: `momentum_source`, `velocity_profile`, `kinematic_viscosity`, `monitor`, `temperature_profile`, `wall_heat_flux`, `thermal_conductivity`, `energy_source`, `scalar_source`, `initial_value`, `execute_at_end`.
- At most one module provides each of momentum source, viscosity, conductivity, energy source and initialisation; explicit expressions win over UDF boundary hooks.
- Limits: no imports (refused at load); `fuel_per_call` (default 1e6) renewed before every call; linear memory 16 MiB, one memory, one table of 10 000 entries, module at most 4 MiB.
- A trap, exhausted fuel, a non-finite result or a non-positive viscosity stops the run: state `failed`, exit 1. "Nothing panics." Loading problems are case diagnostics under `udf.<name>.*`. Calls are deterministic.
- Source and property hooks act in steady SIMPLE/SIMPLEC only; `execute_at_end` is not called when a run is ended from outside the step.
- Not implemented: transient/coupled/frozen-flow hooks, turbulence/species sources, host functions, batch interface ("would be ABI version 2"); "expect a few microseconds per call".
- Since ADR 0782 (F-856, 09647baa): "The unit of fuel is the interpreter's own count of executed instructions. The interpreter is wasmi 1.1.0"; "a loop of 100 turns that cost 915 units under wasmi 0.31 costs 407 now, so the same budget buys about 2.2 times as many turns of that loop. Other code was not measured." ([[adr-thread-early-workflow-and-partials]])
- The limits held by `crates/rcfd-api/tests/udf_limits_adr0782.rs`: growth to exactly 16 MiB succeeds and one page more fails the call; refused at load are a declared memory of 257 pages, a start function that never ends, an import of a function, a memory, a global or a table, a second memory, a 64-bit memory, a tail call, an extended constant expression and a module in the text format.
- Tests: `crates/rcfd-api/tests/udf_hooks_w38a.rs` (each hook reproduces the built-in setting below 1e-9, measured 0) and `udf.rs` (WAT modules; 20 000 calls of a 1000-fuel budget succeed).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[udf-sandbox]], [[exit-codes]], [[located-error]]
