---
type: analysis
tags: [foundation, p6, map, to-do]
sources: []
question: Which more basic concepts does each rcfd concept page build on, down to first-year calculus and physics?
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Foundations map (P6 step 1)

> [!important] This page is a to-do list, not a set of claims
> Every row below is a *proposed* foundation page with a *drafted* prerequisite list. Nothing here is sourced and nothing here should be cited as fact. The scope lines say what a page should cover, not what is true. Each foundation page, when written, must cite a `ref-*` source page built from an open reference that was actually read (see [[foundations-sources]] for candidates per branch), as the P6 rules in `ingest-plan.md` require. Prerequisite lists may change when the sources are read.

**Question:** Which more basic concepts does each rcfd concept page build on, down to first-year calculus and physics?

## Answer

### How to read the tables

- **Node** is the proposed page name (kebab-case, global across folders). Names written as wikilinks are pages that already exist; names in `backticks` are new.
- **builds_on** lists the node's direct prerequisites (what goes into its `builds_on:` frontmatter). A node may build on nodes in other branches; those are not repeated.
- **Led to from** lists the existing rcfd pages whose "How rcfd uses it" or details need this node (where the `## Builds on` link of the rcfd page will point).
- **Depth stop (floor):** first-year calculus (derivatives, integrals, partial derivatives, Taylor series, ODE basics), first-year linear algebra (vectors, matrices, solving $A x = b$, eigenvalues as a definition) and first-year physics (Newton's laws, energy, pressure, heat, temperature, simple waves). Floor items get no page; nodes that need them say `floor`.
- Shared nodes are listed once, in the branch where most of their users are, or under "Shared roots".

### Shared roots (serve several branches)

| Node | Scope | builds_on | Led to from | Exists |
|---|---|---|---|---|
| `divergence-theorem` | Gauss's theorem turning a volume integral of a divergence into a surface flux; gradient, divergence and curl as used in balance laws | floor | every finite-volume and FEM page via `conservation-law`, `finite-volume-method`, `principle-of-virtual-work` | new |
| `continuum-hypothesis` | Fluids and solids as continua; field variables (density, velocity, temperature) at a point | floor | [[structural-solver]], every flow page via `navier-stokes-equations` | new |
| `conservation-law` | Integral and differential balance of mass, momentum, energy over a control volume; fluxes and sources | `divergence-theorem`, `continuum-hypothesis` | [[dynamic-mesh]] (GCL), [[vof]], [[lagrangian-particles]] (two-way sources) | new |
| `stress` | Stress tensor, traction, normal and shear components, symmetry of the stress tensor | `continuum-hypothesis`, floor | [[structural-solver]], [[k-omega-sst]] (via `shear-stress`), [[component-diagonal]] (viscous flux) | new |
| `strain-and-strain-rate` | Displacement gradient and small strain (solids); velocity gradient, strain-rate and rotation (vorticity) tensors (fluids) | `continuum-hypothesis`, floor | [[structural-solver]], [[k-omega-sst]] (strain $S$), [[spalart-allmaras]] (vorticity) | new |
| `viscosity` | Newtonian fluid, dynamic and kinematic viscosity, the constitutive law stress = f(strain rate) | `stress`, `strain-and-strain-rate` | [[k-epsilon]], [[lagrangian-particles]] ($\tau_p$), [[piso]] (diffusion number $\nu\,\Delta t/\Delta x^2$) | new |
| `shear-stress` | Wall and internal shear stress $\tau = \mu\, du/dy$; the shared node for turbulence, wall treatment and FEM | `stress`, `viscosity` | [[k-omega-sst]], [[flat-plate]] (Cf), [[component-diagonal]] (`shear` condition) | new |
| `navier-stokes-equations` | Mass and momentum balance of a viscous fluid; convection, diffusion, pressure gradient, body force terms | `conservation-law`, `viscosity`, `stress` | [[simple]], [[piso]], [[mrf]], [[immersed-solid]] | new |
| `incompressible-flow` | $\nabla\cdot\mathbf{u} = 0$, pressure as the constraint force, when density can be taken constant (low Mach) | `navier-stokes-equations` | [[simple]], [[piso]], [[cosimulation-protocol]] (fluid side) | new |
| `dimensionless-numbers` | Reynolds, Mach, Prandtl, Peclet, Courant, Strouhal, Stokes numbers: what each ratio compares | `viscosity`, floor | [[cylinder-re100]], [[lid-driven-cavity]], [[backward-facing-step]], [[lagrangian-particles]] | new |
| `fourier-analysis-and-spectra` | Fourier series and transform, FFT, power spectral density, frequency resolution | floor | [[aeroacoustics]], [[campbell-diagram]], [[fw-h]] (blade-passing frequency) | new |
| `probability-and-statistics-basics` | Mean, variance, standard deviation, normal distribution, covariance, confidence | floor | [[verification-vs-validation]], [[gaussian-process-surrogate]], [[lagrangian-particles]] (random walk) | new |
| `linear-regression-least-squares` | Fitting a model to data by least squares; normal equations; residuals | floor | [[fidelity-management]] (R1, R2), `gradient-reconstruction` (least-squares gradient) | new |
| `newton-raphson-method` | Newton's method for nonlinear equations and systems; Jacobian, tangent, quadratic convergence, line search | floor | [[arc-length-continuation]], [[hyperelastic-materials]], [[incompatible-modes-hex8]] (local Newton) | new |

### Turbulence

| Node | Scope | builds_on | Led to from | Exists |
|---|---|---|---|---|
| `turbulence` | Chaotic, three-dimensional, multiscale motion at high Reynolds number; laminar to turbulent transition | `navier-stokes-equations`, `dimensionless-numbers` | [[k-omega-sst]], [[k-epsilon]], [[spalart-allmaras]] | new |
| `energy-cascade-and-kolmogorov-scales` | Production at large scales, transfer, dissipation at small scales; meaning of $k$ and $\varepsilon$ | `turbulence` | [[k-epsilon]], [[lagrangian-particles]] (dispersion from $k$, $\varepsilon$) | new |
| `reynolds-averaging` | Mean plus fluctuation decomposition, averaging rules, the RANS equations, the closure problem | `turbulence`, `navier-stokes-equations` | [[k-omega-sst]], [[k-epsilon]], [[spalart-allmaras]], [[adr-thread-transient-rans]] (URANS) | new |
| `reynolds-stress` | The averaged $-\rho\,\langle u'u'\rangle$ tensor, turbulent kinetic energy $k$ as its half trace | `reynolds-averaging`, `stress` | [[k-omega-sst]], [[k-epsilon]] | new |
| `boussinesq-eddy-viscosity` | Reynolds stress modelled as $2 \nu_t S - \tfrac{2}{3} k I$; eddy viscosity as a modelled property, not a fluid property | `reynolds-stress`, `viscosity`, `strain-and-strain-rate` | [[k-omega-sst]], [[k-epsilon]], [[spalart-allmaras]] | new |
| `mixing-length-model` | Prandtl's algebraic $\nu_t = l^2\,\lvert dU/dy\rvert$, $l = \kappa d$; the zero-equation baseline | `boussinesq-eddy-viscosity`, `law-of-the-wall` | [[adr-thread-transient-rans]] (F-040 channel with a mixing-length interior) | new |
| `two-equation-turbulence-models` | Generic transport equation for a turbulence quantity: production, dissipation, diffusion, model constants | `boussinesq-eddy-viscosity`, `energy-cascade-and-kolmogorov-scales`, `convection-diffusion-equation` | [[k-epsilon]], [[k-omega-sst]] | new |
| [[k-epsilon]] | Standard, RNG, realizable variants | `two-equation-turbulence-models`, `wall-functions` | [[backward-facing-step]], [[lagrangian-particles]] | exists |
| `k-omega-wilcox` | Wilcox's k-omega: specific dissipation $\omega$, near-wall behaviour, free-stream sensitivity | `two-equation-turbulence-models` | [[k-omega-sst]] | new |
| `wall-distance` | Distance to the nearest wall and how it is computed (exact, Poisson/eikonal) | floor, `finite-volume-method` | [[k-omega-sst]] ($F_1$, $F_2$), [[spalart-allmaras]] (destruction term) | new |
| `sst-blending-functions` | $F_1$ (k-omega near the wall, k-epsilon outside), $F_2$, the cross-diffusion term | `k-omega-wilcox`, [[k-epsilon]], `wall-distance` | [[k-omega-sst]] | new |
| `bradshaw-assumption-and-sst-limiter` | Shear stress proportional to $k$ in the boundary layer ($a_1$); the $\nu_t = \dfrac{a_1 k}{\max(a_1 \omega,\, S F_2)}$ limiter; production limiter | `boussinesq-eddy-viscosity`, `reynolds-stress`, `boundary-layer` | [[k-omega-sst]] | new |
| [[k-omega-sst]] | Menter SST (2003 form via TMR) | `k-omega-wilcox`, [[k-epsilon]], `sst-blending-functions`, `bradshaw-assumption-and-sst-limiter`, `wall-distance` | [[flat-plate]], [[backward-facing-step]], [[fan-01]] | exists |
| [[spalart-allmaras]] | SA-noft2 with SA-neg terms, DES/DDES | `boussinesq-eddy-viscosity`, `wall-distance`, `convection-diffusion-equation`, `les-and-des` | [[flat-plate]], [[naca0012]] | exists |
| `reynolds-stress-models` | Transporting the Reynolds stress tensor (SSG/LRR, pressure-strain, wall reflection) | `reynolds-stress`, `two-equation-turbulence-models` | [[adr-thread-early-turbulence]] (`rsm`, `ssg`) | new |
| `les-and-des` | Spatial filtering, subgrid-scale eddy viscosity (Smagorinsky, WALE), detached-eddy and delayed DES | `turbulence`, `boussinesq-eddy-viscosity` | [[spalart-allmaras]] (`sa.des`), [[adr-thread-early-turbulence]] | new |
| `boundary-layer` | Thin viscous layer at a wall, displacement and momentum thickness, pressure-gradient effects | `viscosity`, `shear-stress`, `dimensionless-numbers` | [[flat-plate]], [[k-omega-sst]] | new |
| `blasius-solution` | Laminar flat-plate similarity solution, $C_f = 0.664/\sqrt{Re_x}$ as a check value | `boundary-layer`, floor (ODE) | [[flat-plate]] | new |
| `skin-friction-coefficient` | $C_f = \tau_w / (\rho U^2 / 2)$; laminar and turbulent correlations | `shear-stress`, `boundary-layer` | [[flat-plate]], [[spalart-allmaras]] | new |
| `friction-velocity-and-y-plus` | $u_\tau = \sqrt{\tau_w / \rho}$, wall units $y^+$, $u^+$; why the first-cell $y^+$ matters | `shear-stress`, `viscosity`, `boundary-layer` | [[k-omega-sst]] (y+ 3 to 20 range), [[spalart-allmaras]] (wall mode by y+), [[fan-passage-mesh]] (y+ 1 set-ups) | new |
| `law-of-the-wall` | Viscous sublayer, buffer layer, log layer ($\kappa$, $E$), Spalding's single formula | `friction-velocity-and-y-plus`, `turbulence` | [[k-epsilon]] (`WallConstants`), [[spalart-allmaras]] (Spalding law) | new |
| `wall-functions` | Using the law of the wall as a boundary condition at a coarse wall cell; standard and scalable forms | `law-of-the-wall` | [[k-epsilon]], [[spalart-allmaras]] (`auto`, `auto_sa_law`) | new |
| `flow-separation-and-reattachment` | Adverse pressure gradient, separation, recirculation, reattachment length | `boundary-layer` | [[backward-facing-step]], [[naca0012]] (stall range) | new |

### Pressure-velocity coupling and time stepping

| Node | Scope | builds_on | Led to from | Exists |
|---|---|---|---|---|
| `pressure-velocity-coupling` | Why incompressible pressure has no own equation; the saddle-point structure; segregated vs coupled solution | `incompressible-flow`, `finite-volume-method` | [[simple]], [[piso]] | new |
| `pressure-correction-equation` | Deriving a pressure (Poisson-type) equation from continuity and the momentum matrix; velocity and flux correction | `pressure-velocity-coupling`, `momentum-matrix-and-h-operator` | [[simple]], [[piso]] | new |
| `momentum-matrix-and-h-operator` | Discrete momentum $a_P u_P = H(u) - \nabla p$; $\mathrm{HbyA} = H / a_P$; the per-component diagonal | `finite-volume-method`, `sparse-linear-systems` | [[simple]], [[component-diagonal]] | new |
| `collocated-grid-and-checkerboarding` | Storing all variables at cell centres; pressure checkerboard decoupling; staggered grid as the alternative | `finite-volume-method`, `pressure-velocity-coupling` | [[simple]], [[piso]] | new |
| `rhie-chow-interpolation` | Momentum-weighted face flux that couples pressure on collocated grids; dependence on relaxation and time step | `collocated-grid-and-checkerboarding`, `momentum-matrix-and-h-operator` | [[simple]], [[piso]], [[adr-thread-bdf2-step-consistent-face-flux]], [[doc-case-schema-rhie-chow]] | new |
| `under-relaxation` | Explicit and implicit (Patankar) relaxation of iterates; why SIMPLE needs it | `stationary-iterative-methods` | [[simple]], [[piso]] (PIMPLE relaxation) | new |
| [[simple]] | SIMPLE / SIMPLEC (consistent correction, `rAtU`) | `pressure-correction-equation`, `rhie-chow-interpolation`, `under-relaxation` | [[lid-driven-cavity]], [[backward-facing-step]], [[mrf]] | exists |
| `projection-method` | Fractional-step / Chorin projection: predict, project onto divergence-free fields | `pressure-correction-equation`, `implicit-time-integration` | [[piso]] (`fractional_step` remedy) | new |
| [[piso]] | PISO / PIMPLE (correctors, outer iterations, consistent correctors) | [[simple]], `implicit-time-integration`, `stability-and-cfl` | [[dynamic-mesh]], [[sliding-mesh]], [[cylinder-re100]] | exists |
| `block-coupled-solver` | Solving momentum and pressure in one system; Schur complement preconditioning | `pressure-velocity-coupling`, `krylov-methods`, `preconditioning` | [[amg]] (coupled solver V-cycle on the Schur complement) | new |
| `implicit-time-integration` | Implicit Euler, Crank-Nicolson, BDF2 (fixed and variable step); order of accuracy in time | floor (ODE), `truncation-error-and-order-of-accuracy` | [[piso]], [[dynamic-mesh]] (BDF2 opt-in), [[k-epsilon]] (BDF2 order miss), [[overset-mesh]] | new |
| `explicit-time-integration-and-runge-kutta` | Forward Euler, RK2/RK3/RK4, TVD Runge-Kutta, explicit stability limits | floor (ODE), `truncation-error-and-order-of-accuracy` | [[lagrangian-particles]] (integrators), [[adr-thread-compressible-acoustic-boundaries]] (RK3) | new |
| `stability-and-cfl` | Von Neumann stability, Courant number, diffusion number, stiffness | `implicit-time-integration`, `explicit-time-integration-and-runge-kutta`, `finite-difference-method` | [[piso]] (diffusion-number limit), [[divergence-guard]] (Courant limit), [[adr-thread-setup-warnings-and-time-step]] | new |
| [[divergence-guard]] | Run-time checks that stop a diverging run | `stability-and-cfl` | — | exists |
| [[component-diagonal]] | `averaged` vs `corrected` momentum diagonal | `momentum-matrix-and-h-operator`, `boundary-conditions` | [[simple]], [[piso]], [[vof]], [[mrf]] | exists |

### Discretisation, gradients, limiters

| Node | Scope | builds_on | Led to from | Exists |
|---|---|---|---|---|
| `truncation-error-and-order-of-accuracy` | Taylor-series error of a discrete operator; order $p$; error falls as $h^p$ | floor (Taylor series) | [[gci-richardson]], [[overset-mesh]] (order checks), [[immersed-solid]] (first order) | new |
| `finite-difference-method` | Difference quotients on grids; the simplest discretisation, model for stability analysis | floor | `stability-and-cfl` | new |
| `finite-volume-method` | Cell balances, face fluxes, face interpolation, conservation by construction, unstructured polyhedral cells | `conservation-law`, `divergence-theorem` | [[simple]], [[piso]], [[mesh-quality]] | new |
| `convection-diffusion-equation` | The scalar transport model problem; Peclet number; boundedness | `conservation-law` | `upwind-and-central-differencing`, [[vof]] (alpha transport) | new |
| `boundary-conditions` | Dirichlet, Neumann, Robin; symmetry, zero-gradient, wall, inlet/outlet roles | floor (PDE basics) | [[component-diagonal]] (`symmetry`), [[spalart-allmaras]] (far-field `freestream`) | new |
| `upwind-and-central-differencing` | First-order upwind, central schemes, numerical diffusion, wiggles | `finite-volume-method`, `convection-diffusion-equation` | [[mrf]] (first-order upwind manufactured swirl), [[k-omega-sst]] (`upwind` on FAN-01) | new |
| `gradient-reconstruction` | Cell gradients by Green-Gauss and least squares; boundary treatment | `finite-volume-method`, `linear-regression-least-squares` | [[adr-thread-gradient-options]], [[doc-case-schema-gradient-options]] | new |
| `linear-upwind-scheme` | Second-order upwind by gradient extrapolation to the face | `upwind-and-central-differencing`, `gradient-reconstruction` | [[k-omega-sst]] (`linearUpwind` diverged for k/omega), [[doc-case-schema-linear-upwind-limiter]] | new |
| `tvd-schemes-and-flux-limiters` | Total variation, Sweby's region, minmod, van Leer, superbee, van Albada; normalised-variable view | `upwind-and-central-differencing`, `linear-upwind-scheme` | [[simple]] (vanLeer limit cycle at Re_h 400), [[vof]] | new |
| `slope-limiters-unstructured` | Barth-Jespersen and Venkatakrishnan limiters on unstructured meshes; convergence stall | `gradient-reconstruction`, `tvd-schemes-and-flux-limiters` | [[mrf]] (limiter cycle on FAN-01), [[fan-01]], [[divergence-guard]] (`barth_jespersen` test) | new |
| `deferred-correction` | Treating higher-order or non-orthogonal parts explicitly and iterating | `finite-volume-method` | [[simple]] (deferred-correction pressure step on mesh W), [[mesh-adaptation]] | new |
| `non-orthogonal-and-skewness-correction` | Face non-orthogonality and skewness, over-relaxed decomposition, correctors | `finite-volume-method`, `gradient-reconstruction`, `deferred-correction` | [[mesh-quality]], [[simple]] (`implicit_non_orth_pressure`) | new |
| [[mesh-quality]] | Non-orthogonality, skewness, aspect and volume ratio as rcfd reports them | `non-orthogonal-and-skewness-correction` | [[fan-passage-mesh]], [[mesh-generators]] | exists |
| `mesh-generation-basics` | Structured vs unstructured, hex/tet/prism/polyhedral cells, Delaunay with exact predicates, inflation layers, octree cores | floor (geometry), `finite-volume-method` | [[mesh-generators]], [[fan-passage-mesh]] | new |
| `a-posteriori-error-indicators` | Gradient, Hessian and recovery (Zienkiewicz-Zhu) indicators; marking cells to refine or coarsen | `gradient-reconstruction`, `truncation-error-and-order-of-accuracy` | [[mesh-adaptation]], `stress-recovery` | new |
| [[mesh-adaptation]] | Hanging-node AMR and directional lattice | `a-posteriori-error-indicators`, `mesh-generation-basics` | — | exists |

### Linear solvers

| Node | Scope | builds_on | Led to from | Exists |
|---|---|---|---|---|
| `sparse-linear-systems` | Matrices from discretisation; sparsity, symmetry, positive definiteness, diagonal dominance, condition number | floor (linear algebra), `finite-volume-method` | [[amg]], [[simple]], [[structural-solver]] | new |
| `direct-solvers-lu-and-cholesky` | Gaussian elimination, pivoting and row interchanges, Cholesky, LDL^T, fill-in, supernodal factors | `sparse-linear-systems` | [[amg]] (coarse dense LU order defect), [[adr-thread-structural-linear-solvers]], [[gaussian-process-surrogate]] (Cholesky) | new |
| `stationary-iterative-methods` | Jacobi, Gauss-Seidel, SOR; smoothing property; convergence by spectral radius | `sparse-linear-systems` | `geometric-multigrid`, `under-relaxation` | new |
| `krylov-methods` | Conjugate gradient (SPD), BiCGStab and GMRES (non-symmetric); residual minimisation | `sparse-linear-systems` | [[amg]] (AMG-CG), [[simple]] (GPU CG/BiCGStab) | new |
| `preconditioning` | Jacobi, incomplete factorisation (ILU, IC, DIC), Chebyshev, block-Jacobi and additive Schwarz | `krylov-methods`, `stationary-iterative-methods` | [[amg]] ("Jacobi or Chebyshev replace DIC/AMG" on GPU), [[doc-case-schema-linear-solver]] | new |
| `geometric-multigrid` | Smoothing, restriction, prolongation, coarse-grid correction, V-cycle | `stationary-iterative-methods` | [[amg]] | new |
| [[amg]] | Smoothed-aggregation AMG | `geometric-multigrid`, `preconditioning`, `direct-solvers-lu-and-cholesky` | [[vof]], [[structural-solver]], [[immersed-solid]] | exists |
| `eigenvalue-solvers` | Power and inverse iteration, Lanczos, subspace iteration, shift-invert Arnoldi, Ritz estimates | `krylov-methods`, floor (eigenvalues) | `modal-analysis`, [[campbell-diagram]] | new |
| `fixed-point-iteration-and-acceleration` | Fixed-point iteration, convergence, Aitken relaxation, least-squares quasi-Newton | `newton-raphson-method` | [[strong-fsi-coupling]], [[cosimulation-protocol]] | new |

### Rotating frames and moving meshes

| Node | Scope | builds_on | Led to from | Exists |
|---|---|---|---|---|
| `rotating-reference-frame` | Velocity and acceleration in a rotating frame; Coriolis and centrifugal terms | floor (Newton's laws, vectors) | [[mrf]], [[campbell-diagram]], `rotordynamics-gyroscopic-effects` | new |
| `relative-and-absolute-velocity-formulations` | Navier-Stokes in a rotating zone written for the relative or the absolute velocity; frame flux | `rotating-reference-frame`, `navier-stokes-equations` | [[mrf]] (absolute $\mathbf{u}$ with $\phi_{rel}$), [[k-omega-sst]] (relative wall velocity) | new |
| [[mrf]] | Multiple reference frame / frozen rotor | `relative-and-absolute-velocity-formulations`, `periodic-and-cyclic-boundaries`, [[simple]] | [[fan-01]], [[f-126]] | exists |
| `frozen-rotor-and-mixing-plane` | Steady rotor-stator interfaces: frozen rotor vs circumferential averaging | `relative-and-absolute-velocity-formulations` | [[mrf]], [[f-126]], [[doc-case-schema-mixing-plane]] | new |
| `periodic-and-cyclic-boundaries` | Translational and rotational periodicity; transforming vectors across a seam | `boundary-conditions`, floor (rotation matrices) | [[mrf]] (cyclic seams), [[sliding-mesh]], [[cyclic-symmetry]], [[adr-thread-coupled-periodic-faces]] | new |
| `ale-formulation` | Arbitrary Lagrangian-Eulerian balance laws; flux relative to moving faces; mesh velocity | `conservation-law`, `finite-volume-method` | [[dynamic-mesh]], [[cosimulation-protocol]] | new |
| `geometric-conservation-law` | Cell volume change equals swept face volumes; why violating it creates spurious sources | `ale-formulation` | [[dynamic-mesh]], [[sliding-mesh]] (GCL closure) | new |
| `mesh-motion-smoothing` | Moving interior points by Laplacian or diffusion-type equations; layering and remeshing as topology changes | `ale-formulation`, floor (Laplace equation) | [[dynamic-mesh]] | new |
| `non-conformal-interfaces` | Faces that do not match across an interface; intersecting and weighting face fluxes | `finite-volume-method`, floor (geometry) | [[sliding-mesh]], [[lagrangian-particles]] (`join_non_conformal`) | new |
| [[sliding-mesh]] | Transient rotor-stator interface | `non-conformal-interfaces`, `geometric-conservation-law`, [[piso]] | [[fan-01]] | exists |
| `overset-interpolation` | Donor search, fringe and hole cells, interpolation weights, mass conservation across the overlap | `finite-volume-method`, floor (interpolation) | [[overset-mesh]] | new |
| [[overset-mesh]] | Chimera mesh in rcfd | `overset-interpolation`, `ale-formulation` | — | exists |
| `immersed-boundary-method` | Imposing a wall inside a non-fitting mesh through forcing; direct vs continuous forcing; order of accuracy | `navier-stokes-equations`, `finite-volume-method` | [[immersed-solid]] | new |
| [[immersed-solid]] | rcfd's F-219 forcing | `immersed-boundary-method` | [[dfg-cylinder-benchmark]] | exists |
| `rigid-body-dynamics-six-dof` | Newton-Euler equations, inertia tensor, quaternion orientation, springs and dampers | floor (Newton's laws), `rotating-reference-frame` | [[dynamic-mesh]] (six-DOF body) | new |
| `added-mass` | Fluid that a moving body must accelerate; why light bodies in dense fluids destabilise loose coupling | `rigid-body-dynamics-six-dof`, `incompressible-flow` | [[strong-fsi-coupling]], [[cosimulation-protocol]] | new |
| `partitioned-fluid-structure-interaction` | Loose vs strong coupling, interface mapping (consistent and conservative), iteration per step | `added-mass`, `fixed-point-iteration-and-acceleration` | [[strong-fsi-coupling]], [[cosimulation-protocol]] | new |
| [[dynamic-mesh]] | Moving mesh and ALE in rcfd | `ale-formulation`, `geometric-conservation-law`, `mesh-motion-smoothing`, `rigid-body-dynamics-six-dof` | — | exists |
| [[strong-fsi-coupling]] | Strong coupling and added mass in rcfd | `partitioned-fluid-structure-interaction` | — | exists |

### Compressible flow and acoustics

| Node | Scope | builds_on | Led to from | Exists |
|---|---|---|---|---|
| `ideal-gas-and-thermodynamics-basics` | Equation of state, $\gamma$, internal energy and enthalpy, isentropic relations | floor | `speed-of-sound-and-mach-number`, `energy-equation`, [[adr-thread-wet-steam]] | new |
| `speed-of-sound-and-mach-number` | $c = \sqrt{\gamma R T}$, Mach number, compressibility effects | `ideal-gas-and-thermodynamics-basics` | [[fw-h]] ($\lvert U\rvert < c_0$), [[doc-case-schema-compressible]] | new |
| `euler-equations` | Inviscid compressible conservation laws in conservative form | `conservation-law`, `ideal-gas-and-thermodynamics-basics` | [[doc-case-schema-compressible]], [[dynamic-mesh]] (`CompressibleSolver::step_ale`) | new |
| `characteristics-and-hyperbolic-systems` | Wave speeds, characteristic variables, Riemann invariants, what boundaries may prescribe | `euler-equations` | [[adr-thread-compressible-acoustic-boundaries]] | new |
| `shock-waves` | Normal shocks, Rankine-Hugoniot relations, entropy condition | `euler-equations` | [[doc-case-schema-compressible]] (shock tube) | new |
| `riemann-problem-and-godunov-method` | Exact Riemann problem, Godunov's finite-volume scheme | `characteristics-and-hyperbolic-systems`, `shock-waves`, `finite-volume-method` | `approximate-riemann-solvers` | new |
| `approximate-riemann-solvers` | Roe, HLL/HLLC, AUSM+ fluxes; wave-speed estimates | `riemann-problem-and-godunov-method` | [[doc-case-schema-compressible]] | new |
| `acoustic-wave-equation` | Linearised Euler equations, the wave equation, plane and spherical waves, impedance | `euler-equations`, `speed-of-sound-and-mach-number` | [[adr-thread-acoustic-wave-solver]], [[aeroacoustics]] | new |
| `non-reflecting-boundaries-and-sponge-layers` | Characteristic and impedance boundaries, absorbing sponge zones, reflection coefficients | `characteristics-and-hyperbolic-systems`, `acoustic-wave-equation` | [[adr-thread-compressible-acoustic-boundaries]], [[aeroacoustics]] | new |
| `greens-function-and-retarded-time` | Free-space Green's function of the wave equation; retarded time; far-field approximation | `acoustic-wave-equation` | [[fw-h]] (retarded time by Newton) | new |
| `acoustic-source-types` | Monopole, dipole, quadrupole sources and their radiation patterns and velocity scaling | `greens-function-and-retarded-time` | [[aeroacoustics]] (Curle, Proudman proxies), [[fw-h]] (thickness, loading, quadrupole) | new |
| `lighthill-acoustic-analogy` | Rearranging the flow equations into a wave equation with a source; Curle's surface extension | `acoustic-wave-equation`, `navier-stokes-equations`, `acoustic-source-types` | [[fw-h]] | new |
| [[fw-h]] | Ffowcs Williams-Hawkings, Formulation 1A | `lighthill-acoustic-analogy`, `greens-function-and-retarded-time` | [[aeroacoustics]] | exists |
| `rotor-tone-noise` | Blade-passing frequency and harmonics, Gutin-type tonal noise of a rotor | `acoustic-source-types`, `fourier-analysis-and-spectra` | [[aeroacoustics]] | new |
| `duct-acoustic-modes` | Modes in circular ducts, cut-on and cut-off, Bessel functions | `acoustic-wave-equation` | [[aeroacoustics]] | new |
| `sound-levels-and-metrics` | SPL, OASPL, dB(A) weighting, octave and third-octave bands, sound power | `fourier-analysis-and-spectra` | [[aeroacoustics]] | new |
| `structural-sound-radiation` | Radiation from vibrating surfaces; baffled plane, radiation efficiency, ERP | `acoustic-wave-equation`, `modal-analysis` | [[aeroacoustics]], [[adr-thread-structural-fatigue-and-radiated-sound]] | new |
| [[aeroacoustics]] | rcfd's fan-noise chain (entity) | [[fw-h]], `rotor-tone-noise`, `sound-levels-and-metrics`, `duct-acoustic-modes` | — | exists |

### Multiphase and particles

| Node | Scope | builds_on | Led to from | Exists |
|---|---|---|---|---|
| `two-phase-flow-basics` | Immiscible fluids, the interface, mixture density and viscosity, interface capturing vs tracking | `navier-stokes-equations` | [[vof]] | new |
| `volume-fraction-transport` | Advecting the volume fraction; boundedness; algebraic vs geometric VOF | `two-phase-flow-basics`, `convection-diffusion-equation`, `tvd-schemes-and-flux-limiters` | [[vof]] | new |
| `interface-compression` | Artificial compression flux that keeps the interface sharp | `volume-fraction-transport` | [[vof]] | new |
| `surface-tension-and-curvature` | Young-Laplace pressure jump, curvature from the volume fraction, continuum surface force, height functions | `two-phase-flow-basics`, floor (curvature) | [[vof]] | new |
| [[vof]] | rcfd's VOF | `volume-fraction-transport`, `interface-compression`, `surface-tension-and-curvature` | — | exists |
| `cavitation-and-mass-transfer-models` | Vapour bubbles, Rayleigh-Plesset basis of Schnerr-Sauer, mixture models | `two-phase-flow-basics`, `ideal-gas-and-thermodynamics-basics` | [[vof]] (ADR 0056), [[doc-case-schema-multiphase-mixture-cavitation]] | new |
| `nucleation-and-condensation` | Phase change in a supersaturated vapour, droplet nucleation and growth (wet steam) | `ideal-gas-and-thermodynamics-basics`, `two-phase-flow-basics` | [[adr-thread-wet-steam]] | new |
| `particle-equation-of-motion` | Drag on a small sphere, Stokes drag, particle response time, Stokes number, drag correlations, the Maxey-Riley terms rcfd omits | floor (Newton's laws), `viscosity`, `dimensionless-numbers` | [[lagrangian-particles]] | new |
| `turbulent-dispersion-random-walk` | Discrete random walk of particles in a turbulent field from $k$ and $\varepsilon$ | `particle-equation-of-motion`, `energy-cascade-and-kolmogorov-scales`, `probability-and-statistics-basics` | [[lagrangian-particles]] (F-255) | new |
| `two-way-coupling` | Momentum, mass and heat exchange between particles and carrier fluid | `particle-equation-of-motion`, `conservation-law` | [[lagrangian-particles]] | new |
| `droplet-evaporation-and-breakup` | d-squared evaporation law, TAB breakup | `particle-equation-of-motion`, `convective-heat-transfer` | [[adr-thread-particle-wall-and-droplets]] | new |
| `particle-wall-interaction` | Restitution, rough-wall rebound, erosion, wall films | `particle-equation-of-motion` | [[lagrangian-particles]], [[adr-thread-particle-wall-and-droplets]] | new |
| [[lagrangian-particles]] | rcfd's particle tracking | `particle-equation-of-motion`, `explicit-time-integration-and-runge-kutta`, `turbulent-dispersion-random-walk`, `two-way-coupling` | — | exists |

### Heat transfer

| Node | Scope | builds_on | Led to from | Exists |
|---|---|---|---|---|
| `heat-conduction` | Fourier's law, thermal conductivity and diffusivity, steady and transient conduction (erfc solution) | floor (heat, temperature) | `energy-equation`, `conjugate-heat-transfer` | new |
| `energy-equation` | Temperature or enthalpy transport: convection, conduction, sources | `conservation-law`, `heat-conduction`, `convection-diffusion-equation` | [[differentially-heated-cavity]], [[doc-case-schema-conjugate-heat-transfer]] | new |
| `convective-heat-transfer` | Heat transfer coefficient, Nusselt and Prandtl numbers, thermal boundary layer | `energy-equation`, `boundary-layer` | [[doc-case-schema-conjugate-heat-transfer]] | new |
| `boussinesq-buoyancy-approximation` | Density variation kept only in the buoyancy term; Rayleigh and Grashof numbers; natural convection (not the eddy-viscosity Boussinesq) | `energy-equation`, `navier-stokes-equations` | [[differentially-heated-cavity]] | new |
| `conjugate-heat-transfer` | Coupled conduction in solids and convection in fluids; interface temperature and flux continuity | `heat-conduction`, `convective-heat-transfer`, `boundary-conditions` | [[doc-case-schema-conjugate-heat-transfer]] | new |
| `thermal-radiation` | Stefan-Boltzmann law, emissivity, view factors, participating media basics | floor | [[doc-case-schema-radiation]] | new |
| `heat-exchanger-effectiveness-ntu` | Effectiveness-NTU method for heat exchangers | `convective-heat-transfer` | [[adr-thread-early-physics-models]] (ADR 0039) | new |
| `chemical-kinetics-arrhenius` | Reaction rates, Arrhenius temperature dependence, stiff chemistry | floor | [[rcfd-reactor]], `viscoelasticity-and-creep` (Arrhenius creep) | new |

### Structural mechanics and FEM

| Node | Scope | builds_on | Led to from | Exists |
|---|---|---|---|---|
| `linear-elasticity` | Hooke's law, Young's modulus, Poisson ratio, Lame, bulk and shear moduli; isotropic and anisotropic (orthotropic, laminated) | `stress`, `strain-and-strain-rate` | [[structural-solver]], [[hyperelastic-materials]] (linearised moduli) | new |
| `equilibrium-equations` | Strong form $\nabla\cdot\boldsymbol{\sigma} + \mathbf{b} = 0$, traction and displacement boundary conditions | `stress`, `divergence-theorem` | `principle-of-virtual-work` | new |
| `principle-of-virtual-work` | Weak form by integration by parts; virtual displacements | `equilibrium-equations`, `divergence-theorem` | `finite-element-method-basics` | new |
| `finite-element-method-basics` | Shape functions, Galerkin method, element stiffness, assembly, boundary conditions | `principle-of-virtual-work`, `sparse-linear-systems` | [[structural-solver]], [[rcfd-fem]] | new |
| `isoparametric-elements-and-quadrature` | Tet4, Tet10, Hex8, Hex20; mapping and Jacobian; Gauss quadrature | `finite-element-method-basics` | [[structural-solver]], [[incompatible-modes-hex8]] | new |
| `element-locking` | Shear and volumetric locking; B-bar, F-bar, selective integration, incompatible modes, hourglass modes | `isoparametric-elements-and-quadrature`, `linear-elasticity` | [[incompatible-modes-hex8]], [[hyperelastic-materials]] (mean-dilatation Hex8) | new |
| `beam-theory` | Euler-Bernoulli and Timoshenko beams; Saint-Venant torsion; section properties, shear centre | `linear-elasticity` | [[beam-warping]] | new |
| `plate-and-shell-theory` | Kirchhoff and Mindlin plates, shell elements, MITC tying, drilling rotations | `beam-theory`, `element-locking` | [[structural-solver]], [[adr-thread-structural-beams-shells]] | new |
| `structural-dynamics` | Mass, damping and stiffness matrices; consistent mass; Rayleigh damping | `finite-element-method-basics`, floor (Newton's laws) | `modal-analysis`, `newmark-and-generalised-alpha` | new |
| `modal-analysis` | Generalised eigenproblem $K \phi = \lambda M \phi$, natural frequencies, mode shapes, rigid-body modes | `structural-dynamics`, `eigenvalue-solvers` | [[campbell-diagram]], [[cyclic-symmetry]], [[mpc-transformation]] | new |
| `newmark-and-generalised-alpha` | Newmark average acceleration, generalised-alpha, numerical dissipation, energy-momentum schemes | `structural-dynamics`, `implicit-time-integration` | [[cosimulation-protocol]] (`DynamicSolid`), [[adr-thread-structural-dynamics-harmonic]] | new |
| `harmonic-and-random-response` | Frequency response, complex amplitudes, modal superposition, PSD response | `modal-analysis`, `fourier-analysis-and-spectra` | [[adr-thread-structural-dynamics-harmonic]] | new |
| `linear-buckling-and-stress-stiffening` | Geometric stiffness, pre-stress, spin softening, critical loads | `modal-analysis`, `finite-strain-kinematics` | [[beam-warping]] (LTB), [[campbell-diagram]] | new |
| `rotordynamics-gyroscopic-effects` | Coriolis (gyroscopic) matrix, forward and backward whirl, critical speeds, bearings | `rotating-reference-frame`, `modal-analysis` | [[campbell-diagram]] | new |
| `multi-point-constraints` | Constraint equations, elimination by transformation, penalty, Lagrange multipliers | `finite-element-method-basics` | [[mpc-transformation]], [[cyclic-symmetry]] | new |
| `substructuring-and-craig-bampton` | Static condensation, superelements, Craig-Bampton reduction, residual vectors | `modal-analysis`, `multi-point-constraints` | [[structural-solver]] | new |
| `finite-strain-kinematics` | Deformation gradient $F$, $J$, Green-Lagrange strain, Piola-Kirchhoff stresses, total Lagrangian form | `strain-and-strain-rate`, `stress` | [[hyperelastic-materials]], [[incompatible-modes-hex8]] (finite strain) | new |
| [[hyperelastic-materials]] | Neo-Hookean, Mooney-Rivlin in rcfd | `finite-strain-kinematics`, `element-locking` | — | exists |
| `nonlinear-fem-newton` | Residual and tangent stiffness, load stepping, follower loads | `finite-strain-kinematics`, `newton-raphson-method`, `finite-element-method-basics` | [[arc-length-continuation]], [[hyperelastic-materials]] | new |
| `stability-of-equilibrium` | Limit points, bifurcation, snap-through, inertia of the tangent | `nonlinear-fem-newton`, floor (eigenvalues) | [[arc-length-continuation]] | new |
| [[arc-length-continuation]] | Path following through limit points | `stability-of-equilibrium`, `nonlinear-fem-newton` | — | exists |
| `plasticity-j2` | Yield surface, J2 flow rule, hardening, return mapping | `linear-elasticity` | [[adr-thread-structural-materials-nonlinear]] | new |
| `viscoelasticity-and-creep` | Prony series, relaxation, Norton creep, temperature dependence | `linear-elasticity`, floor (ODE) | [[adr-thread-structural-creep-and-temperature]] | new |
| `fatigue` | S-N curves, mean-stress corrections, damage accumulation | `stress` | [[adr-thread-structural-fatigue-and-radiated-sound]] | new |
| `fracture-mechanics` | Stress intensity factors, energy release rate, J-integral | `linear-elasticity` | [[adr-thread-structural-fracture]] | new |
| `contact-mechanics` | Gap function, penalty and Lagrange enforcement, rigid contact | `multi-point-constraints`, `nonlinear-fem-newton` | [[adr-thread-structural-contact]] | new |
| `stress-recovery` | Superconvergent patch recovery, nodal stress smoothing | `finite-element-method-basics`, `a-posteriori-error-indicators` | [[structural-solver]] | new |
| [[incompatible-modes-hex8]] | Wilson/Taylor incompatible modes | `element-locking`, `finite-strain-kinematics` | — | exists |
| [[beam-warping]] | Vlasov torsion and LTB | `beam-theory`, `linear-buckling-and-stress-stiffening` | — | exists |
| [[cyclic-symmetry]] | Sector models by harmonic index | `multi-point-constraints`, `modal-analysis`, `periodic-and-cyclic-boundaries`, `fourier-analysis-and-spectra` | — | exists |
| [[campbell-diagram]] | Frequencies against speed | `rotordynamics-gyroscopic-effects`, `modal-analysis` | — | exists |
| [[mpc-transformation]] | rcfd's transformation path | `multi-point-constraints` | — | exists |

### Verification, validation and uncertainty

| Node | Scope | builds_on | Led to from | Exists |
|---|---|---|---|---|
| `code-verification-and-manufactured-solutions` | Checking that code solves its equations: exact solutions, method of manufactured solutions, observed vs formal order | `truncation-error-and-order-of-accuracy` | [[verification-vs-validation]], [[overset-mesh]] (Taylor-Green order) | new |
| `solution-verification` | Estimating a solution's numerical error: iterative, round-off and discretisation error; refinement studies | `truncation-error-and-order-of-accuracy`, `sparse-linear-systems` (residuals) | [[gci-richardson]], [[verification-vs-validation]] | new |
| [[gci-richardson]] | Richardson extrapolation and GCI | `solution-verification`, `truncation-error-and-order-of-accuracy` | [[fan-01]], [[flat-plate]], [[cylinder-re100]] | exists |
| `measurement-uncertainty` | Standard and expanded uncertainty, combining uncertainties, coverage factors, scatter between data sets | `probability-and-statistics-basics` | [[verification-vs-validation]] (band from the measurement's scatter), [[adr-thread-fan01-design-point]] | new |
| `validation-comparison` | Comparing simulation with measurement: validation metrics, model-form error, what "validated" can claim | `measurement-uncertainty`, `solution-verification` | [[verification-vs-validation]], [[validation-data-pack]] | new |
| [[verification-vs-validation]] | rcfd's three kinds of checking | `code-verification-and-manufactured-solutions`, `solution-verification`, `validation-comparison` | — | exists |

### Optimisation and surrogates

| Node | Scope | builds_on | Led to from | Exists |
|---|---|---|---|---|
| `optimisation-basics` | Objective, design variables, constraints, local vs global optima, gradient-free vs gradient-based | floor | [[design-loop]] | new |
| `design-of-experiments` | Full factorial, Latin hypercube, Sobol sequences, discrepancy | `probability-and-statistics-basics` | [[design-loop]], [[parametric-study]], [[gaussian-process-surrogate]] (LHS start) | new |
| `nelder-mead-simplex` | Downhill simplex: reflection, expansion, contraction, shrink; adaptive coefficients | `optimisation-basics` | [[design-loop]] | new |
| `evolutionary-algorithms` | Differential evolution, CMA-ES (covariance adaptation, restarts) | `optimisation-basics`, `probability-and-statistics-basics` | [[design-loop]] | new |
| `multi-objective-optimisation` | Pareto dominance, NSGA-II (non-dominated sorting, crowding, SBX), constraint ranking | `evolutionary-algorithms` | [[design-loop]] | new |
| `multivariate-normal-and-kernels` | Multivariate normal distribution, covariance functions (squared exponential, Matern) | `probability-and-statistics-basics` | [[gaussian-process-surrogate]] | new |
| `maximum-likelihood-estimation` | Fitting model parameters by maximising the likelihood | `probability-and-statistics-basics`, `optimisation-basics` | [[gaussian-process-surrogate]] (length scales, nugget) | new |
| `cross-validation` | Leave-one-out, held-out error, the one-standard-error rule | `probability-and-statistics-basics`, `linear-regression-least-squares` | [[fidelity-management]], [[gaussian-process-surrogate]] | new |
| [[gaussian-process-surrogate]] | Kriging and expected improvement | `multivariate-normal-and-kernels`, `maximum-likelihood-estimation`, `direct-solvers-lu-and-cholesky`, `design-of-experiments` | [[fidelity-management]] | exists |
| `multi-fidelity-modelling` | Screening with cheap models, discrepancy (additive, multiplicative) corrections | [[gaussian-process-surrogate]], `linear-regression-least-squares`, `cross-validation` | [[fidelity-management]] | new |
| [[fidelity-management]] | Screen, confirm, correct | `multi-fidelity-modelling` | — | exists |
| [[design-loop]] | Operating maps, DOE, optimisation | `design-of-experiments`, `nelder-mead-simplex`, `evolutionary-algorithms`, `multi-objective-optimisation` | — | exists |
| [[parametric-study]] | `study` section sweeps | `design-of-experiments` | — | exists |

### Node counts

| Branch | Rows | Existing pages | New nodes |
|---|---|---|---|
| Shared roots | 14 | 0 | 14 |
| Turbulence | 23 | 3 | 20 |
| Pressure-velocity coupling and time stepping | 15 | 4 | 11 |
| Discretisation, gradients, limiters | 16 | 2 | 14 |
| Linear solvers | 9 | 1 | 8 |
| Rotating frames and moving meshes | 19 | 6 | 13 |
| Compressible flow and acoustics | 18 | 2 | 16 |
| Multiphase and particles | 13 | 2 | 11 |
| Heat transfer | 8 | 0 | 8 |
| Structural mechanics and FEM | 32 | 7 | 25 |
| Verification, validation and uncertainty | 6 | 2 | 4 |
| Optimisation and surrogates | 13 | 4 | 9 |
| **Total** | **186** | **33** | **153** |

## Evidence

- Built from the "How rcfd uses it" and details sections of every method concept page in `concepts/` ([[k-omega-sst]], [[k-epsilon]], [[spalart-allmaras]], [[simple]], [[piso]], [[amg]], [[component-diagonal]], [[divergence-guard]], [[mrf]], [[sliding-mesh]], [[dynamic-mesh]], [[overset-mesh]], [[immersed-solid]], [[mesh-adaptation]], [[mesh-quality]], [[cyclic-symmetry]], [[campbell-diagram]], [[mpc-transformation]], [[hyperelastic-materials]], [[incompatible-modes-hex8]], [[arc-length-continuation]], [[beam-warping]], [[strong-fsi-coupling]], [[fw-h]], [[vof]], [[lagrangian-particles]], [[gci-richardson]], [[verification-vs-validation]], [[gaussian-process-surrogate]], [[fidelity-management]], [[design-loop]], [[parametric-study]]) and of the method-like entity pages ([[structural-solver]], [[aeroacoustics]], [[mesh-generators]], [[cosimulation-protocol]], [[flat-plate]]).
- Concept pages left out as project or process terms, not methods: `adr`, `benchmark-descriptor`, `bit-identical`, `brief`, `case-file`, `checkpoint`, `clean-room`, `design-ledger`, `executable-tutorials`, `exit-codes`, `f-item`, `field-store`, `gate`, `hypothesis-record`, `interchange-formats`, `known-miss`, `llm-wiki`, `located-error`, `maintainer-questions`, `mcp-access-class`, `mcp`, `mutation-testing`, `project-journal`, `provenance`, `registered-limit`, `registration`, `report`, `roadmap-proposal`, `roadmap-status`, `run-site`, `run`, `setup-rules`, `solid-case-file`, `sources-read-addendum`, `status-stream`, `stop-file`, `template`, `tolerance-class`, `udf-sandbox`, `unverified`, `view-and-report`, `wave`, `work-streams`.
- Existing rcfd pages appear in the tables as rows marked `exists` so that each one's `builds_on` list is drafted; 33 rows are existing pages.

## Follow-ups

- Step 2: candidate open references per branch are in [[foundations-sources]].
- Before writing pages, check each proposed name against `index.md` again (names are global); none of the new names existed on 2026-11-22.
- Possible merges to decide when writing (fewer, richer pages): `blasius-solution` into `boundary-layer`; `skin-friction-coefficient` into `shear-stress`; `projection-method` into `pressure-correction-equation`; `finite-difference-method` into `truncation-error-and-order-of-accuracy`; `heat-exchanger-effectiveness-ntu` into `convective-heat-transfer`; `fatigue` and `fracture-mechanics` could stay as stubs.
- `ideal-gas-and-thermodynamics-basics` sits in the compressible branch but serves heat transfer and multiphase too.
- Lint target for step 4: every row marked `exists` must reach a shared root through `builds_on`.
