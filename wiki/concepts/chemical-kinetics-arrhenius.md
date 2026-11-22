---
type: concept
tags: [foundation, chemistry, combustion]
sources: [ref-openstax-chemistry-2e, ref-wikipedia-multiphase-heat]
builds_on: []
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Chemical kinetics and the Arrhenius law

## In short

A chemical reaction goes faster at higher temperature because more molecular collisions carry enough energy to react. The Arrhenius law gives the rate constant as a frequency factor times $e^{-E_a/(RT)}$, with an activation energy $E_a$; finite-rate combustion and reactor models build on it ([[ref-openstax-chemistry-2e]] sec. 12.5; [[ref-wikipedia-multiphase-heat]] "Arrhenius equation").

## Explanation

**Collision theory.** The reaction rate is proportional to the rate of reactant collisions; the colliding species must be oriented so that the atoms that bond in the product touch; and the collision must carry enough energy, the activation energy ([[ref-openstax-chemistry-2e]] sec. 12.5).

**The Arrhenius equation.**

$$k = A\,e^{-E_a/(RT)}$$ (OpenStax Chemistry 2e, sec. 12.5)

with the frequency (pre-exponential) factor $A$, which contains the collision frequency and orientation, the activation energy $E_a$ in J/mol, $R = 8.314$ J/(mol K) and $T$ in kelvin ([[ref-openstax-chemistry-2e]] sec. 12.5). Per molecule, $E_a/R$ becomes $E_a/k_B$ ([[ref-wikipedia-multiphase-heat]] "Arrhenius equation", Formulation).

**Reading $E_a$ from data.** Taking logarithms,

$$\ln k = -\frac{E_a}{R}\,\frac1T + \ln A$$ (OpenStax Chemistry 2e, sec. 12.5)

so $\ln k$ against $1/T$ is a straight line of slope $-E_a/R$; from two temperatures, $\ln(k_1/k_2) = \frac{E_a}{R}(1/T_2 - 1/T_1)$. For the decomposition of HI the slope is about $-2.2\times10^4$ K, $E_a \approx 180$ kJ/mol (Example 12.13) ([[ref-openstax-chemistry-2e]] sec. 12.5). For common activation energies the rate roughly doubles or triples per 10 °C rise ([[ref-wikipedia-multiphase-heat]] "Arrhenius equation").

**Modified form.** A modified form is $k = AT^n e^{-E_a/(RT)}$; $n = 0$ is the original law ([[ref-wikipedia-multiphase-heat]] "Arrhenius equation", Modified Arrhenius equation).

## Builds on

- Floor: temperature, the gas constant, logarithms (first-year chemistry).

## Leads to

- [[rcfd-reactor]] (0D reactors with CHEMKIN mechanisms), finite-rate combustion, temperature-dependent viscosity shifts

## How rcfd uses it

- `rcfd-reactor` reads CHEMKIN-II mechanisms (NASA-7 thermo, third bodies, falloff, reverse rates) and integrates them with an extrapolated linearly implicit Euler method ([[rcfd-reactor]], [[doc-reactor-and-acoustics]]).
- F-054 finite-rate and eddy-dissipation combustion with a CHEMKIN-format reader is partial; F-110 includes an Arrhenius/WLF shift for temperature-dependent viscosity ([[roadmap-heat-materials-and-chemistry]]).

## Open questions

- No open source on stiff ODE integration for chemistry was read in this job; the rcfd integrator is described only from its own pages.
