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

In a turbulent flow the large eddies take kinetic energy from the mean flow, hand it on to smaller and smaller eddies, and the smallest eddies turn it into heat through viscosity. The rate of that hand-over is the dissipation `epsilon`, and the size of the smallest eddies, the Kolmogorov scale, follows from `epsilon` and the viscosity alone ([[ref-davidson-turbulence-notes]], [[ref-mit-ocw-2-29-turbulence]]).

## Explanation

**The cascade.** Richardson's rhyme "Big whorls have little whorls, which feed on their velocity, and little whorls have lesser whorls, and so on to viscosity" summarises it ([[ref-mit-ocw-2-29-turbulence]] L24 s.21). The largest eddies have a length `l0` and velocity `v0` of the order of the mean flow and extract energy from it; energy is passed from each eddy size to the next smaller one, and the energy transferred per unit time "is the same for each eddy size"; most of it is dissipated at the smallest scales ([[ref-davidson-turbulence-notes]] sec. 5.2, p. 78). Instantaneously energy can also go the other way.

**Two quantities models use.**
- Turbulent kinetic energy, the kinetic energy of the fluctuations: `k = (1/2) mean(u_i' u_i') = (1/2)(mean(u'^2) + mean(v'^2) + mean(w'^2))` [m^2/s^2] ([[ref-davidson-turbulence-notes]] eq. 5.11; [[ref-mit-ocw-2-29-turbulence]] L25 s.10). It is half the trace of the Reynolds stress ([[reynolds-stress]]).
- Dissipation `epsilon` [m^2/s^3], energy per unit time and mass turned into heat; it equals the transfer rate through the cascade. An eddy loses its energy in about one turnover, so for the large eddies `epsilon ~ v0^3/l0` ([[ref-davidson-turbulence-notes]] eq. 5.15; [[ref-mit-ocw-2-29-turbulence]] L24 s.22). Exactly, `epsilon = 2 nu mean(e_ij' e_ij')` with `e_ij'` the fluctuating strain rate ([[ref-mit-ocw-2-29-turbulence]] L25 s.18).

**Kolmogorov scales.** Assume the smallest eddies depend only on `nu` and `epsilon`. Dimensional analysis then gives ([[ref-davidson-turbulence-notes]] eq. 5.5):
- velocity `v_eta = (nu epsilon)^(1/4)`,
- length `eta = (nu^3/epsilon)^(1/4)`,
- time `tau_eta = (nu/epsilon)^(1/2)`.

With `epsilon ~ U^3/L` the ratios to the largest scales are `L/eta ~ Re_L^(3/4)`, `T/tau_eta ~ Re_L^(1/2)`, `U/v_eta ~ Re_L^(1/4)` ([[ref-mit-ocw-2-29-turbulence]] L24 s.21). This is why direct simulation becomes impossible at high Reynolds number (about `Re_L^3` operations in 3D, L25 s.4).

**The spectrum.** Sorting the fluctuations by wavenumber `kappa` (about one over eddy size) gives the energy spectrum `E(kappa)`, with `k = int_0^inf E(kappa) d kappa`. It has three ranges: (I) large energy-containing eddies, where production happens; (II) the inertial subrange, isotropic, where energy is only passed on and `production = epsilon`; (III) the dissipation range at the Kolmogorov scales. In the inertial range dimensional analysis gives Kolmogorov's -5/3 law `E(kappa) = C_K epsilon^(2/3) kappa^(-5/3)`, `C_K ~ 1.5` ([[ref-davidson-turbulence-notes]] sec. 5.3, eq. 5.13; [[ref-mit-ocw-2-29-turbulence]] L24 s.22).

**What it adds to its prerequisite.** [[turbulence]] says there is a range of eddy sizes; the cascade says how energy moves through that range and gives the two numbers, `k` and `epsilon`, from which a model can build a velocity scale `sqrt(k)`, a length scale `k^(3/2)/epsilon` and a time scale `k/epsilon`. That is the basis of every two-equation model ([[two-equation-turbulence-models]]).

## Builds on

- [[turbulence]]: the range of eddy sizes and the dissipative character.

## Leads to

- [[two-equation-turbulence-models]] (`k` and `epsilon` as transported quantities), [[k-epsilon]], [[k-omega-wilcox]] (`omega ~ epsilon/k`), [[les-and-des]] (where the filter cuts the spectrum)
- [[lagrangian-particles]] (random-walk dispersion from `k` and `epsilon`)

## How rcfd uses it

- Particle dispersion reads `k` and `epsilon` from k-epsilon or SST, with `epsilon = beta* k omega`, `beta* = 0.09`, since F-255 ([[lagrangian-particles]]).
- Inlet turbulence is set from an intensity and a length: `k = 1.5 (I U)^2`, `eps = Cmu^0.75 k^1.5 / l`, `l = 0.07 L` (`rcfd-api/src/turb.rs`) ([[doc-reference-data-turbulence-constants]]).
