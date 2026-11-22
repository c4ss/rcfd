---
type: concept
tags: [turbulence, rans, two-equation]
sources: [doc-case-schema-turbulence, doc-reference-data-sst-wall, adr-thread-transient-rans, adr-thread-periodic-seam-diffusivity, adr-thread-particle-wall-and-droplets, doc-roadmap-json]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# k-omega SST

Menter's shear-stress-transport two-equation RANS model: k-omega near walls blended to k-epsilon behaviour in the free stream by `F1`, with the eddy viscosity limited by `nu_t = a1 k / max(a1 omega, S F2)`. rcfd implements the Menter 2003 form as given by the NASA Turbulence Modeling Resource (`turbulence.model = "k_omega_sst"`) ([[doc-case-schema-turbulence]], [[doc-reference-data-sst-wall]]).

## How rcfd uses it

- **Constants and wall:** beta* `0.09`, a1 `0.31`, beta1 `0.075`, beta2 `0.0828`; wall omega `10 * 6 nu/(beta1 d1^2)`; production limited to `10 beta* rho omega k` ([[doc-reference-data-sst-wall]]). The least accurate range is y+ about 3 to 20; `linearUpwind` for k/omega "diverged in tests" ([[doc-case-schema-turbulence]]).
- **Validation state (F-043 partial):** flat plate misses White's 5 percent band (`-7.36%` at x = 0.97) though it is `-0.7` percent from the TMR CFL3D value; wall-resolved BFS reattachment SST `6.554` against `6.16` to `6.36` ([[doc-roadmap-json]] via [[roadmap-turbulence]], [[doc-reference-data-sst-wall]]).
- **Transient (F-286 done):** SST on the outer iterations of `pimple` on a static mesh, one sweep per outer iteration with the momentum's time derivative; BDF2 order `2.0450` from a start that meets the walls (S3'); `piso` is refused ([[adr-thread-transient-rans]]).
- **Periodic seams:** `turbulence.periodic_seam.diffusivity = "pair"` gives both faces of a seam pair one diffusivity; FAN-01 runs SST, so SST came first (F-295) ([[adr-thread-periodic-seam-diffusivity]]).
- **Particles:** particle dispersion may read the k-epsilon or SST fields (`particles.dispersion_fields = "turbulence_model"`, F-255) ([[adr-thread-particle-wall-and-droplets]]).

## Related

- [[k-epsilon]], [[spalart-allmaras]], [[rcfd-turb]], [[flat-plate]], [[backward-facing-step]], [[fan-01]], [[roadmap-turbulence]]

## Open questions

- NACA0012, generalised k-omega/EARSM and the low-Re toggle are not done (F-043 gap) ([[roadmap-turbulence]]).
