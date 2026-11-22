---
type: source
tags: [interface, case, hub]
sources: []
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema (hub)

**Origin:** [docs/case-schema.md](../../docs/case-schema.md) at `origin/main` 2e94e5ef (3145 lines)
**Kind:** reference page; split into one source page per top-level `##` section

## Summary

The reference for the [[case-file]]: every section and key, with its default, its allowed values, the combinations that are refused and what is not delivered. The case file (TOML or JSON) is a settings tree. `rcfd schema` prints its JSON Schema, the service returns it from `settings.schema`, and the Studio's case editor and the assistant tools read the same tree, so every key is available everywhere. Validation errors from `rcfd validate` and `Case::parse_toml` name the offending key and, for TOML, its line ([[located-error]]); diagnostics found only while the simulation is built are located in the file text afterwards.

## Key points (whole page)

- Recurring rules across sections: a new key or optional table has an explicit unset sentinel (`-1`, `""`, `[]`, `0`) and is inert at its default; a case that does not write it keeps its text, hash and results ([[bit-identical]]). Options a solver or generator does not apply are refused with a located diagnostic instead of being ignored.
- Several keys are left out of the settings hash at their default (`SettingsTree::content_hash`), e.g. `solver.corrector_control`, `[solver.consistent_correctors]`, `[solver.rhie_chow]`, `[cosim]`, the `solver.gradient_*` keys ([[doc-case-schema-other-solver-physics]], [[doc-case-schema-consistent-correctors]]).
- Example cases named in the preamble: `cases/channel/kepsilon_channel.toml`, `kepsilon_inlet.toml`, `sst_channel.toml`, `cases/sod/sod.toml`, `cases/wedge/wedge.toml` ("all run in seconds to a minute in a debug build").
- "Where to go next" points to [[doc-run-control]], [[doc-service-protocol]], [[doc-udf-abi]] and [[doc-studies]] (folded into this hub, no page of its own).

## Section pages

| Section | Page |
| --- | --- |
| `turbulence` | [[doc-case-schema-turbulence]] |
| Mesh generators and boundary-layer inflation | [[doc-case-schema-mesh-generators]] |
| `compressible` | [[doc-case-schema-compressible]] |
| `fluid` variable-density solver | [[doc-case-schema-fluid-variable-density]] |
| Other solver and physics settings | [[doc-case-schema-other-solver-physics]] |
| Wave 13: schemes, wall motion, solution limits | [[doc-case-schema-schemes-wall-motion-limits]] |
| `tables` and profile inputs | [[doc-case-schema-tables]] |
| `statistics` and `output` policy | [[doc-case-schema-statistics-output]] |
| SIMPLEC non-orthogonal compensation | [[doc-case-schema-simplec-compensation]] |
| Landing on `solver.end_time` | [[doc-case-schema-end-time-landing]] |
| Adaptive time stepping | [[doc-case-schema-adaptive-time-stepping]] |
| `rheology` | [[doc-case-schema-rheology]] |
| `coupled` block-coupled solver | [[doc-case-schema-coupled]] |
| `scalars`, `species`, `properties` | [[doc-case-schema-scalars-species-properties]] |
| `sources` momentum sources | [[doc-case-schema-sources]] |
| `chemistry` | [[doc-case-schema-chemistry]] |
| `components` lumped components | [[doc-case-schema-components]] |
| `radiation` | [[doc-case-schema-radiation]] |
| `udf` user functions | [[doc-case-schema-udf]] |
| Caveats | [[doc-case-schema-caveats]] |
| `registers` and `initial.patches` | [[doc-case-schema-registers]] |
| Wave 15A: initialisation, zone sources, user equations | [[doc-case-schema-initialisation-zone-sources]] |
| Wave 16W: `decomposition`, `postprocessing`, partition/sample/inflate | [[doc-case-schema-decomposition-postprocessing]] |
| Wave 16B: boundary-condition catalogue `bc_kind` | [[doc-case-schema-bc-catalogue]] |
| Reactor cases and FW-H surface files | [[doc-case-schema-reactor-fwh-files]] |
| Wave 17B: conjugate heat transfer | [[doc-case-schema-conjugate-heat-transfer]] |
| `particles` | [[doc-case-schema-particles]] |
| `mixing_plane` | [[doc-case-schema-mixing-plane]] |
| Wave 19C: `restart`, `chain`, setup rules | [[doc-case-schema-restart-chain-setup-rules]] |
| Wave 20C: `study` | [[doc-case-schema-study]] |
| Wave 20B: `dynamic_mesh` | [[doc-case-schema-dynamic-mesh]] |
| Wave 21B: `regions` | [[doc-case-schema-regions]] |
| Wave 22: `linear_solver` | [[doc-case-schema-linear-solver]] |
| Wave 27C: viscoelastic and structural load export | [[doc-case-schema-viscoelastic-structural]] |
| Wave 27A: `multiphase` | [[doc-case-schema-multiphase]] |
| Wave 28A: mass transfer, waves, reference point | [[doc-case-schema-multiphase-mass-transfer-waves]] |
| Wave 29B: mixture and cavitation | [[doc-case-schema-multiphase-mixture-cavitation]] |
| Divergence guard, transient | [[doc-case-schema-transient-divergence-guard]] |
| Divergence guard, steady | [[doc-case-schema-steady-divergence-guard]] |
| Pressure-lag check of PIMPLE | [[doc-case-schema-pressure-lag-check]] |
| Consistent pressure correctors | [[doc-case-schema-consistent-correctors]] |
| Step-consistent face flux `[solver.rhie_chow]` | [[doc-case-schema-rhie-chow]] |
| Co-simulation fluid participant `[cosim]` | [[doc-case-schema-cosim]] |
| Least-squares gradient on periodic faces | [[doc-case-schema-periodic-gradient]] |
| Limited linear-upwind extrapolation | [[doc-case-schema-linear-upwind-limiter]] |
| Gradient options `solver.gradient_*` | [[doc-case-schema-gradient-options]] |
| Pressure interpolation of the steady kernel | [[doc-case-schema-pressure-interpolation]] |
| Design-loop files (not part of the schema) | [[doc-case-schema-design-loop-files]] |
| Labels, help and the advanced flag (`rcfd schema --ui`) | [[doc-case-schema-schema-ui]] |

## Contradictions with the wiki

> [!warning] Contradiction (within the repo doc)
> The "Caveats" section says only the standard wall-function formulation is implemented (no scalable or enhanced wall treatment, no rough walls) and that k-epsilon is steady-only; the `turbulence` section and Wave 13 say ADR 0015 wired `wall_function`, `roughness_height` etc. and ADR 0210 added transient `pimple` for k-epsilon. See [[doc-case-schema-caveats]].

## Pages this source informs

- [[case-file]], [[report]], [[located-error]], [[setup-rules]], [[divergence-guard]], [[checkpoint]], [[parametric-study]], [[udf-sandbox]], [[field-store]], [[bit-identical]]
