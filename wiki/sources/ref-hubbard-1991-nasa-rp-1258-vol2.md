---
type: source
tags: [reference, foundation, acoustics, duct-modes, noise-metrics, impedance, public-domain]
sources: []
origin: https://ntrs.nasa.gov/citations/19920005561
licence: NTRS determination GOV_PUBLIC_USE_PERMITTED (record API read 2026-11-22). Copyable; not copied to raw/ (141 MB scan; left for the lead).
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Hubbard (ed.) (1991), Aeroacoustics of Flight Vehicles: Theory and Practice, Vol. 2 Noise Control, NASA RP-1258

**Origin:** H. H. Hubbard (ed.), *Aeroacoustics of Flight Vehicles: Theory and Practice. Volume 2: Noise Control*, NASA Reference Publication 1258, Vol. 2, August 1991. NTRS 19920005561. PDF: https://ntrs.nasa.gov/api/citations/19920005561/downloads/19920005561.pdf
**Kind:** edited NASA reference book (chapters 11-18 and a glossary). Locators: chapter, book page, equation.
**Licence:** US Government work, public use permitted (NTRS record checked 2026-11-22; the foundations-sources table had it "licence not checked"). Copyable, but the 141 MB scan was not copied into `raw/`.
**Read:** 2026-11-22 by job p6-compressible-acoustics: ch. 11 (Powell and Fields, "Human Response to Aircraft Noise") pp. 2-9; ch. 12 (Embleton and Daigle, "Atmospheric Propagation") pp. 54-56; ch. 13 (Eversman, "Theoretical Models for Duct Acoustic Propagation and Radiation") pp. 103-112; ch. 16 (Mixson and Wilby, "Interior Noise") p. 301; Glossary of Terms pp. 415-417. Other chapters not read.

## Summary

The noise-control volume: perception and noise metrics, atmospheric propagation, duct acoustics and liners, jet noise suppression, interior noise, flyover measurement and quiet design. The parts read give the decibel definitions and A-weighted summation, geometrical spreading, the linearised acoustic equations, hard-walled circular-duct modes with cut-off, and the wall impedance boundary condition.

## Key points by chapter (own summary)

### Ch. 11, Human Response (Powell and Fields)
- **Hearing (pp. 3-4):** the pressure ratio of the threshold of pain to the threshold of audibility is "approximately 1 million"; the audible range "is normally considered to be 20 Hz to 20 kHz", with lower sensitivity at both ends; levels above about 85 dB can cause hearing loss depending on exposure.
- **Loudness (p. 4, fig. 2):** equal-loudness contours of octave-band SPL; greatest sensitivity at about 3 kHz.
- **A-weighting (pp. 8-9, fig. 5, Eq. 1):** the A-weighting filter, originally meant to approximate loudness for SPL between 24 and 55 dB, correlates well with noisiness and loudness of many broadband sounds; band levels are summed on an energy basis after weighting: `L_A = 10 log10 [ sum_i 10^(L_A(i)/10) ]` (Eq. 1), `L_A(i)` the weighted band SPLs of octave or 1/3-octave bands.

### Ch. 12, Atmospheric Propagation (Embleton and Daigle)
- **Geometrical spreading (p. 55):** far from a source, wave fronts spread spherically; for a point source "the sound level decreases at the rate of 6 dB per doubling of distance", the same for all frequencies; a line of incoherent point sources (cylindrical spreading) gives 3 dB per doubling. Near a coherent source (the Fresnel region, out to about the source size squared over the wavelength) there is no simple level-distance relation.
- **Absorption (p. 56):** atmospheric absorption, unlike spreading, depends strongly on frequency, temperature, pressure and humidity (classical plus molecular relaxation).

### Ch. 13, Duct Acoustics (Eversman)
- **Acoustic field equations (pp. 103-104, Eqs. 1-8):** start from continuity, momentum and isentropic state of a non-viscous, non-heat-conducting perfect gas (Eqs. 1-4); small perturbations on a mean state `rho_0, p_0, V_0`, with second-order terms dropped, give the acoustic continuity, momentum and energy equations (Eqs. 5-7) and `p = gamma (p_0/rho_0) rho = c_0^2 rho` (Eq. 8).
- **Convected wave equation (p. 105, Eq. 13):** in a uniform duct with uniform mean Mach number `M`, `(d/dt + M d/dx)^2 p = nabla^2 p` (nondimensional); with `M = 0` it is the classic wave equation.
- **Hard-walled circular duct (pp. 105-107, Eqs. 14-17):** wall condition `dp/dr = 0` at `r = 1`; with `exp(i eta t)`, `eta = omega R/c_r`, the convected Helmholtz equation (Eq. 14) has solutions `p_mn = P_mn J_m(kappa_mn r) exp[i(eta t +- m theta - k_x,mn x)]` (Eq. 17), with eigenvalues from `J'_m(kappa_mn) = 0` (Eq. 16) and `k_x,mn/eta = [-M +- sqrt(1 - (1 - M^2)(kappa_mn/eta)^2)]/(1 - M^2)` (Eq. 15). Each mode is a spinning mode `exp[i(eta t +- m theta)]` combined with an axial travelling wave.
- **Cut-on and cut-off (pp. 108-109):** if `(1 - M^2)(kappa_mn/eta)^2 < 1`, `k_x` is real and the mode propagates; if it is `> 1`, `k_x` is complex, the wave decays with distance and "no acoustic power is associated with these modes" ("cutoff"); the cut-off ratio is `beta_mn = eta/(kappa_mn sqrt(1 - M^2))`, cut on when it exceeds one. Tyler and Sofrin: if a source makes only modes with cut-off ratio below one, in principle no power propagates; an isolated rotor can be designed that way, but struts and vanes add interaction tones. Equivalent picture: plane waves reflecting between the walls at an angle set by the cut-off ratio (90 degrees at cut-off). For `x > 0` only waves travelling or decaying in `+x` are kept.
- **Lined ducts and impedance (pp. 111-112, Eqs. 18-23):** with a sheared mean flow `M(r)`, a locally reacting liner relates wall pressure and lining particle velocity by `p/v = Z/(rho_r c_r)`, the nondimensional specific wall impedance (Eq. 23); fluid and wall particle displacements are equal at the wall.

### Ch. 16, Interior Noise (Mixson and Wilby)
- **Radiation efficiency (p. 301, Eq. 22):** `sigma_s,e` is "the ratio of the actual power radiated to the power radiated by an infinite flat plate (with the same mean-square velocity) generating a plane wave"; it enters the coupling loss factor of statistical energy analysis.

### Glossary (pp. 415-417)
- **Acoustic power level** `PWL = 10 log10 (W/W_ref) dB`, `W_ref = 10^-12 W`.
- **Sound pressure level** `SPL = 20 log10 (p/p_ref) dB`, `p_ref = 2 x 10^-5 Pa`, `p` the sound pressure in a particular frequency bandwidth.
- **Overall sound pressure level (OASPL):** equal weight to all frequencies; "not standardized but is generally considered to extend from 20 to 20000 Hz".
- **A-weighted sound pressure level:** weighted to approximate the response of the human ear, measured with an "A" weighting network.
- **Cutoff, cut-on modes:** duct modes that are attenuated with distance and carry no acoustic power are "cutoff"; modes that propagate are "cut on".
- **Geometrical spreading:** for a point source, 6 dB per doubling of distance for all frequencies.

## Contradictions with the wiki

None found.

## Pages this source informs

[[sound-levels-and-metrics]], [[duct-acoustic-modes]], [[acoustic-wave-equation]], [[non-reflecting-boundaries-and-sponge-layers]]
