---
type: concept
tags: [turbulence, rans, one-equation, des]
sources: [doc-case-schema-turbulence, doc-reference-data-spalart-allmaras, adr-thread-transient-rans, adr-thread-periodic-seam-diffusivity, doc-roadmap-json]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Spalart-Allmaras

The one-equation RANS model transporting a modified eddy viscosity `nuTilda`. rcfd implements SA-noft2 with the SA-neg terms, constants from the NASA Turbulence Modeling Resource (`turbulence.model = "spalart_allmaras"`), with optional DES/DDES ([[doc-case-schema-turbulence]], [[doc-reference-data-spalart-allmaras]]).

## How rcfd uses it

- **Constants:** sigma `2/3`, cb1 `0.1355`, cb2 `0.622`, kappa `0.41`, cw2 `0.3`, cw3 `2`, cv1 `7.1` ([[doc-reference-data-spalart-allmaras]]).
- **Validation state (F-041 partial):** flat plate Cf misses the power law `0.0576 Re_x^-0.2` at x = 1.0 (`+5.29%` against 5 percent), though it passes against TMR CFL3D, TMR theory and White; no mesh study and no other validation case ([[roadmap-turbulence]], from [[doc-roadmap-json]]).
- **DES:** `sa.des` `off`/`des`/`ddes`, transient only and "not validated on a separated flow" ([[doc-case-schema-turbulence]]).
- **Transient without DES (F-294):** on the outer iterations of `pimple` (ADR 0210); SA-DES stays on the once-per-step path; `supports_transient("spalart_allmaras")` answers true for every DES mode (ADR 0225 Q1) ([[adr-thread-transient-rans]]).
- **Periodic seams:** the `nuTilda` diffusivity was already one value per pair; whether `pair` should use the internal-face rule (which differs where `nuTilda < 0`) is ADR 0212 Q3 ([[adr-thread-periodic-seam-diffusivity]]).

## Related

- [[k-omega-sst]], [[k-epsilon]], [[rcfd-turb]], [[flat-plate]], [[naca0012]], [[roadmap-turbulence]]

## Open questions

- A second validation case (BFS, aerofoil) for F-041 ([[roadmap-turbulence]]).
