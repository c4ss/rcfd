---
type: source
tags: [reference, foundation, multiphase, vof, cc-by]
sources: []
origin: https://arxiv.org/abs/2103.16116
licence: CC BY 4.0 (arXiv licence link, read 2026-11-22)
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Barreiro-Villaverde, Gosset and Mendez (2021), jet wiping simulated with algebraic VOF

**Origin:** D. Barreiro-Villaverde, A. Gosset, M. A. Mendez, "On the Dynamics of the Jet Wiping Process: Numerical Simulations and Modal Analysis", arXiv:2103.16116 (submitted to Physics of Fluids; related DOI 10.1063/5.0051451). PDF: https://arxiv.org/pdf/2103.16116
**Kind:** research paper; its sec. II.B is a compact description of an algebraic VOF method with interface compression and CSF surface tension (OpenFOAM `interFoam`).
**Licence:** CC BY 4.0. Copyable, but not copied into `raw/` because the PDF is about 30 MB (figures); the lead may add it.
**Read:** 2026-11-22 by job p6-multiphase-heat: sec. II.A-II.B (Eqs. 1-6) only; the modal analysis was not read.

## Summary

Two-phase LES of a gas jet wiping a liquid film, with a one-fluid VOF formulation: one volume fraction, mixture properties, a compressive term in the volume-fraction equation, MULES limiting for boundedness, and the continuum surface force.

## Key points by section

### Sec. II.B "Numerical methodology"
- The VOF method (Hirt and Nichols) transports a colour function, the liquid volume fraction $\alpha$: 1 in liquid cells, 0 in gas cells, the interface where $0<\alpha<1$. Local properties are volume-weighted: $\rho = \alpha\rho_l + (1-\alpha)\rho_g$, likewise $\mu$ (Eq. 1).
- One set of incompressible Navier-Stokes equations with the mixture properties, gravity and a surface-tension force $F_\sigma$ (Eqs. 2a, 2b); volume-fraction transport $\partial\alpha/\partial t + \nabla\cdot(\alpha\vec U) = 0$ (Eq. 3).
- `interFoam` is algebraic VOF: "this approach does not include a geometrical reconstruction of the interface", unlike geometric VOF with SLIC (Noh and Woodward) or PLIC (Rider and Kothe).
- To limit numerical diffusion of the interface, an artificial relative velocity $\vec U_r = \vec U_l - \vec U_g$ compresses it, using $\vec U = \alpha\vec U_l + (1-\alpha)\vec U_g$: $\partial\alpha/\partial t + \nabla\cdot(\vec U\alpha) + \nabla\cdot(\vec U_r\alpha(1-\alpha)) = 0$ (Eq. 4). Its advantage is low cost and simple implementation.
- MULES (Multidimensional Universal Limiter with Explicit Solution) limits the face fluxes, with a user parameter $C_\alpha$, "to guarantee boundedness and stability"; here 4 MULES correctors and $C_\alpha = 1$, "meaning that there is no additional relaxation of the interface compression".
- CSF (Brackbill et al. 1992) turns the surface force into a volume force on the smeared interface: $F_\sigma = \sigma\kappa\nabla\alpha$ (Eq. 5), with unit normal $\vec n = \nabla\alpha/|\nabla\alpha|$ and curvature $\kappa = -\nabla\cdot\vec n$.
- Time step limited by a flow Courant number of 0.95 and an interface Courant number of 0.2; PISO with two correctors.

## Contradictions with the wiki

None.

## Pages this source informs

[[two-phase-flow-basics]], [[volume-fraction-transport]], [[interface-compression]], [[surface-tension-and-curvature]]
