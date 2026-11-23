---
type: concept
tags: [foundation, structural, dynamics, random-vibration]
sources: [ref-bathe-mit-2-092-fea-notes, ref-nasa-sp-8050-structural-vibration-prediction, adr-thread-structural-dynamics-harmonic, adr-thread-structural-fatigue-and-radiated-sound]
builds_on: [modal-analysis, fourier-analysis-and-spectra]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Harmonic and random response

## In short

Many loads on a structure are periodic (an unbalanced rotor, blade-passing pressure) or random (turbulence, acoustic noise). For a periodic load one computes the steady response at each forcing frequency; it is large near natural frequencies and small far above them. For a random load the response is itself random and is described by its spectral density over frequency and its root-mean-square value ([[ref-bathe-mit-2-092-fea-notes]], lecture 19; [[ref-nasa-sp-8050-structural-vibration-prediction]], sec. 2.1.2.2).

## Explanation

**One mode under a harmonic load.** In modal coordinates each mode obeys $$\ddot x_i + 2\omega_i\xi_i\,\dot x_i + \omega_i^2 x_i = \sin\hat\omega t$$ (Bathe, lecture 19). The dynamic load factor, dynamic peak over static peak response, shows the behaviour: when $\hat\omega$ is very small the mass follows the load and the response is static; when $\hat\omega$ is very large there is no response; in between, near $\hat\omega = \omega_i$, it is amplified, limited by damping ([[ref-bathe-mit-2-092-fea-notes]]).

**Which modes to keep.** Fourier analysis of the loads gives their highest frequency $\omega_u$; a cut-off frequency above it decides the modes retained, and the mesh must represent the structure accurately up to that cut-off. Modes far above the load frequencies respond quasi-statically, which a static correction $\mathbf K\Delta\mathbf U_S = \Delta\mathbf R$ adds back ([[ref-bathe-mit-2-092-fea-notes]], lecture 19; [[modal-analysis]]).

**Frequency-domain solution.** The uncoupled modal equations can be solved by the mode-displacement or mode-acceleration methods, or by mechanical admittance (impedance) methods, which also work on the coupled system; the response at each frequency of interest is the sum over the orthogonal modes ([[ref-nasa-sp-8050-structural-vibration-prediction]], sec. 2.1.2.2).

**Random loads.** "When the applied loading is random in nature, the response is also random"; the solution at a point is then a function of frequency, a spectral density. The acceleration spectral density follows from the displacement spectral density, internal loads and stresses from root-mean-square displacements, and fatigue data (number of response cycles, probability of peak levels) can be predicted. Spatially distributed random loads need their cross-correlation, e.g. for aeroacoustic noise ([[ref-nasa-sp-8050-structural-vibration-prediction]], sec. 2.1.2.2). Spectra and spectral density are on [[fourier-analysis-and-spectra]].

**Damping decides the peaks.** At resonance the amplitude is set by damping, and damping is the least certain input; viscous or modal damping is assumed for convenience ([[ref-nasa-sp-8050-structural-vibration-prediction]]; [[structural-dynamics]]).

**Not covered by the sources read.** The explicit transfer-function formula $S_y = |H|^2 S_x$, complex-amplitude (direct) harmonic solves and loss-factor (hysteretic) damping were not written out in the sources read.

## Builds on

- [[modal-analysis]]: modal coordinates, damping ratios, truncation and static correction.
- [[fourier-analysis-and-spectra]]: frequency content of loads, spectral density.

## Leads to

- [[fatigue]] (damage from response cycles and spectral moments), [[adr-thread-structural-dynamics-harmonic]]

## How rcfd uses it

- Harmonic response by mode superposition with modal damping, static correction, effective mass and force harmonics (F-399, ADR 0391, done); static correction of free bodies by inertia relief (F-491); a direct complex symmetric solve with loss factors, dashpots and base motion (F-412, ADR 0448, partial) ([[adr-thread-structural-dynamics-harmonic]]).
- Random vibration: force and base-acceleration spectra, spectral moments, RMS displacement, stress and von Mises (F-417, ADR 0477, done); complex cross-spectral matrices and coherence (F-567, ADR 0510, done) ([[adr-thread-structural-dynamics-harmonic]]).
- Spectral moments feed the broadband Dirlik fatigue damage (F-566) ([[adr-thread-structural-fatigue-and-radiated-sound]]).
