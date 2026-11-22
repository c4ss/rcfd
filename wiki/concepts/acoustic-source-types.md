---
type: concept
tags: [foundation, acoustics, monopole, dipole, quadrupole]
sources: [ref-hubbard-1991-nasa-rp-1258-vol1, ref-farassat-2007-nasa-tm-214853]
builds_on: [greens-function-and-retarded-time]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Acoustic source types: monopole, dipole, quadrupole

## In short

Sources of sound are classed by how they enter the wave equation. A monopole is a fluctuating injection of volume (or mass), a dipole is a fluctuating force, and a quadrupole is a fluctuating stress inside the fluid, such as turbulent momentum flux. Each higher order is a less efficient radiator at low Mach number, so at low speed forces on surfaces usually beat turbulence in the free stream ([[ref-hubbard-1991-nasa-rp-1258-vol1]] chs. 1, 4).

## Explanation

**Classification by derivative.** In an inhomogeneous wave equation, source terms involving $\partial/\partial t$, $\partial/\partial x_i$, $\partial^2/\partial x_i \partial x_j$ and $\partial^3/\partial x_i\partial x_j\partial x_k$ "are labeled, respectively, monopole, dipole, quadrupole, and octopole" ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 4, p. 215). Lighthill's source $\partial^2 T_{ij}/\partial x_i\partial x_j$ is therefore a quadrupole ([[lighthill-acoustic-analogy]]).

**Physical meaning for a rotor.** For propellers, Magliozzi, Hanson and Amiet write the linear wave equation with a volume distribution of sources $q$ for the blade thickness and of doublets of the blade force $\mathbf{F}$ for the loading, the same source and vortex picture as wing aerodynamics ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 1, p. 11, eq. 2, fig. 8). Thickness noise, from the air displaced by the passing blade volume, "can be represented by a monopole source distribution and becomes important at high speeds"; loading noise, from the thrust and torque forces, is "an important mechanism at low to moderate speeds"; quadrupole sources in the volume around the blades matter when the flow over the blade sections is transonic (pp. 5-6).

**A caution on moving surfaces.** Farassat calls the thickness and loading terms of the FW-H equation "known also as monopole and dipole sources", but finds the terminology "misleading for sources on a moving surface because the radiation patterns ... are not similar to those from stationary monopoles and dipoles"; the thickness noise equals the loading noise of a uniform surface pressure $\rho_0 c^2$ ([[ref-farassat-2007-nasa-tm-214853]] sec. 2, eqs. 5-6).

**Random sources.** Following Curle, the noise of a surface comes from forces (dipoles) and sources (monopoles) on it. Harmonic terms give harmonic noise; random dipoles give broadband noise. "Usually there are no monopoles with a random time behavior", since that would need the surface to move randomly. Turbulence away from an edge "produces quadrupole sound, which is generally a weak generation mechanism compared with dipole sound"; near a trailing edge the loading changes as each eddy passes and sound is produced ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 1, p. 28).

**Velocity scaling.** From Lighthill's analogy, the sound power radiated per unit volume of turbulence (a quadrupole) is "proportional to the eighth power of the flow velocity" ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 4, p. 218). In a heated jet the temperature fluctuations add a dipole term, and the far-field intensity becomes proportional to $M^6$ at low Mach number instead of $M^8$ for the isothermal jet (p. 219). The lower power of the dipole is why it dominates at low speed. (The monopole's own scaling is not given in the sources read.)

**What it adds.** The [[greens-function-and-retarded-time]] gives the field of a point impulse; differentiating that field in space gives the dipole and quadrupole fields. This page names those building blocks and says which flow mechanism each represents.

## Builds on

- [[greens-function-and-retarded-time]]: the point-source solution that the higher orders are derivatives of.

## Leads to

- [[lighthill-acoustic-analogy]], [[rotor-tone-noise]], [[fw-h]]
- [[aeroacoustics]]

## How rcfd uses it

- rcfd's FW-H computes the thickness and loading surface terms and no volume (quadrupole) term ([[fw-h]]); for an impermeable fan blade $U_i = v_i$, $L_i = p' n_i$, and `loading` mode sets $U = 0$ ([[fw-h]]).
- It is verified on closed forms including a pulsating sphere and a compact dipole ([[fw-h]]).
- The broadband proxies (Proudman for turbulence, Curle for surfaces, trailing edge) are "NOT validated", constants uncalibrated, "not predictions" ([[aeroacoustics]]).
