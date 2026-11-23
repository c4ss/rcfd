---
type: source
tags: [reference, foundation, pressure-velocity, discretisation, cite-only]
sources: []
origin: https://ocw.mit.edu/courses/2-29-numerical-fluid-mechanics-spring-2015/pages/lecture-notes-and-references/
licence: CC BY-NC-SA 4.0 (course page footer licence link, checked 2026-11-22); some figures marked "excluded from our Creative Commons license"
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# MIT OCW 2.29 Numerical Fluid Mechanics (Spring 2015), Lectures 12 and 14-19: finite differences, stability, finite volumes, incompressible Navier-Stokes

**Origin:** P. F. J. Lermusiaux, *2.29 Numerical Fluid Mechanics*, Spring 2015, MIT OpenCourseWare, lecture slides `MIT2_29S15_Lecture12.pdf`, `...Lecture14.pdf` to `...Lecture19.pdf`, linked from the course page above. The lecture titles on the course page are offset from the PDF contents (the page lists time marching under lecture 15, but the PDF of lecture 15 is about finite volumes); locators here follow the PDFs.
**Kind:** lecture slides (PDF).
**Licence:** CC BY-NC-SA 4.0 (non-commercial): cite only, not copied into `raw/` (ingest-plan decision 6). Locators are "L18 s.5" for lecture and slide number as printed in the slide footer.
**Read:** 2026-11-22 by job p6-pv-discretisation (text layer of the seven PDFs; equations are partly garbled in the text layer, so only legible formulas are quoted). Companion page for the turbulence lectures: [[ref-mit-ocw-2-29-turbulence]].

## Summary

The middle third of the course: grid refinement and error estimation, Fourier analysis and stability of finite-difference schemes (L12); elliptic and parabolic finite differences, explicit and implicit time stepping, Crank-Nicolson, ADI (L14); finite-volume methods from the integral conservation law, surface and volume integral rules, face interpolation (UDS, CDS, QUICK, higher-order and compact schemes) (L15-L16); the incompressible Navier-Stokes equations: variable arrangement, the pressure equation, pressure-correction, projection and fractional-step methods, boundary conditions (L17-L19).

## Key points (own summary)

- **Order of accuracy from grid refinement (L12 s.5):** for a convergent scheme the discretisation error behaves like $O(\Delta x^p)$; with three systematically refined grids the exponent is estimated as $p = \log\left(\dfrac{u_{2\Delta x} - u_{4\Delta x}}{u_{\Delta x} - u_{2\Delta x}}\right) / \log 2$ and the error on the finest grid as $(u_{\Delta x} - u_{2\Delta x})/(2^p - 1)$; checking that $p$ is what the scheme should give is "a good idea" to check code; two or more grids also allow Richardson extrapolation (L12 s.5-8).
- **Stability (L12 s.16-19):** three approaches: heuristic (introduce an isolated error and watch it), the energy method (show a norm stays bounded; works for nonlinear equations and non-periodic boundaries), and von Neumann (Fourier) analysis, which assumes linear problems with periodic boundaries and requires the amplification factor of each mode to have magnitude at most one.
- **CFL condition (L14 s.1, reviewing L13):** "Numerical domain of dependence must include Mathematical domain of dependence"; for first-order linear convection the explicit schemes need $C = c\, \Delta t/\Delta x \le 1$.
- **Explicit diffusion (L14 s.17-18):** forward in time, centred in space for the 1D heat equation, with $r = \kappa\, \Delta t / \Delta x^2$, is conditionally stable with $r \le 0.5$ (von Neumann); in 2D on a uniform grid the criterion becomes $r \le 1/4$ (L14 s.26).
- **Implicit diffusion (L14 s.21-22):** backward in time, centred in space is first order in time, second in space and unconditionally stable; Crank-Nicolson is second order in time and space and unconditionally stable; each gives a linear system per step. ADI splits a 2D step into two half steps with tridiagonal systems (L14 s.27-28).
- **Finite volumes (L15 s.6-10):** finite differences discretise the differential form, finite volumes the integral form over non-overlapping control volumes; the method is conservative locally and globally (summing over all cells the internal surface integrals cancel) and applies directly to unstructured meshes of arbitrary polyhedra. Basic elements: reconstruct the variable in each cell from its average, resolve the two face values into one flux, integrate the fluxes over the surface, integrate sources over the volume, advance in time.
- **Grids (L15 s.11-12):** cell-centred (node at the CV centre, so the nodal value is a second-order approximation of the cell mean) versus face-centred and cell-vertex arrangements; structured (points on intersecting grid lines) versus unstructured (cells identified by number).
- **Integral rules (L15 s.13-20):** midpoint rule for a face integral $F_e \approx f_e S_e$, second order; trapezoid rule, second order; Simpson's rule, fourth order; volume integral $S_P \approx s_P V$, second order (exact for constant or linear $s$). The face value must itself be interpolated to at least the order of the rule. Cell average and centre value differ by a second-order term (L15 s.23).
- **Face interpolation (L16 s.13-18):** upwind (UDS) takes the upstream node; it never oscillates but is first order and its leading truncation error is "diffusive", with numerical diffusion proportional to the mass flux times the spacing. Linear interpolation (CDS) $\phi_e = \phi_E \lambda_e + \phi_P (1 - \lambda_e)$ is second order but "can provide oscillatory solutions". QUICK fits a parabola through two upstream and one downstream node, third order (coefficients 6/8, 3/8, -1/8 on uniform grids), second order overall with the midpoint rule. Large molecules lead to "deferred-correction schemes and/or compact (Pade') schemes"; "Linear Upwind Scheme, Skewed Upwind schemes, Hybrid. Blending schemes to eliminate oscillations at higher order" are listed as further options.
- **CDS for convection in time (L16 s.4):** non-dissipative (imaginary eigenvalues); explicit centred in time and space is stable with $c\, \Delta t/\Delta x \le 1$; implicit in time is unconditionally stable.
- **Pressure term and conservation (L17 s.8-16):** the pressure term "has no analog in the generic conservation equation"; kinetic-energy conservation is a consequence of momentum conservation and requires the discrete divergence and gradient operators to be compatible; Crank-Nicolson can be energy conservative.
- **Variable arrangement (L17 s.17-20):** collocated grids store all variables at the same points; they were out of favour until the 1980s because of "Occurrence of oscillations in the pressure" and "Difficulties with pressure-velocity coupling", and became popular with non-orthogonal grids for complex geometries, where staggered grids need grid-oriented velocity components and curvature terms. Staggered grids evaluate the pressure and diffusion terms without interpolation and can conserve kinetic energy directly.
- **No pressure equation (L18 s.4-5):** the incompressible equations have no independent equation for pressure; mass conservation becomes a kinematic constraint on velocity. Taking the divergence of the momentum equation gives an elliptic Poisson equation for pressure; its Laplacian must use the divergence operator of the continuity equation and the gradient of the momentum equation, so it is "Best to derive pressure equation from discretized momentum/continuity equations".
- **Explicit pressure scheme (L18 s.7-9):** with forward Euler, demanding a discretely divergence-free velocity at the new step gives the Poisson equation $\dfrac{\partial}{\partial x_i}\left(\dfrac{\partial p^n}{\partial x_i}\right) = \dfrac{\partial H_i^n}{\partial x_i}$; then the momentum update is discretely divergence free.
- **Implicit schemes (L18 s.10-15):** with implicit Euler the Poisson and momentum equations must be solved together; three ways: a nonlinear solver per step, linearisation about $t_n$ (a large coupled linear system for velocity and pressure, "Direct solution is not recommended: use an iterative scheme"), or ADI with a local linearisation and a pressure-correction Poisson equation.
- **Steady pressure-correction (L18 s.16-19):** outer iterations update the nonlinear matrix; inner iterations solve the linear systems; the velocity from momentum with the old pressure does not satisfy continuity and is corrected after a Poisson equation for pressure. Variants: "SIMPLE ... Neglects contributions of $u'$ in the pressure equation"; "SIMPLEC: approximate $u'$ in the pressure equation as a function of $p'$ (better)"; "SIMPLER and PISO methods: iterate to obtain $u'$".
- **Projection methods (L18 s.19-22):** "The divergence producing part of the velocity is 'projected out'". Non-incremental (Chorin 1968): no pressure in the predictor. Incremental (Goda 1979): old pressure in the predictor, pressure increment from continuity, "as the SIMPLE method, but without the iterations". Rotational incremental (Timmermans et al. 1996): accounts for $u'$ in the pressure equation.
- **Fractional step (L19 s.7-10):** Kim and Moin (1985): split the step term by term (convection, diffusion, pressure); a Crank-Nicolson example whose splitting error is second order in time; "Main difference with SIMPLE-type time-marching schemes: SIMPLE schemes solve the nonlinear pressure and momentum equations several times per time-step in outer iterations".
- **Artificial compressibility (L19 s.15-19):** add $(1/\beta)\, \partial p/\partial t$ to continuity; large $\beta$ makes the system stiff; most useful for steady problems.
- **Boundary conditions (L19 s.20-22):** no-slip walls are Dirichlet on velocity; at a symmetry plane the shear stress is zero and the normal stress is not; with collocated grids boundary pressure is extrapolated; where the mass flux is specified the pressure-correction gradient is zero; at an outlet with Neumann pressure everywhere "one needs to specify $p$ at a one point to be fixed (or impose a fixed mean $p$)".

## Contradictions with the wiki

None found.

## Pages this source informs

- [[finite-difference-method]], [[truncation-error-and-order-of-accuracy]], [[stability-and-cfl]], [[implicit-time-integration]], [[explicit-time-integration-and-runge-kutta]]
- [[finite-volume-method]], [[upwind-and-central-differencing]], [[deferred-correction]], [[mesh-generation-basics]]
- [[pressure-velocity-coupling]], [[pressure-correction-equation]], [[collocated-grid-and-checkerboarding]], [[projection-method]], [[block-coupled-solver]], [[boundary-conditions]]
