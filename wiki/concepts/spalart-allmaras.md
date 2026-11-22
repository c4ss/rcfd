---
type: concept
tags: [turbulence, rans, one-equation, des]
sources: [doc-case-schema-turbulence, doc-reference-data-spalart-allmaras, adr-thread-transient-rans, adr-thread-periodic-seam-diffusivity, doc-roadmap-json, doc-investigation-job49a-sa-drag, doc-investigation-job49e-sa-wall, doc-investigation-job49d-aoa-sweep-fixed, adr-thread-naca0012-sa-drag, ref-nasa-tmr, ref-menter-1992-nasa-tm-103975, ref-davidson-turbulence-notes]
builds_on: [boussinesq-eddy-viscosity, wall-distance, convection-diffusion-equation, les-and-des]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Spalart-Allmaras

The one-equation RANS model transporting a modified eddy viscosity `nuTilda`. rcfd implements SA-noft2 with the SA-neg terms, constants from the NASA Turbulence Modeling Resource (`turbulence.model = "spalart_allmaras"`), with optional DES/DDES ([[doc-case-schema-turbulence]], [[doc-reference-data-spalart-allmaras]]).

## Builds on

*(added by p6-turbulence)*
- [[boussinesq-eddy-viscosity]]: SA is a linear eddy-viscosity model; one-equation models "solve one partial differential equation for the eddy-viscosity" and "appear to be very robust numerically" ([[ref-menter-1992-nasa-tm-103975]] p. 1).
- [[wall-distance]]: the destruction term depends on $d$, which must be the true minimum distance to the wall, not the distance to the nearest wall gridpoint; the source terms are non-zero but small in the free stream, "proportional to 1/d2" ([[ref-nasa-tmr]] `spalart.html`).
- [[convection-diffusion-equation]]: the form of the `nuTilda` transport equation.
- [[les-and-des]]: in DES the wall distance is replaced by $\min(d,\, C_{DES} \Delta)$, $C_{DES} = 0.65$ ([[ref-davidson-turbulence-notes]] eq. 20.3); the TMR notes that with DES/DDES the `ft2` term "may cause an undesirable delay in transition to turbulence in the RANS region", and that SA-neg "should yield essentially identical results to (SA), and is generally recommended because of its more robust numerical behavior" ([[ref-nasa-tmr]] `spalart.html`).

## How rcfd uses it

- **Constants:** sigma `2/3`, cb1 `0.1355`, cb2 `0.622`, kappa `0.41`, cw2 `0.3`, cw3 `2`, cv1 `7.1` ([[doc-reference-data-spalart-allmaras]]).
- **Validation state (F-041 partial):** flat plate Cf misses the power law $0.0576\, Re_x^{-0.2}$ at x = 1.0 (`+5.29%` against 5 percent), though it passes against TMR CFL3D, TMR theory and White; no mesh study and no other validation case ([[roadmap-turbulence]], from [[doc-roadmap-json]]).
- **DES:** `sa.des` `off`/`des`/`ddes`, transient only and "not validated on a separated flow" ([[doc-case-schema-turbulence]]).
- **Transient without DES (F-294):** on the outer iterations of `pimple` (ADR 0210); SA-DES stays on the once-per-step path; `supports_transient("spalart_allmaras")` answers true for every DES mode (ADR 0225 Q1) ([[adr-thread-transient-rans]]).
- **Periodic seams:** the `nuTilda` diffusivity was already one value per pair; whether `pair` should use the internal-face rule (which differs where `nuTilda < 0`) is ADR 0212 Q3 ([[adr-thread-periodic-seam-diffusivity]]).
- On the NACA 0012 at wall y+ about 40 the default molecular force report gave about a third of the applied wall-function friction (0.32), and a `zero_gradient` far field let nuTilda grow to 6.0e4 nu; the fixes `total_effective` and far-field role `freestream` ([[doc-investigation-job49a-sa-drag]]) gave cd +2.5 % (0 degrees) and +9.8 % (10 degrees) against TMR on 500 chords ([[doc-investigation-job49d-aoa-sweep-fixed]]). With the wall cell in the buffer layer, wall mode `auto` over-predicts plate friction by about 20 % at y+ 5 to 8 (linear interpolation of the eddy viscosity across the buffer layer); opt-in `auto_sa_law` brings the plate within +2.3 % to +4.4 % of TMR at y+ 1 to 60 ([[doc-investigation-job49e-sa-wall]]).
- **Defaults since 2026-10-29** ([[adr-thread-naca0012-sa-drag]]): force and force-coefficient reports with `total` or `viscous` take the momentum equation's face viscosity under a turbulence model (old values `total_molecular`, `viscous_molecular`; ADR 0166), moment reports follow (ADR 0170), and a `velocity = "freestream"` patch gets the `freestream` turbulence role under `turbulence = "auto"` (SA in ADR 0166, SST and k-epsilon in ADR 0169).
- **Buffer layer:** with the wall cell at y+ 5 the default `auto` wall mode over-predicts plate friction by +19.9 % against TMR, because `effective_viscosity` interpolates nut linearly across the buffer layer; the opt-in `turbulence.sa.wall_mode = "auto_sa_law"` meets every limit after amendment 1 (plate +2.3 % to +4.4 % at all 10 y+), and the default stays `auto` pending one more job (ADR 0168, [[adr-thread-naca0012-sa-drag]]).

## Related

- [[k-omega-sst]], [[k-epsilon]], [[rcfd-turb]], [[flat-plate]], [[naca0012]], [[roadmap-turbulence]]
- [[doc-investigation-job49a-sa-drag]], [[doc-investigation-job49e-sa-wall]], [[doc-investigation-job49d-aoa-sweep-fixed]], [[naca0012]]

## Open questions

- A second validation case (BFS, aerofoil) for F-041 ([[roadmap-turbulence]]).
