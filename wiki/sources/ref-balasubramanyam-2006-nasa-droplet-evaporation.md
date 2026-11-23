---
type: source
tags: [reference, foundation, multiphase, droplets, evaporation, public-domain]
sources: []
origin: https://ntrs.nasa.gov/citations/20060047753
licence: "Work of the US Gov. Public Use Permitted." (NTRS record, read 2026-11-22)
read: 2026-11-22
raw: raw/balasubramanyam-2006-nasa-droplet-evaporation.pdf
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Balasubramanyam, Chen and Trinh (2006), finite-conductivity droplet evaporation model (NASA preprint)

**Origin:** M. S. Balasubramanyam, C. P. Chen (University of Alabama in Huntsville), H. P. Trinh (NASA Marshall Space Flight Center), "A New Finite-Conductivity Droplet Evaporation Model Including Liquid Turbulence Effect", preprint for ASME J. Heat Transfer (technical brief), NTRS 20060047753. Copy: [raw/balasubramanyam-2006-nasa-droplet-evaporation.pdf](../raw/balasubramanyam-2006-nasa-droplet-evaporation.pdf).
**Kind:** NASA-hosted preprint (scanned with OCR; several equations are illegible in the text layer).
**Licence:** Work of the US Government, public use permitted.
**Read:** 2026-11-22 by job p6-multiphase-heat: whole text (short brief); only equations legible in the OCR layer are used.

## Summary

Extends the T-blob/T-TAB atomisation and spray model of Trinh and Chen with a two-temperature (finite-conductivity) droplet heating model, with Spalding's classical gas-phase evaporation model, and compares one-way and two-way coupled spray calculations with data.

## Key points by section

### Introduction and model development
- Spray evaporation in combustors, engines and rockets depends on atomisation and evaporation; reviews by Sazhin. Eulerian-Lagrangian approach: numerical droplets tracked in Lagrangian coordinates in the Eulerian gas; primary and secondary breakup by the T-blob/T-TAB hybrid model (details in Trinh and Chen, not given here).
- Infinite-conductivity models assume a uniform droplet temperature; the two-temperature model keeps a well-mixed core temperature $T_d$ and a different surface temperature $T_s$ with a thin liquid-side film.
- Surface balance: no heat accumulation at the droplet surface; latent heat at the surface temperature.
- Evaporation rate (Eq. 10, the prefactor partly garbled in the OCR and read as $2\pi r_d$): $\dot m_d = 2\pi r_d\,(\rho_g D)\,Sh_g\ln(1+B_M)$, $B_M$ the Spalding mass transfer number, $Sh_g$ the Sherwood number, $D$ the binary diffusivity. Classical correlations (Eqs. 11, 12): $Nu_g = 2 + 0.6\,Re_p^{0.5}\dots$ (the Prandtl factor is not legible) and $Sh_g = 2 + 0.6\,Re_p^{0.5}Sc^{0.333}$.
- Clausius-Clapeyron and Raoult's law give the vapour mole fraction at the surface.

### Results and conclusions
- The finite-conductivity model predicts shorter droplet lifetimes than the infinite-conductivity model in a quiescent 600 K nitrogen test; two-way coupled spray penetration agrees better with the Yokota et al. data.

## Contradictions with the wiki

None.

## Pages this source informs

[[droplet-evaporation-and-breakup]], [[two-way-coupling]]
