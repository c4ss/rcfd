---
type: concept
tags: [foundation, multiphase, vof, surface-tension]
sources: [ref-wikipedia-multiphase-heat, ref-lienhard-ahtt-6e, ref-barreiro-villaverde-2021-jet-wiping-vof, ref-sharifi-2024-mules-interface-advection]
builds_on: [two-phase-flow-basics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Surface tension and curvature (Young-Laplace, CSF)

## In short

Surface tension pulls an interface between two fluids tight, like a stretched membrane. Where the interface is curved this pulling produces a pressure jump, larger for smaller drops and bubbles. In VOF the surface force is spread over the interface cells as a body force computed from the volume fraction and its curvature ([[ref-wikipedia-multiphase-heat]] "Young-Laplace equation"; [[ref-barreiro-villaverde-2021-jet-wiping-vof]] sec. II.B).

## Explanation

**Young-Laplace pressure jump.** For a static interface with surface tension $\gamma$ (or $\sigma$) and principal radii of curvature $R_1$, $R_2$:

$$\Delta p = -\gamma\,\nabla\cdot\hat n = -2\gamma H_f = -\gamma\left(\frac1{R_1} + \frac1{R_2}\right)$$ (Young-Laplace, as Wikipedia writes it: $\Delta p$ is the exterior minus the interior pressure, $\hat n$ the outward normal, $H_f$ the mean curvature)

and for a spherical meniscus or drop the magnitude is of radius $R$, $\Delta p = 2\gamma/R$ ([[ref-wikipedia-multiphase-heat]] "Young-Laplace equation"). The same balance fixes the size of a vapour bubble in equilibrium with superheated liquid: $R_b = 2\sigma/[p_{sat}(T_{sup}) - p_{ambient}]$ (Lienhard eq. 9.1); smaller bubbles collapse and larger ones grow ([[ref-lienhard-ahtt-6e]] sec. 9.2). Surface tension is a property of the fluid pair and depends on temperature (Lienhard eq. 9.2a, Table 9.1).

**Curvature from the volume fraction.** On a VOF mesh the interface is not drawn; its unit normal and curvature come from the gradient of $\alpha$ ([[ref-barreiro-villaverde-2021-jet-wiping-vof]] sec. II.B):

$$\vec n = \frac{\nabla\alpha}{|\nabla\alpha|}, \qquad \kappa = -\nabla\cdot\vec n$$ (Barreiro-Villaverde et al., after Eq. 5)

**Continuum surface force (CSF).** Brackbill, Kothe and Zemach (1992) turn the surface force into a volume force acting on the smeared interface ([[ref-barreiro-villaverde-2021-jet-wiping-vof]] sec. II.B):

$$F_\sigma = \sigma\kappa\nabla\alpha$$ (Barreiro-Villaverde et al., Eq. 5)

$\nabla\alpha$ is non-zero only in interface cells, so the force sits where the interface is.

**Spurious (parasitic) currents.** Errors in the computed curvature, or an imbalance between the discrete surface-tension force and the discrete pressure gradient, drive small vortices next to the interface even in a fluid at rest, typically for static bubbles and drops at high density ratios ([[ref-sharifi-2024-mules-interface-advection]] sec. 1; [[ref-wikipedia-multiphase-heat]] "Volume of fluid method", Interface capture issues). Smoothing $\alpha$ before taking the curvature helps up to a point: one filtering step cut the spurious-flow capillary number by 73.31 %, more steps reintroduced errors ([[ref-sharifi-2024-mules-interface-advection]] abstract). Height-function curvature is the alternative rcfd uses (below); none of the sources read here describes it (see Open questions).

## Builds on

- [[two-phase-flow-basics]]: the interface and the one-fluid momentum equation that receives $F_\sigma$.
- Floor: curvature of a surface (first-year calculus).

## Leads to

- [[vof]] (CSF and height-function curvature), [[cavitation-and-mass-transfer-models]] (surface tension in the Rayleigh-Plesset equation), [[nucleation-and-condensation]] (critical radius)

## How rcfd uses it

- rcfd's VOF has CSF surface tension with `curvature_smoothing` (default 2) and, since ADR 0114, `curvature = "height_function"` with a CSF fallback as the default ([[doc-case-schema-multiphase]], [[vof]]).
- In the Hysing rising-bubble study the CSF curvature with a fixed number of smoothing passes was a suspect for the 10 % slow bubble; the opt-in height-function curvature came in ADR 0106 and the CSF fallback in ADR 0108 ([[vof]], [[adr-thread-vof-multiphase-particles]]).

## Open questions

- No readable open source on height-function curvature was found in this job; the method is named on [[vof]] from the ADRs only.
