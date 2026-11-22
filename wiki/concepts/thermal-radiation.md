---
type: concept
tags: [foundation, heat-transfer, radiation]
sources: [ref-lienhard-ahtt-6e, ref-openstax-college-physics-ch14]
builds_on: []
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Thermal radiation (Stefan-Boltzmann, emissivity, view factors, absorbing media)

## In short

Every body emits electromagnetic radiation at a rate that grows with the fourth power of its absolute temperature. Exchange between surfaces depends on their emissivities and on how much they see of each other (view factors); a gas between them can absorb and emit along the way ([[ref-lienhard-ahtt-6e]] sec. 1.3, 10.1-10.5; [[ref-openstax-college-physics-ch14]] 14.7).

## Explanation

**Black body and Stefan-Boltzmann.** A black body absorbs all radiation that falls on it and emits the most a body can at its temperature:

$$e_b = \sigma T^4, \qquad \sigma = 5.670374\times10^{-8}\ \mathrm{W/(m^2K^4)}$$ (Lienhard eq. 1.28)

with $T$ in kelvin ([[ref-lienhard-ahtt-6e]] sec. 1.3). The peak wavelength moves to shorter wavelengths as the temperature rises, $(\lambda T)_{max} = 2897.77\ \mu$m K (Wien, eq. 1.29). A real surface emits $Q/t = \sigma eAT^4$ with emissivity $e$ between 0 and 1 (about 1 for a black body, near 0 for a perfect reflector) ([[ref-openstax-college-physics-ch14]] 14.7).

**Gray, diffuse surfaces and Kirchhoff.** A gray body has an emittance independent of wavelength; diffuse emission and reflection have no preferred direction. For such a surface Kirchhoff's law makes the emittance equal the absorptance, $\varepsilon = \alpha$ (Lienhard eq. 10.8c) ([[ref-lienhard-ahtt-6e]] sec. 10.1).

**View factors.** Two black bodies exchange $Q_{net} = A_1\sigma(T_1^4 - T_2^4)$ when body 1 sees only body 2 (eq. 1.31); in general a view factor $F_{1-2}$, "the fraction of energy leaving object 1 that is intercepted by object 2", enters (eq. 1.32), and with emittances a transfer factor $\mathcal F_{1-2}$ (eq. 1.34), which equals $\varepsilon_1$ for a small object in a large isothermal enclosure (eq. 1.35) ([[ref-lienhard-ahtt-6e]] sec. 1.3). View factors from one surface sum to one and obey reciprocity:

$$1 = F_{1-1} + F_{1-2} + \dots + F_{1-n}, \qquad A_1F_{1-2} = A_2F_{2-1}$$ (Lienhard eqs. 10.12, 10.15)

For gray surfaces the radiosity $B = \rho H + \varepsilon e_b$, reflected plus emitted flux, is the unknown of the exchange network (eq. 10.18) ([[ref-lienhard-ahtt-6e]] sec. 10.3).

**Absorbing gases (Beer's law).** Gases scatter and absorb in bands. Along a ray the spectral intensity decays as $di_\lambda = -\rho\kappa_\lambda i_\lambda\,dx$ (eq. 10.47), so

$$i_\lambda(L) = i_\lambda(0)\,e^{-\rho\kappa_\lambda L}$$ (Lienhard eq. 10.48)

with the monochromatic absorption coefficient $\kappa_\lambda$; $\rho\kappa_\lambda L$ is the optical depth, and a non-scattering gas absorbs the fraction $1 - e^{-\rho\kappa_\lambda L}$ (eq. 10.50). The gas also emits, adding to the intensity along the ray (eqs. 10.52, 10.53) ([[ref-lienhard-ahtt-6e]] sec. 10.5). Rosseland's approximation for optically thick media and the P1 method, which rcfd uses, were not described by the sources read here (see Open questions).

## Builds on

- Floor: electromagnetic waves, absolute temperature (first-year physics).

## Leads to

- [[conjugate-heat-transfer]] (radiation as a wall flux), [[energy-equation]] (radiation as a source in participating media)

## How rcfd uses it

- `radiation.model` is `none` (default), `rosseland` or `p1`; `rosseland` adds a radiative conductivity $16\sigma n^2T^3/(3eta)$ (optically thick); `p1` solves the incident radiation $G$ with Marshak conditions and a wall `emissivity`; not done: non-grey band models, WSGG, discrete ordinates, surface-to-surface radiation, transient and ideal-gas runs (F-130, F-052, ADR 0012) ([[doc-case-schema-radiation]]).
- A wall `wall_emissivity` and `radiation_temperature` on the convection wall condition give radiation to the surroundings ([[doc-case-schema-conjugate-heat-transfer]]).
- The Rosseland and P1 references in ADR 0012 are partly "from memory" ([[doc-reference-data-lumped-radiation]], [[adr-thread-early-physics-models]]).

## Open questions

- No open source on the Rosseland approximation, the P1 method or Marshak boundary conditions was read in this job; the rcfd forms stay unchecked here.
