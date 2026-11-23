---
type: source
tags: [reference, foundation, acoustics, fw-h, retarded-time, public-domain]
sources: []
origin: https://ntrs.nasa.gov/citations/20070010579
raw: raw/farassat-2007-nasa-tm-214853.pdf
licence: NTRS determination GOV_PUBLIC_USE_PERMITTED ("Work of the US Gov. Public Use Permitted.", record API read 2026-11-22)
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Farassat (2007), Derivation of Formulations 1 and 1A of Farassat, NASA/TM-2007-214853

**Origin:** F. Farassat, "Derivation of Formulations 1 and 1A of Farassat", NASA/TM-2007-214853, NASA Langley Research Center, March 2007. NTRS 20070010579. PDF: https://ntrs.nasa.gov/api/citations/20070010579/downloads/20070010579.pdf
**Kind:** NASA technical memorandum (tutorial derivation, written as a Mathematica notebook).
**Licence:** US Government work, public use permitted; full copy in [raw/farassat-2007-nasa-tm-214853.pdf](../raw/farassat-2007-nasa-tm-214853.pdf). The repo already cites it for FW-H (ADR 0013).
**Read:** 2026-11-22 by job p6-compressible-acoustics: whole report from the text layer (equations partly garbled in extraction; their structure and numbering are clear). Locators are section, remark and equation numbers.

## Summary

Derives Formulations 1 and 1A, the retarded-time integral solutions of the Ffowcs Williams-Hawkings (FW-H) equation with surface sources only, for a surface moving at subsonic speed. Formulation 1A takes the observer-time derivative analytically and is the one the author recommends for rotor and propeller noise.

## Key points (own summary)

- **Summary / section 1:** Formulations 1 and 1A solve the FW-H equation "with surface sources only when the surface moves at subsonic speed"; 1A (Brentner, NASA TM 87721, 1986) removed the assumption that the blade stays in the disc plane; Formulation 1 needs a numerical observer-time derivative.
- **FW-H equation (section 2, Eq. 1):** $\Box^2 p' = \dfrac{\partial}{\partial t}[\rho_0 v_n \delta(f)] - \dfrac{\partial}{\partial x_i}[p\, n_i \delta(f)] + \dfrac{\partial^2}{\partial x_i \partial x_j}[H(f)\, T_{ij}]$, with $\Box^2$ the wave (D'Alembertian) operator, the moving surface $f(x,t) = 0$ with $\nabla f = n$ (unit outward normal, $f > 0$ outside), $p' = c^2 \rho' = c^2 (\rho - \rho_0)$, $H$ the Heaviside function, $T_{ij}$ the Lighthill stress tensor; the viscous shear on the surface is neglected. "We have artificially converted a nonlinear problem of noise generation by a moving surface to a linear problem by using the acoustic analogy. All the nonlinearities are lumped into the Lighthill stress tensor".
- **Permeable surface (section 2, Eqs. 2-4):** Ffowcs Williams proposed a penetrable (porous, permeable) data surface enclosing the non-negligible quadrupoles; the sources become $\rho_0 U_n$ and $L_i$ with $U_n = (1 - \rho/\rho_0)\, v_n + \rho u_n/\rho_0$ (Eq. 3) and $L_i = p\, \delta_{ij} n_j + \rho u_i (u_n - v_n)$ (Eq. 4).
- **Thickness and loading (Eqs. 5-6):** thickness-noise equation $\Box^2 p'_T = \dfrac{\partial}{\partial t}[\rho_0 v_n \delta(f)]$ and loading-noise equation $\Box^2 p'_L = -\dfrac{\partial}{\partial x_i}[p\, n_i \delta(f)]$. These are "known also as monopole and dipole sources", but the author calls the terminology "misleading for sources on a moving surface because the radiation patterns ... are not similar to those from stationary monopoles and dipoles"; thickness noise equals the loading noise of a uniform surface pressure $\rho_0 c^2$.
- **Free-space Green's function (3.1, Eq. 10):** $G(x,t; y,\tau) = \delta(t - \tau - r/c)/(4 \pi r)$ for $\tau \le t$ and $0$ for $\tau > t$, with $r = \lvert x - y \rvert$; $g = \tau - t + r/c$. $(x, t)$ are observer variables, $(y, \tau)$ source variables. $\partial r/\partial x_i = \hat{r}_i$, $\partial r/\partial y_i = -\hat{r}_i$ (Eq. 11).
- **Collapsing sphere (Remark 3):** for fixed observer, $g = 0$ is a sphere centred at $x$ with radius $c (t - \tau)$ that shrinks to zero as the source time rises to $t$; it is "the characteristic cone of the wave equation with the vertex at (x, t)" pointing to the past; causality rules out the future cone.
- **Retarded (emission) time (3.2, Eqs. 12-24):** solving $\Box^2 p' = Q\, \delta(f)$ with $G$ and a blade-fixed Lagrangian frame $\eta$ gives the Jacobian $dg/d\tau = 1 - M_r$ (Eq. 20), $M_r$ the source Mach number in the radiation direction; $1 - M_r$ is the Doppler factor, positive for a subsonic surface. The emission time solves $\lvert x - y(\eta, \tau) \rvert = c (t - \tau)$ (Eq. 23); for a subsonic source there is only one emission time; for rotating blades it has no closed form and is found numerically (a shooting search back along the source trajectory). For uniform rectilinear motion it has a closed form, with one emission time subsonic and two supersonic (Remark 4).
- **Retarded integral (Eqs. 27-31):** $4 \pi p'(x,t) = \displaystyle\int_{f=0} \left[\dfrac{Q}{r \lvert 1 - M_r \rvert}\right]_\text{ret} dS$ (Eq. 29), where $[\ ]_\text{ret}$ means evaluated at the emission time of each surface point, not simply $\tau = t - r/c$ (Eq. 30).
- **Formulation 1 (3.3, Eqs. 32-39):** thickness noise has the observer-time derivative outside the integral (Eq. 32); loading noise uses the far-field approximation $\partial/\partial x_i \approx -(\hat{r}_i/c)\, \partial/\partial t$ (Eq. 33) made exact via the identity of Eq. 35, giving a far term with $p \cos\theta$ and a near term $1/r^2$.
- **Formulation 1A (3.4, Eqs. 52-68):** $d\tau/dt = 1/(1 - M_r)$ at fixed $(x, \eta)$; taking the observer-time derivative inside gives integrands with source-time derivatives ($dp/d\tau$ is the pressure a blade-fixed transducer measures) and separate near-field ($1/r^2$) and far-field ($1/r$) terms (Eqs. 66-68). Quadrupole formulations of Farassat and Brentner followed from the same observer-space-to-time conversion (Remark 8).
- **Moving observer (3.5, Eq. 73):** the observer $x$ in the formulations is in the frame fixed to the medium; for a microphone on the aircraft, first find where it is in the medium frame at time $t$.
- **Practical advice (section 4, "Concluding Remarks"):** derive the formulation fully; design the emission-time algorithm carefully; in the near field the computed pressure can carry a large constant (DC) term, negative on the suction side and positive on the pressure side.

## Contradictions with the wiki

None found.

## Pages this source informs

[[greens-function-and-retarded-time]], [[lighthill-acoustic-analogy]], [[acoustic-source-types]], [[acoustic-wave-equation]], [[fw-h]]
