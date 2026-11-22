---
type: source
tags: [reference, foundation, turbulence, cite-only]
sources: []
origin: https://ocw.mit.edu/courses/2-29-numerical-fluid-mechanics-spring-2015/pages/lecture-notes-and-references/
licence: CC BY-NC-SA 4.0 (course page footer licence link, checked 2026-11-22); some figures marked "excluded from our Creative Commons license"
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# MIT OCW 2.29 Numerical Fluid Mechanics (Spring 2015), Lectures 24-25: turbulent flows

**Origin:** P. F. J. Lermusiaux, *2.29 Numerical Fluid Mechanics*, Spring 2015, MIT OpenCourseWare, lecture slides 24 (`MIT2_29S15_Lecture24.pdf`) and 25 (`MIT2_29S15_Lecture25.pdf`), linked from the course page above. Lecture 23 (listed for turbulence in [[foundations-sources]]) covers finite elements, not turbulence, and was not used.
**Kind:** lecture slides (PDF).
**Licence:** CC BY-NC-SA 4.0 (non-commercial): cite only, not copied into `raw/` (ingest-plan decision 6). Locators are "L24 s.n" / "L25 s.n" for lecture and slide number as printed in the slide footer.
**Read:** 2026-11-22 by job p6-turbulence (text layer of both PDFs, all turbulence slides).

## Summary

A compact lecture treatment of turbulence for CFD: properties of turbulent flow, the energy cascade and Kolmogorov scales, the classification DNS / LES / RANS, Reynolds decomposition and averaging, the Reynolds stresses and the closure problem, eddy viscosity, Prandtl's mixing length, the k and K energy equations, the standard k-epsilon model and its wall functions, Reynolds-stress equation models, and LES filtering with the Smagorinsky subgrid model.

## Key points (own summary)

- **Properties (L24 s.18-19):** turbulent flow is highly unsteady (velocity at a point "appears random"), three-dimensional even when the averaged field is 2D, highly nonlinear (high Re), has high vorticity ("vortex stretching is one of the main mechanisms to maintain or increase the intensity of turbulence"), increases stirring and so mixing, and contains a "wide range of eddies' size"; in a boundary layer the large eddy "has the size of the BL thickness". Ensemble and time averages are equal for a stationary process.
- **Energy cascade and scales (L24 s.21-22):** Richardson's "big whorls have little whorls ..."; with production balancing dissipation, the ratios of largest to Kolmogorov scales are $L/\eta \sim O(Re_L^{3/4})$, $T/\tau \sim O(Re_L^{1/2})$, $U/u_v \sim O(Re_L^{1/4})$; in the inertial sub-range $S(K) = A \varepsilon^{2/3} K^{-5/3}$ with $A \sim 1.5$; dissipation estimated as turbulent energy over turbulent time scale, $\varepsilon \sim u^3/L$; Kolmogorov microscale $\eta = (\nu^3/\varepsilon)^{1/4}$.
- **Method classes (L24 s.23-24):** correlations, integral equations, averaged equations (RANS, "one-(spatial) point closure"), LES ("solves for the largest scales ... only approximates or parameterizes the small scale motions"), DNS ("solves for all scales"); the methods make fewer approximations and cost more in that order.
- **DNS cost (L25 s.4):** grid points per direction $\sim L/\eta \sim O(Re_L^{3/4})$, total 3D cost $\sim O(Re_L^3)$.
- **RANS (L25 s.9-12):** decomposition $u = \bar{u} + u'$ with a moving time average (statistically steady flow) or an ensemble average (unsteady flow); $\overline{u'} = 0$; turbulent kinetic energy $k = \tfrac{1}{2}\left(\overline{u'^2} + \overline{v'^2} + \overline{w'^2}\right)$; "some arbitrariness in the decomposition". Averaging a linear term gives the same term of the mean; derivatives commute with averaging. The averaged incompressible momentum equation keeps the product term $\rho\, \overline{u_i' u_j'}$ (Reynolds stresses) and scalar equations keep $\overline{u_i' \phi'}$; "Equations are thus not closed (more unknown variables than equations)".
- **Reynolds stresses (L25 s.13):** total stress on the mean flow is viscous plus $-\rho\, \overline{u_i' u_j'}$; for isotropic fluctuations the off-diagonal terms vanish; in a mean shear flow with $du/dy > 0$, a parcel moving up ($v' > 0$) slows its neighbours ($u' < 0$), so $\overline{u'v'} < 0$.
- **Eddy viscosity (L25 s.14):** "Effect of turbulence is to increase stirring/mixing on the mean-fields, hence increase effective viscosity"; $\tau_{ij}^{Re} = -\rho\, \overline{u_i' u_j'} = \mu_t \left(\dfrac{\partial u_i}{\partial x_j} + \dfrac{\partial u_j}{\partial x_i}\right) - \tfrac{2}{3} \rho k \delta_{ij}$; the last term is "required to ensure correct results for the sum of normal stresses"; a scalar $\mu_t$ amounts to an "assumption of isotropic turbulence, which is often inaccurate"; Reynolds analogy: turbulent Prandtl number about 1.
- **Mixing length (L25 s.15-16):** dimensional analysis $\mu_t = C \rho u^* L$; in 2D shear $\mu_t = \rho L^2 \lvert du/dy \rvert$ ("Prandtl's 'mixing length' model"); for a plate Prandtl took $L = \kappa y$, which gives the log law; mixing-length models "agree well with data" in simple 2D flows but "in general turbulence, more than one space and time scale".
- **k-epsilon (L25 s.17-22):** a model needs at least a length and a velocity scale, "hence two PDEs?"; exact $K$ and $k$ equations with production $P_k = -\rho\, \overline{u_i'u_j'}\, \partial u_i/\partial x_j$, which appears with opposite sign in the two, and viscous dissipation $\varepsilon = 2 \nu\, \overline{e_{ij}' e_{ij}'}$; turbulent transport modelled as eddy diffusion $(\mu_t/\sigma_k)\, \partial k/\partial x_j$; in "equilibrium turbulent flows" dissipation balances production, giving $L \sim k^{3/2}/\varepsilon$ and $\mu_t = \rho C_\mu k^2/\varepsilon$; standard (Launder and Spalding 1974) constants $C_\mu = 0.09$, $C_1 = 1.44$, $C_2 = 1.92$, $\sigma_k = 1.0$, $\sigma_\varepsilon = 1.3$; k-epsilon time scales "much shorter than for the mean flow", so they are solved separately with under-relaxation.
- **Walls for k-epsilon (L25 s.22-23):** $k = 0$ at a wall but dissipation is not zero there; at high Re a "wall function" avoids solving k-epsilon at the wall: log law $u^+ = (1/\kappa) \ln n^+ + B$, $\kappa = 0.41$, with $k^{3/2}/\varepsilon \sim L \sim \kappa y$ and $\tau_\text{wall} \sim \rho u^{*2}$ matched "without resolving the viscous sub-layer".
- **Reynolds-stress models (L25 s.25-26):** eddy-viscosity and k-epsilon models assume isotropic turbulence, "which fails in many flows"; RSMs solve transport equations for $\overline{u_i'u_j'}$ and $\overline{u_i' \phi'}$ with production, pressure-strain, dissipation and turbulent-diffusion terms, the unknown ones modelled; "Extra 6 + 1 = 7 PDEs"; isotropic dissipation $\varepsilon_{ij} = \tfrac{2}{3} \varepsilon \delta_{ij}$.
- **LES (L25 s.27-29):** "Instead of time-averaging, LES uses spatial filtering to separate large and small eddies"; filtered velocity $\bar{u}(x,t) = \int G(x, x'; \Delta)\, u(x',t)\, dV'$; $\overline{u_i u_j} \ne \bar{u}_i \bar{u}_j$, so the subgrid-scale stress $\tau_{ij}^{SG} = \rho\left(\overline{u_i u_j} - \bar{u}_i \bar{u}_j\right)$ is modelled, e.g. Smagorinsky, "an eddy viscosity model".

## Contradictions with the wiki

None found.

## Pages this source informs

- [[turbulence]], [[energy-cascade-and-kolmogorov-scales]], [[reynolds-averaging]], [[reynolds-stress]], [[boussinesq-eddy-viscosity]], [[mixing-length-model]], [[two-equation-turbulence-models]], [[reynolds-stress-models]], [[les-and-des]], [[wall-functions]], [[law-of-the-wall]], [[k-epsilon]]
