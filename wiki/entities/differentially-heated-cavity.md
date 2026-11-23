---
type: entity
tags: [validation, benchmark, natural-convection, heat-transfer]
sources: [doc-reference-data-heated-cavity, doc-reference-data-variable-density, doc-reference-data-wave31a-solver-defects, doc-validation-data, doc-example-projects, doc-milestones-json]
created: 2026-11-22
builds_on: [boussinesq-buoyancy-approximation, energy-equation, convective-heat-transfer]
updated: 2026-11-23
status: draft
---

# Differentially heated cavity (de Vahl Davis 1983)

**What it is:** the square natural-convection cavity benchmark of de Vahl Davis, Int. J. Numer. Meth. Fluids 3 (1983) 249-264, at Ra 1e3 to 1e6, Pr 0.71 ([[doc-reference-data-heated-cavity]]).
**Where:** `cases/heated_cavity/reference/devahldavis1983.csv`, `crates/rcfd-fv/tests/natural_convection.rs`; dataset `heated-cavity-devahldavis1983` ([[doc-validation-data]]).

## Details

- Reference values for Ra 1e3 / 1e4 / 1e5 / 1e6 ([[doc-reference-data-heated-cavity]]):
  - Nu_avg 1.118, 2.243, 4.519, 8.800
  - u_max 3.649, 16.178, 34.73, 64.63
  - v_max 3.697, 19.617, 68.59, 219.36
  - Nu_max 1.505, 3.528, 7.717, 17.925 (arXiv:1412.2866 prints 3.328 for Ra 1e4, "an evident typo")
- First verified from secondary copies; wave 30C (ADR 0061) read the primary Table V from a page scan, so the values are now primary ([[doc-reference-data-heated-cavity]], [[doc-reference-data-wave31a-solver-defects]]).
- Wave 31A diagnosis (rcfd result, not literature): converged 32 x 32 Ra 1e6 value v_max 220.48, Nu 8.873 at alpha_u 0.5 ([[doc-reference-data-wave31a-solver-defects]]).
- Variable-density variant (wave 10, ADR 0005): Le Quere et al. (2005) values, e.g. case T1 at Ra 1e6 with constant properties Nu 8.85978, P/P0 0.856338 ([[doc-reference-data-variable-density]]).
- Since 2026-11-23 the case is a simulation milestone, `heated-cavity` (ADR 0775): headline F-050 (done), 13 member items, 96 %, headline met ([[doc-milestones-json]], [[simulation-milestones]]).

## Builds on

- [[boussinesq-buoyancy-approximation]]: the buoyancy force and the Rayleigh number that defines the benchmark cases.
- [[energy-equation]]: the temperature equation coupled to the flow.
- [[convective-heat-transfer]]: the Nusselt number the benchmark reports.

## Related

[[lid-driven-cavity]], [[validation-data-pack]], [[doc-reference-data]]
- [[example-project-quick-start]] (40 x 40 Studio example, Ra 1e3 and 1e4: nu_hot 1.119 and 2.257)

## Open questions

- None recorded beyond the general list in [[doc-verification-report]].
