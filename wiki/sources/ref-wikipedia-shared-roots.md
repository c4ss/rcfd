---
type: source
tags: [reference, foundation, shared-roots, wikipedia, secondary, cite-only]
sources: []
origin: https://en.wikipedia.org/
licence: CC BY-SA 4.0 (page footers; "additional terms may apply"); never copied (maintainer decision 6)
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Wikipedia articles used as a secondary source for the shared roots

**Origin:** English Wikipedia, eleven articles listed below, read on 2026-11-22 (current revisions of that day).
**Kind:** encyclopedia, secondary/tertiary.
**Licence:** CC BY-SA 4.0. By maintainer decision 6 ([[ingest-plan]]) Wikipedia is a cited secondary source for elementary concepts only, never the sole source of a model-level claim, and never copied into `raw/`.
**Read:** 2026-11-22, by job p6-roots. Locators are the article's section headings. Several articles are longer than 100 000 characters; only the first 100 000 characters were read (noted per article).

## Summary

Elementary definitions that the open textbooks read in this job state only in one dimension or not at all: the stress tensor and traction, the strain-rate and spin tensors, the Newtonian constitutive law in tensor form, the continuum assumption and Knudsen regimes, conservation laws in differential and integral form, incompressible flow, the Courant number, Newton's method for systems, the discrete Fourier transform and the power spectral density.

## Key points by article

### "Cauchy stress tensor"
- Traction $T(n)$: force per area on a surface with unit normal $n$; Cauchy's theorem $T(n) = n \cdot \sigma$, $T_j = \sigma_{ij} n_i$. Stresses on three perpendicular planes fix the stress on any plane.
- Normal stress $\sigma_n = \sigma_{ij} n_i n_j$; shear stress magnitude $\tau_n = \sqrt{T \cdot T - \sigma_n^2}$; diagonal components normal, off-diagonal shear.
- Balance of angular momentum gives $\sigma_{ij} = \sigma_{ji}$ (six independent components); couple stresses and some non-Newtonian fluids are exceptions.
- Principal stresses are the eigenvalues of $\sigma$ (invariants $I_1 = \operatorname{tr} \sigma$, $I_2$, $I_3 = \det \sigma$); $\tau_\text{max} = (\sigma_1 - \sigma_3)/2$.
- Split into mean (hydrostatic) part $\pi = I_1/3$ and deviator $s_{ij} = \sigma_{ij} - \pi \delta_{ij}$ (trace zero); von Mises $\sigma_{vM} = \sqrt{3 J_2}$, $J_2 = s_{ij} s_{ji} / 2$.

### "Strain-rate tensor"
- Near a point $v(p + r) \approx v(p) + J r$, $J_{ij} = \partial_j v_i$ (velocity gradient, unit 1/time).
- $L = \nabla v$ splits into the strain-rate tensor $E = (L + L^T)/2$ (stretching and shearing) and the spin tensor $W = (L - L^T)/2$ (rigid rotation, no deformation); the rotation rate is $\omega = \tfrac{1}{2} \nabla \times v$, and $\nabla \times v$ is the vorticity.
- $\operatorname{tr} E = \nabla \cdot v$ is the rate of volume change per unit volume; $E$ splits into a spherical part $\tfrac{1}{3}(\nabla \cdot v)\, \delta_{ij}$ and a traceless deviatoric (shear) part.
- Newton's relation $\tau = \mu\, du/dy$.

### "Navier-Stokes equations" (first 100 000 of 148 716 characters read)
- Cauchy momentum equation, convective form $\rho \left(\dfrac{\partial u}{\partial t} + (u \cdot \nabla) u\right) = -\nabla p + \nabla \cdot \tau + \rho a$, and conservation form with $\nabla \cdot (\rho u \otimes u)$.
- Newtonian fluid: $\sigma = -p I + \lambda (\nabla \cdot u) I + \mu (\nabla u + \nabla u^T)$; with bulk viscosity $\zeta = \lambda + \tfrac{2}{3} \mu$, $\sigma = -[p - \zeta\, \nabla \cdot u]\, I + \mu \left[\nabla u + \nabla u^T - \tfrac{2}{3}(\nabla \cdot u) I\right]$. The Stokes hypothesis $\zeta = 0$ holds for monatomic gases and is "generally incorrect" for other gases and liquids.
- Incompressible: $\nabla \cdot u = 0$ and $\rho \left(\dfrac{\partial u}{\partial t} + (u \cdot \nabla) u\right) = -\nabla p + \mu \nabla^2 u + f$, or divided by $\rho$ with $\nu = \mu / \rho$. The pressure keeps fluid-element volume constant; there is no explicit mechanism for advancing pressure in time; it is determined by the velocity field through the continuity constraint.
- Taking the divergence of the momentum equation gives a pressure Poisson equation $\nabla^2 p = -\rho\, \dfrac{\partial u_i}{\partial x_k} \dfrac{\partial u_k}{\partial x_i}$.

### "Continuum mechanics" (first 100 000 of 105 283 characters read)
- Matter is modelled as continuous: the substance "completely fill[s] the space it occupies", ignoring atomic structure, so properties are smooth functions of position and time.
- Accurate at length scales much larger than inter-atomic distances; validity can be checked through a representative elementary volume and separation of scales.
- General balance in integral form for a quantity $f$ with volume source $h$, surface source $g$: $\dfrac{d}{dt} \displaystyle\int_\Omega f\, dV = \int_{\partial\Omega} f (u_n - v \cdot n)\, dA + \int_{\partial\Omega} g\, dA + \int_\Omega h\, dV$. Differential balances: mass $d\rho/dt + \rho\, \nabla \cdot v = 0$ (material derivative), momentum $\rho\, dv/dt - \nabla \cdot \sigma - \rho b = 0$, angular momentum $\sigma = \sigma^T$, energy $\rho\, de/dt - \sigma : \nabla v + \nabla \cdot q - \rho s = 0$.

### "Knudsen number"
- $Kn = \lambda / L$, mean free path over a representative length. Near or above one the continuum assumption "is no longer a good approximation".
- Regimes (empirical, problem dependent): $Kn < 0.01$ continuum; $0.01$ to $0.1$ slip; $0.1$ to $10$ transitional; $> 10$ free molecular.

### "Conservation law"
- The most general exact conservation law is a continuity equation $\partial \rho/\partial t + \nabla \cdot j = 0$, $j = \rho u$ for continuous motion. With a source term it is a balance equation rather than a conservation law.
- Conservative (current-density) forms $y_t + \nabla \cdot j(y) = 0$, systems $y_t + \nabla \cdot J(y) = 0$ with Jacobian $A = dJ/dy$; quasilinear 1D form $y_t + A(y)\, y_x = 0$ is hyperbolic.
- The integral form needs less smoothness than the differential form and admits discontinuous (weak) solutions.

### "Incompressible flow"
- Incompressible flow: the material derivative of density vanishes, $D\rho/Dt = 0$. From continuity $D\rho/Dt = -\rho\, \nabla \cdot u$, so this is equivalent to $\nabla \cdot u = 0$. $\partial \rho/\partial t$ at a fixed point need not vanish.
- Incompressible flow differs from an incompressible (homogeneous) material: the latter always gives incompressible flow, not conversely.
- Compressibility $\beta = (1/\rho)\, d\rho/dp$; if acceptably small the flow is treated as incompressible. The low-Mach (pseudo-incompressible) form assumes a Mach number "normally less than 0.3".

### "Courant-Friedrichs-Lewy condition"
- A necessary convergence condition for explicit time marching of (usually hyperbolic) PDEs: the numerical domain of dependence must contain the analytical one.
- 1D Courant number $C = u\, \Delta t/\Delta x \le C_\text{max}$, $C_\text{max}$ typically 1 for explicit solvers; implicit solvers tolerate larger values. n-D: $C = \Delta t \sum (u_i / \Delta x_i)$. From Courant, Friedrichs and Lewy (1928).

### "Newton's method" (first 100 000 of 130 887 characters read)
- $x_{n+1} = x_n - f(x_n)/f'(x_n)$; "The number of correct digits roughly doubles with each step."
- Sec. 4.1: if $f'(\text{root}) \ne 0$ and the start is close enough, convergence is at least quadratic, $\lvert \varepsilon_{n+1} \rvert \le M \varepsilon_n^2$. Multiple roots give linear convergence (sec. 3.3).
- Failure modes (sec. 3): costly derivatives (secant variant, slower), bad starting point, oscillation ($x^3 - 2x + 2$), divergence when $f$ is not differentiable at the root, $f'(x_n) = 0$. Robust codes cap iterations, bracket the root and combine with a safer method.
- Sec. 6.1: for $k$ equations, $J_F(x_n)\, (x_{n+1} - x_n) = -F(x_n)$: solve the linear system rather than form the inverse. More equations than unknowns: Gauss-Newton with $(J^T J)^{-1} J^T$. Damped Newton / line search is not discussed in the part read.

### "Discrete Fourier transform" (first 100 000 of 115 179 characters read)
- $X_k = \sum_{n=0}^{N-1} x_n\, e^{-i 2 \pi k n / N}$ (Eq. 1); inverse with $1/N$ (Eq. 2). Bin $k$ is $k$ cycles per $N$ samples, frequency $f_k = k f_s / N$, resolution $\Delta f = f_s / N = 1/(N\, \Delta t)$ (sec. 1.1).
- Real input: $X_k = \overline{X_{N-k}}$ (sec. 3.17). Parseval: $\sum \lvert x_n \rvert^2 = \tfrac{1}{N} \sum \lvert X_k \rvert^2$ (sec. 3.6).
- Sampling introduces aliasing; truncation introduces leakage; zero-padding samples the spectrum more finely but does not remove leakage (sec. 6.1). FFT algorithms make the DFT practical (intro; $O(N \log N)$ stated in sec. 6.6).

### "Spectral density"
- PSD $S_{xx}(f) = \lim_{T \to \infty} \tfrac{1}{T} \lvert x_T(f) \rvert^2$; by Wiener-Khinchin it is the Fourier transform of the autocorrelation (Definition, Eq. 2-3).
- Units e.g. W/Hz, $\mathrm{V^2/Hz}$, $\mathrm{m^2/Hz}$; integrating the PSD over frequency gives the mean power (variance for a statistical process).
- A single periodogram is noisy; averaging many (e.g. Welch's method) converges to the PSD. One-sided (engineering) vs two-sided (physics) conventions.

## Contradictions with the wiki

None.

## Pages this source informs

[[stress]], [[strain-and-strain-rate]], [[shear-stress]], [[navier-stokes-equations]], [[continuum-hypothesis]], [[conservation-law]], [[incompressible-flow]], [[dimensionless-numbers]], [[newton-raphson-method]], [[fourier-analysis-and-spectra]]
