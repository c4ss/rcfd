---
type: concept
tags: [foundation, structural, materials, time-dependent]
sources: [ref-roylance-mit-3-11-mechanics-of-materials, adr-thread-structural-materials-nonlinear, adr-thread-structural-creep-and-temperature]
builds_on: [linear-elasticity]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Viscoelasticity and creep

## In short

Some materials respond with a delay: a polymer held at fixed strain slowly relaxes its stress, and a metal held at fixed stress at high temperature slowly keeps stretching (creep). Linear viscoelasticity models the delay with springs and dashpots, giving a relaxation modulus that decays as a sum of exponentials. Metal creep at high temperature is usually written as a power law in stress with an Arrhenius temperature factor ([[ref-roylance-mit-3-11-mechanics-of-materials]], "Linear Viscoelasticity", "Dislocation Basis of Yield and Creep").

## Explanation

**Creep and relaxation.** Under a step stress the strain grows over time (creep compliance); under a step strain $\epsilon_0$ the stress decays, measured by the relaxation modulus $E_{rel}(t) = \sigma(t)/\epsilon_0$ ([[ref-roylance-mit-3-11-mechanics-of-materials]], "Linear Viscoelasticity").

**Spring-dashpot models.** A Maxwell element, spring $k$ in series with dashpot $\eta$ (eq. 22), relaxes as $$E_{rel}(t) = k\,e^{-t/\tau}, \qquad \tau = \eta/k$$ The standard linear solid adds a parallel spring $k_e$, so the stress relaxes to a non-zero level. The Wiechert model uses as many Maxwell arms as needed: $E_{rel}(t) = k_e + \sum_i k_i\,e^{-t/\tau_i}$, a distribution of relaxation times (in rcfd's terms a Prony series) ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Boltzmann superposition.** For a linear viscoelastic material the response at time $t$ is the sum of the responses to all earlier increments of excitation, each weighted by the time elapsed since it was applied ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Time-temperature shift.** For thermorheologically simple materials a change of temperature shifts the response along log time: $\log a_T = \log\tau(T) - \log\tau(T_{ref})$ (eq. 47), so one master curve serves all temperatures ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Creep in metals.** Creep curves show primary (decelerating), secondary (steady-state) and tertiary (accelerating, to rupture) stages. The secondary rate follows the Weertman-Dorn equation $$\dot\epsilon_{II} = A\,\sigma^m\exp\!\left(-\frac{E_c}{RT}\right)$$ (Roylance, "Dislocation Basis", eq. 8): a power law in stress with an activation energy $E_c$ ([[ref-roylance-mit-3-11-mechanics-of-materials]]). Like plastic flow it is driven by the deviatoric stress ([[linear-elasticity]], [[plasticity-j2]]).

**Not covered by the sources read.** Time-hardening creep laws with an explicit $t^m$ factor, the time-stepping of creep in a finite element code and the complex modulus in frequency were not written out in the sources read.

## Builds on

- [[linear-elasticity]]: the elastic limit of these models and the deviatoric split.

## Leads to

- [[harmonic-and-random-response]] (complex modulus in frequency), [[plasticity-j2]]

## How rcfd uses it

- Linear viscoelasticity by Prony series in time, complex modulus in frequency and a time-temperature shift (F-410, ADR 0485, done), also in the direct harmonic solve and damped modes ([[adr-thread-structural-materials-nonlinear]]).
- Arrhenius creep: `Material::creep_arrhenius(base, a, n, m, Q)` with rate `A exp(-Q/(R T)) q^n t^m` (F-581, ADR 0558, partial) ([[adr-thread-structural-creep-and-temperature]]).
- The thick-walled creep cylinder on B-bar and F-bar Hex8 (F-597, F-580, partial) ([[adr-thread-structural-creep-and-temperature]]).
