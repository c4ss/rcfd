---
type: concept
tags: [turbulence, rans, two-equation, wall-functions]
sources: [doc-case-schema-turbulence, adr-thread-transient-rans, adr-thread-periodic-seam-diffusivity, doc-roadmap-json]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# k-epsilon

The two-equation RANS model transporting the turbulent kinetic energy k and its dissipation rate epsilon, with wall functions at the wall. rcfd has the standard, RNG and realizable variants (`turbulence.model = "k_epsilon"`, `variant`); k-epsilon walls must be at rest ([[doc-case-schema-turbulence]]).

## How rcfd uses it

- **Validation state (F-042 partial):** on BFS-C (window `6.16` to `6.36`) Standard `5.350` and Realizable `6.413` miss, RNG passes marginally; scalable wall functions are not wired to the settings tree ([[roadmap-turbulence]], from [[doc-roadmap-json]]).
- **Transient (F-294, F-300 partial):** k-epsilon runs on the outer iterations of `pimple` (ADR 0210), but its BDF2 time order is a known miss whose cause is not found: `p` `0.8031` / `1.4814` / `1.4287` even with the smooth Spalding wall eddy viscosity, so "the kink is not the cause" ([[adr-thread-transient-rans]]).
- **Wall eddy viscosity law:** library option `KEpsilonModel::set_option("wall_nut_law", ..)` (Spalding); no case key ([[adr-thread-transient-rans]]).
- **Periodic seams:** the pair diffusivity applies to the momentum `nu + nu_t` and to k and epsilon `nu + nut_f / sigma` (F-296) ([[adr-thread-periodic-seam-diffusivity]]).
- **The F-040 wall functions** (`WallConstants` kappa `0.41`, E `9.8`, Cmu `0.09`) are the constants the models share; the F-040 channel recovers the log law with a mixing-length interior, "not yet with k-epsilon" ([[adr-thread-transient-rans]]).

## Related

- [[k-omega-sst]], [[spalart-allmaras]], [[rcfd-turb]], [[backward-facing-step]], [[roadmap-turbulence]]

## Open questions

- The cause of the BDF2 order miss (F-300) ([[adr-thread-transient-rans]]).
