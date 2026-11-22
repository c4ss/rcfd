---
type: entity
tags: [crate]
sources: [doc-architecture, doc-reactor-and-acoustics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# rcfd-reactor

**What it is:** 0D reactor toolkit with a stiff ODE integrator. Depends on no workspace crate. ([[doc-architecture]])
**Where:** `crates/rcfd-reactor/` — outside the layering chain of the [[crate-graph]].

## Dependencies

- Depends on: no workspace crate
- Used by: no workspace crate (binary / top of graph)

## Details

- Not placed in the layering chain of the architecture page.
- Exposed as the stand-alone command `rcfd reactor <case.json> [--out <file.csv>]` with its own JSON input, not a CFD case (F-112, ADR 0013) ([[doc-reactor-and-acoustics]]).
- Reactor types: `constant_pressure`, `constant_volume`, `plug_flow`, `psr` (with blowout search), `equilibrium` (`TP`, `HP`, `UV`); energy `adiabatic` or `isothermal` ([[doc-reactor-and-acoustics]]).
- Reads CHEMKIN-II text mechanisms (NASA-7 thermo, third bodies, `LOW`/`TROE` falloff, `REV`, `DUPLICATE`); `SRI`, `PLOG`, `FORD`, `LT`, `HIGH`, `MOLECULES` rejected with a line number. No mechanism ships; GRI-Mech 3.0 is fetched by `validation-data/fetch.py` (licence recorded as unverified) ([[doc-reactor-and-acoustics]]).
- Integrator: in-repo extrapolated linearly implicit Euler (order 6 columns, forward-difference Jacobian, dense LU), about 80 right-hand-side evaluations per step; adequate for tens of species ([[doc-reactor-and-acoustics]]).
- **Limits:** ideal gas only; plug flow constant-area constant-pressure without pressure drop; no heat exchange model, no surface chemistry, no 1D flame speed ("so F-112 stays partial"), no sensitivity analysis ([[doc-reactor-and-acoustics]]).

## Open questions

_Filled in by P1-P3 ingests._
