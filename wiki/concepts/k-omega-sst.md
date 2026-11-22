---
type: concept
tags: [turbulence, rans, two-equation]
sources: [doc-case-schema-turbulence, doc-reference-data-sst-wall, adr-thread-transient-rans, adr-thread-periodic-seam-diffusivity, adr-thread-particle-wall-and-droplets, doc-roadmap-json, adr-thread-early-turbulence, adr-thread-fan01-rotating-solve]
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
- **Early checks and the flat-plate diagnosis (added by p3-records-a):** SST and BSL were checked against the CFL3D flat-plate reference in ADR 0015 (B1, B3 met; B2 White curves, B4 y+~1 stability, B5 wall-resolved step missed). After the wave 30C validation missed D1, D3 and the D2 log-law fit, three diagnoses followed: ADR 0063 (no defect; a first-order sensitivity to the published wall-face omega factor 10), ADR 0070 (term-by-term audit of `sst.rs` against TMR, no defect; observed order 0.66) and ADR 0074 (on the TMR nested grids rcfd converges with order 1.07, but lies 0.82 to 0.87 percent below the mean of three incompressible codes, still open) ([[adr-thread-early-turbulence]]).
- **On the rotating FAN-01 passage (added by p3-records-a):** ADR 0075 fixed the SST relative wall velocity (Galilean-invariance error from 2.88e-1 to 3.65e-10 of U_cl at y+ 30) and ADR 0078 let `k_omega_sst` accept a `moving_wall`; the F-126 attempt 3 ran SST with `upwind`; ADR 0111 found the SST change criterion of the stop rule "NOT A DEFECT" ([[adr-thread-fan01-rotating-solve]]).

## Related

- [[k-epsilon]], [[spalart-allmaras]], [[rcfd-turb]], [[flat-plate]], [[backward-facing-step]], [[fan-01]], [[roadmap-turbulence]]
- [[adr-thread-early-turbulence]], [[adr-thread-fan01-rotating-solve]]

## Open questions

- NACA0012, generalised k-omega/EARSM and the low-Re toggle are not done (F-043 gap) ([[roadmap-turbulence]]).
- The 0.82 to 0.87 percent offset below the TMR mean on the nested grids is open (ADR 0074) ([[adr-thread-early-turbulence]]).
