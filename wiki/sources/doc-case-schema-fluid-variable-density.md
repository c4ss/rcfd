---
type: source
tags: [interface, case, compressible]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: fluid variable-density solver (ideal gas, real gas)

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "`fluid` variable-density solver: ideal gas and real gas", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`fluid.density = "ideal_gas"` or `"real_gas"` selects the pressure-based variable-density SIMPLE/SIMPLEC solver (`rcfd_fv::varden`; F-050, F-061, ADR 0005, ADR 0007), laminar, with a total-enthalpy energy equation; transient with `pimple` (ADR 0009).

## Key points

- Requires `simple`/`simplec`/`pimple` and `energy.enabled = true`; `fluid.mode` `compressible` (default) or `low_mach`.
- Defaults: `gas_constant` 287.058, `gamma` 1.4, `viscosity` 1.8e-5 Pa s, `prandtl` 0.71, `reference_pressure` 101325 Pa.
- Real gas: `eos` `peng_robinson` (default) or `redlich_kwong`; built-in species N2, O2, AR, CO2, CH4, H2, H2O; no liquid root or phase change; "van der Waals is not available".
- Transient: inner iterations until `fluid.inner_tolerance` (1e-8), at most `fluid.inner_iterations` (30); fixed steps only.
- Many features are errors with the setting path (turbulence, rheology, scalars, sources, periodic meshes, `statistics`, ...).
- `res_u` "typically floors near 1e-5..1e-6", so the default `tolerance_u` may never be met; stop on `convergence.*` instead.
- Limits: laminar, no non-orthogonal pressure correctors; "no shock-tube or choked transient benchmark is claimed (ADR 0009)".

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[case-file]]
