---
type: source
tags: [interface, case, physics]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: scalars, species and properties (with the material library)

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "`scalars`, `species` and `properties` (F-053)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

Passive scalars and species mass fractions transported on the flow flux, temperature-dependent property laws, and the Rust material library (F-111) that has no case keys of its own.

## Key points

- Scalars/species run in steady `simple`/`simplec`/`coupled` and (wave 15A) transient `piso`/`pimple`; constant density.
- Per entry: `diffusivity` or `schmidt`, `initial`, `source`, `relax` (1), `boundaries.<patch>` `zero_gradient`/`fixed`/`gradient`. Fields appear as `res_<name>` monitors; reports `scalar_flux`, `scalar_average`.
- Species: the balance species (`remainder = true`, else the last by name) closes `Y_N = 1 - sum Y_i`; initial and fixed fractions must sum to one.
- `properties.viscosity`/`conductivity`/`specific_heat` laws: `constant`, `polynomial`, `sutherland`, and (wave 21B, F-220, ADR 0036) `piecewise_linear`, `piecewise_polynomial`, `vogel`; `extrapolation` `clamp` (default)/`linear`/`error`. Need `energy.enabled` and steady SIMPLE/SIMPLEC. "No property data is shipped."
- Material library (`rcfd_core::fluids`, `rcfd_core::iapws`): NASA-polynomial species, Wilke mixing, ideal/RK/PR equations of state, IAPWS-IF97 regions 1 and 2. Ready-to-paste air laws valid 200-600 K at 100 kPa.
- Not covered: variable density, multicomponent diffusion, reactions (a first-order sink as a zone source).

## Contradictions with the wiki

See [[doc-case-schema-other-solver-physics]] (scalars "steady-only" there vs. transient here).

## Pages this source informs

- [[rcfd-core]], [[report]]
