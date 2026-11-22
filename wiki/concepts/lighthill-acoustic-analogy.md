---
type: concept
tags: [foundation, acoustics, lighthill, acoustic-analogy]
sources: [ref-hubbard-1991-nasa-rp-1258-vol1, ref-farassat-2007-nasa-tm-214853]
builds_on: [acoustic-wave-equation, navier-stokes-equations, acoustic-source-types]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Lighthill's acoustic analogy

## In short

Lighthill rearranged the exact equations of fluid motion into a wave equation for the density, with every term that is not plain acoustics moved to the right-hand side as a source. The flow is thus replaced by an equivalent distribution of quadrupole sources sitting in a fluid at rest. The equation is exact, but it only predicts sound once the source term is known from a flow calculation or a model ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 4, pp. 211-216).

## Explanation

**Derivation.** Take the time derivative of the continuity equation, subtract the divergence of the momentum equation, and drop external source terms. The result is the inhomogeneous wave equation ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 4, p. 215):

$$\frac{\partial^2 \rho}{\partial t^2} - c_\infty^2 \nabla^2 \rho = A(\mathbf{x}, t), \qquad A = \frac{\partial^2 T_{ij}}{\partial x_i \partial x_j}$$ (Lilley, eqs. 11-12)

with the Lighthill stress tensor

$$T_{ij} = \rho v_i v_j + (p - \rho c_\infty^2)\,\delta_{ij} - \tau_{ij}$$ (Lilley, eq. 13)

where $\rho v_i v_j$ is the momentum flux (Reynolds stress), $p - \rho c_\infty^2$ measures departure from the acoustic pressure-density relation (entropy and temperature effects), and $\tau_{ij}$ is the viscous stress of the [[navier-stokes-equations]]. The source is a quadrupole ([[acoustic-source-types]]).

**Solution and its meaning.** "Lighthill's equation is exact"; for an unbounded flow its solution is the retarded integral (eq. 14) of [[greens-function-and-retarded-time]]. "The acoustic source distribution $A(\mathbf{y},\tau)$ replaces the actual fluid flow and, moreover, the sources may move, but the fluid in which they are embedded may not": the sources sit in a medium at rest with the constant properties $\rho_\infty$, $c_\infty$ of the surrounding fluid ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 4, p. 215). Farassat puts the same idea this way: the acoustic analogy artificially converts a nonlinear problem into a linear one, and "all the nonlinearities are lumped into the Lighthill stress tensor which is assumed known from near field aerodynamic calculations" ([[ref-farassat-2007-nasa-tm-214853]] sec. 2).

**Far field and the eighth-power law.** In the far field only the component of $T_{ij}$ along the source-observer direction radiates, with amplitude proportional to its second time derivative at emission; ignoring retarded time would make the far-field integral vanish, so the retardation is essential ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 4, pp. 216-217). With turbulence velocity scale $u_0$ and length scale $l_0$, the sound power per unit volume of turbulence is proportional to the eighth power of the flow velocity (p. 218), and the radiated power is a small fraction of the jet's kinetic-energy flux (p. 216). At high Reynolds number the viscous part of $T_{ij}$ is negligible (p. 218).

**Limits.** The theory "is only predictive when the equivalent acoustic source field is known to some good approximation", and it is hard to apply when the sound interacts with the flow (refraction and convection inside the jet), which the analogy hides inside $T_{ij}$ ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 4, pp. 211-212, 219). It assumes no solid boundaries; Curle extended it to flows with solid surfaces (p. 212).

**Extension to moving surfaces.** Ffowcs Williams and Hawkings embedded moving surfaces in the analogy: their equation adds a thickness term and a loading term on the surface to Lighthill's quadrupole volume term ([[ref-farassat-2007-nasa-tm-214853]] sec. 2, eq. 1). That is the basis of [[fw-h]].

**What it adds.** The [[acoustic-wave-equation]] covers sound in a quiet medium; the [[navier-stokes-equations]] cover the flow. Lighthill's rearrangement connects them exactly: flow quantities define a source, the wave equation carries its sound to the observer.

## Builds on

- [[acoustic-wave-equation]]: the wave operator on the left.
- [[navier-stokes-equations]]: continuity and momentum, rearranged; the viscous stress in $T_{ij}$.
- [[acoustic-source-types]]: the source is a quadrupole.

## Leads to

- [[fw-h]], [[aeroacoustics]]

## How rcfd uses it

- rcfd's FW-H (Formulation 1A) computes the surface terms and no volume term; the quadrupole is "small for subsonic tip Mach numbers, not zero" ([[fw-h]]).
- The Proudman broadband proxy (alpha = 0.1, from a secondary source) estimates turbulence noise from a steady RANS field and is "NOT validated" ([[aeroacoustics]], [[doc-acoustics]]).
