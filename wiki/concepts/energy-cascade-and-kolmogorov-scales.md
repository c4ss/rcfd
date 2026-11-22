---
type: concept
tags: [foundation, turbulence]
sources: [ref-davidson-turbulence-notes, ref-mit-ocw-2-29-turbulence]
builds_on: [turbulence]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Energy cascade and Kolmogorov scales

## In short

In a turbulent flow the large eddies take kinetic energy from the mean flow, hand it on to smaller and smaller eddies, and the smallest eddies turn it into heat through viscosity. The rate of that hand-over is the dissipation $\varepsilon$, and the size of the smallest eddies, the Kolmogorov scale, follows from $\varepsilon$ and the viscosity alone ([[ref-davidson-turbulence-notes]], [[ref-mit-ocw-2-29-turbulence]]).

## Explanation

**The cascade.** Richardson's rhyme "Big whorls have little whorls, which feed on their velocity, and little whorls have lesser whorls, and so on to viscosity" summarises it ([[ref-mit-ocw-2-29-turbulence]] L24 s.21). The largest eddies have a length $l_0$ and velocity $v_0$ of the order of the mean flow and extract energy from it; energy is passed from each eddy size to the next smaller one, and the energy transferred per unit time "is the same for each eddy size"; most of it is dissipated at the smallest scales ([[ref-davidson-turbulence-notes]] sec. 5.2, p. 78). Instantaneously energy can also go the other way.

**Two quantities models use.**
- Turbulent kinetic energy, the kinetic energy of the fluctuations: $k = \tfrac{1}{2} \overline{u_i' u_i'} = \tfrac{1}{2}(\overline{u'^2} + \overline{v'^2} + \overline{w'^2})$ $[\mathrm{m^2/s^2}]$ ([[ref-davidson-turbulence-notes]] eq. 5.11; [[ref-mit-ocw-2-29-turbulence]] L25 s.10). It is half the trace of the Reynolds stress ([[reynolds-stress]]).
- Dissipation $\varepsilon$ $[\mathrm{m^2/s^3}]$, energy per unit time and mass turned into heat; it equals the transfer rate through the cascade. An eddy loses its energy in about one turnover, so for the large eddies $\varepsilon \sim v_0^3/l_0$ ([[ref-davidson-turbulence-notes]] eq. 5.15; [[ref-mit-ocw-2-29-turbulence]] L24 s.22). Exactly, $\varepsilon = 2 \nu\, \overline{e_{ij}' e_{ij}'}$ with $e_{ij}'$ the fluctuating strain rate ([[ref-mit-ocw-2-29-turbulence]] L25 s.18).

**Kolmogorov scales.** Assume the smallest eddies depend only on $\nu$ and $\varepsilon$. Dimensional analysis then gives ([[ref-davidson-turbulence-notes]] eq. 5.5):
- velocity $v_\eta = (\nu \varepsilon)^{1/4}$,
- length $\eta = (\nu^3/\varepsilon)^{1/4}$,
- time $\tau_\eta = (\nu/\varepsilon)^{1/2}$.

With $\varepsilon \sim U^3/L$ the ratios to the largest scales are $L/\eta \sim Re_L^{3/4}$, $T/\tau_\eta \sim Re_L^{1/2}$, $U/v_\eta \sim Re_L^{1/4}$ ([[ref-mit-ocw-2-29-turbulence]] L24 s.21). This is why direct simulation becomes impossible at high Reynolds number (about $Re_L^3$ operations in 3D, L25 s.4).

**The spectrum.** Sorting the fluctuations by wavenumber $\kappa$ (about one over eddy size) gives the energy spectrum $E(\kappa)$, with $k = \int_0^\infty E(\kappa)\, d\kappa$. It has three ranges: (I) large energy-containing eddies, where production happens; (II) the inertial subrange, isotropic, where energy is only passed on and $\text{production} = \varepsilon$; (III) the dissipation range at the Kolmogorov scales. In the inertial range dimensional analysis gives Kolmogorov's $-5/3$ law $E(\kappa) = C_K \varepsilon^{2/3} \kappa^{-5/3}$, $C_K \sim 1.5$ ([[ref-davidson-turbulence-notes]] sec. 5.3, eq. 5.13; [[ref-mit-ocw-2-29-turbulence]] L24 s.22).

**What it adds to its prerequisite.** [[turbulence]] says there is a range of eddy sizes; the cascade says how energy moves through that range and gives the two numbers, $k$ and $\varepsilon$, from which a model can build a velocity scale $\sqrt{k}$, a length scale $k^{3/2}/\varepsilon$ and a time scale $k/\varepsilon$. That is the basis of every two-equation model ([[two-equation-turbulence-models]]).

## Builds on

- [[turbulence]]: the range of eddy sizes and the dissipative character.

## Leads to

- [[two-equation-turbulence-models]] ($k$ and $\varepsilon$ as transported quantities), [[k-epsilon]], [[k-omega-wilcox]] ($\omega \sim \varepsilon/k$), [[les-and-des]] (where the filter cuts the spectrum)
- [[lagrangian-particles]] (random-walk dispersion from $k$ and $\varepsilon$)

## How rcfd uses it

- Particle dispersion reads $k$ and $\varepsilon$ from k-epsilon or SST, with $\varepsilon = \beta^* k \omega$, $\beta^* = 0.09$, since F-255 ([[lagrangian-particles]]).
- Inlet turbulence is set from an intensity and a length: $k = 1.5\, (I U)^2$, $\varepsilon = C_\mu^{0.75} k^{1.5} / l$, $l = 0.07\, L$ (`rcfd-api/src/turb.rs`) ([[doc-reference-data-turbulence-constants]]).
