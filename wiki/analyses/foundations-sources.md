---
type: analysis
tags: [foundation, p6, sources, licences, to-do]
sources: []
question: Which open references can the P6 foundation pages read and cite, per branch, and which may be copied into raw/?
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Foundations sources (P6 step 2)

> [!important] A reading list, not claims
> This page lists candidate references for the foundation pages of [[foundations-map]]. Nothing was downloaded into `raw/`. No reference here has been read for content yet; a later agent reads it, makes a `ref-<short-name>` source page with locators, and only then cites it. Vendor documentation (ANSYS, Fluent, CFX, STAR-CCM+, COMSOL, Siemens, Autodesk and similar theory guides) is excluded by the clean-room rule and was not searched.

**Question:** Which open references can the P6 foundation pages read and cite, per branch, and which may be copied into `raw/`?

## Answer

### How to read the tables

- **Licence** is what the page itself showed when fetched on 2026-11-22 (**verified**), or what a search result reported without the page being fetched (**reported, re-check**). "Unknown" means no licence statement was found.
- **raw/?** says whether a full copy may go into `raw/` of this public AGPL repository: **copy** only for public domain (US Government work, "Public Use Permitted"), CC0, CC BY and CC BY-SA; **cite** for everything else (non-commercial or no-derivatives licences, arXiv's non-exclusive licence, all rights reserved, unknown). For CC BY-SA the copy must keep attribution and the share-alike licence. NC material stays cite-only until the maintainer decides otherwise.
- **Nodes** are names from [[foundations-map]].
- OpenStax pages also state that the books "may not be used to train large language models without OpenStax's prior written permission" (seen on the Calculus Volume 3 page); reading and citing is not training, but a copy in `raw/` is not proposed.

### Shared roots

| Reference | URL | Licence | Nodes | raw/? |
|---|---|---|---|---|
| OpenStax, *Calculus Volume 3*, sec. 6.8 "The Divergence Theorem" | https://openstax.org/books/calculus-volume-3/pages/6-8-the-divergence-theorem | CC BY-NC-SA 4.0 (verified, page footer) | `divergence-theorem` | cite |
| OpenStax, *University Physics Volume 1*, sec. 14.7 "Viscosity and Turbulence" | https://openstax.org/books/university-physics-volume-1/pages/14-7-viscosity-and-turbulence | CC BY-NC-SA 4.0 (verified, page footer) | `viscosity`, `dimensionless-numbers` (Re), `turbulence` (entry level) | cite |
| Barba and Forsyth, "CFD Python: the 12 steps to Navier-Stokes equations", JOSE 1(9) 21 (2018), doi:10.21105/jose.00021; notebooks github.com/barbagroup/CFDPython | https://jose.theoj.org/papers/10.21105/jose.00021.pdf | CC BY 4.0 text, BSD-3 code (reported, re-check) | `navier-stokes-equations`, `finite-difference-method`, `convection-diffusion-equation`, `pressure-correction-equation` | copy (after re-check) |
| Wikipedia articles (e.g. "Reynolds-averaged Navier-Stokes equations") | https://en.wikipedia.org/wiki/Reynolds-averaged_Navier%E2%80%93Stokes_equations | CC BY-SA 4.0 (verified, footer of the RANS article) | any node, as a tertiary source | copy (attribution + share-alike) |

Wikipedia is tertiary: use it to cross-check a primary or textbook source, not as the only citation of a foundation claim (recommendation, not a registered rule).

### Turbulence

| Reference | URL | Licence | Nodes | raw/? |
|---|---|---|---|---|
| NASA Turbulence Modeling Resource (TMR), site mirror maintained by the TMBWG: `sst.html`, `wilcox.html`, `spalart.html`, `ke-chien.html`, `rsm-ssglrr.html`, `implementrans.html`, `flatplate.html` | https://tmbwg.github.io/turbmodels/ | Pages show no licence; the source repository github.com/TMBWG/turbmodels has LICENSE "CC0 1.0 Universal" (verified). `docs/validation-data.md` already records CC0 for the TMR data files | [[k-omega-sst]], `k-omega-wilcox`, [[spalart-allmaras]], `reynolds-stress-models`, `sst-blending-functions`, `bradshaw-assumption-and-sst-limiter`, `wall-distance` | copy |
| Menter, "Improved two-equation k-omega turbulence models for aerodynamic flows", NASA TM-103975 (1992), NTRS 19930013620 | https://ntrs.nasa.gov/citations/19930013620 | "Work of the US Gov. Public Use Permitted." (verified) | `k-omega-wilcox`, `sst-blending-functions`, `bradshaw-assumption-and-sst-limiter`, [[k-omega-sst]] | copy |
| MIT OCW 2.29 Numerical Fluid Mechanics (Spring 2015), lectures 23-25 (turbulent flow models, turbulence) | https://ocw.mit.edu/courses/2-29-numerical-fluid-mechanics-spring-2015/pages/lecture-notes-and-references/ | CC BY-NC-SA 4.0 (verified, footer link) | `turbulence`, `reynolds-averaging`, `reynolds-stress`, `boussinesq-eddy-viscosity`, `two-equation-turbulence-models`, `les-and-des` | cite |
| L. Davidson, "Fluid mechanics, turbulent flow and turbulence modeling", Chalmers lecture notes | https://research.chalmers.se/en/publication/129778 | unknown (no statement found) | `turbulence`, `reynolds-averaging`, `law-of-the-wall`, `boundary-layer` | cite |
| Wikipedia: "Reynolds-averaged Navier-Stokes equations", "K-epsilon turbulence model", "Spalart-Allmaras turbulence model", "Law of the wall" | https://en.wikipedia.org/wiki/Reynolds-averaged_Navier%E2%80%93Stokes_equations | CC BY-SA 4.0 (verified on the RANS article only) | `reynolds-averaging`, `reynolds-stress`, [[k-epsilon]], `law-of-the-wall` | copy |

Cited by the repo, no open copy found: Menter (1994) AIAA J. 32:1598; Menter, Kuntz and Langtry (2003); Wilcox (2006) *Turbulence Modeling for CFD*; Launder and Spalding (1974); Spalart and Allmaras (1992/1994); Shih et al. (1995); Yakhot et al. (1992); Pope (2000) *Turbulent Flows*; Schlichting, *Boundary-Layer Theory*; Grotjans and Menter (1998). The repo's own ADRs mark several of these "not read" or "from memory".

### Pressure-velocity coupling and time stepping

| Reference | URL | Licence | Nodes | raw/? |
|---|---|---|---|---|
| MIT OCW 2.29 (Spring 2015), lectures 14 (finite volume), 15-16 (time marching, stiffness, multistep), 17 (pressure-correction and fractional-step methods) | https://ocw.mit.edu/courses/2-29-numerical-fluid-mechanics-spring-2015/pages/lecture-notes-and-references/ | CC BY-NC-SA 4.0 (verified) | `pressure-velocity-coupling`, `pressure-correction-equation`, `projection-method`, `implicit-time-integration`, `explicit-time-integration-and-runge-kutta`, `stability-and-cfl` | cite |
| Langtangen and Linge, *Finite Difference Computing with PDEs*, Springer TCSE 16 (2017), doi:10.1007/978-3-319-55456-3 | https://link.springer.com/book/10.1007/978-3-319-55456-3 | CC BY 4.0 (reported by the Springer listing in search, re-check) | `implicit-time-integration`, `explicit-time-integration-and-runge-kutta`, `stability-and-cfl`, `finite-difference-method`, `truncation-error-and-order-of-accuracy` | copy (after re-check) |
| Barba and Forsyth, CFD Python (see shared roots) | https://jose.theoj.org/papers/10.21105/jose.00021.pdf | CC BY 4.0 (reported) | `pressure-correction-equation`, `stability-and-cfl` | copy (after re-check) |
| Martinez et al., momentum interpolation (Rhie-Chow variants with SIMPLE, SIMPLEC, PISO) in an open-source code, Politecnico di Milano repository copy | https://re.public.polimi.it/bitstream/11311/1010574/5/MARTJ_OA_01-17.pdf | unknown (fetch refused, HTTP 403) | `rhie-chow-interpolation`, `collocated-grid-and-checkerboarding` | cite |
| Wikipedia: "SIMPLE algorithm", "PISO algorithm", "Backward differentiation formula", "Courant-Friedrichs-Lewy condition" | https://en.wikipedia.org/ | CC BY-SA 4.0 (site licence, verified on one article) | [[simple]], [[piso]], `implicit-time-integration`, `stability-and-cfl` | copy |

Cited by the repo, no open copy found: Patankar (1980) *Numerical Heat Transfer and Fluid Flow*; Patankar, Liu and Sparrow (1977); Ferziger and Peric, *Computational Methods for Fluid Dynamics*; Rhie and Chow (1983); Issa (1986); Van Doormaal and Raithby (1984); Majumdar (1988); Moukalled, Mangani and Darwish (2016); Choi (1999).

### Discretisation, gradients, limiters

| Reference | URL | Licence | Nodes | raw/? |
|---|---|---|---|---|
| B. van Leer, "Progress in multi-dimensional upwind differencing", ICASE 92-43 / NASA-CR-189708, NTRS 19930001620 | https://ntrs.nasa.gov/citations/19930001620 | "Work of the US Gov. Public Use Permitted." (verified) | `upwind-and-central-differencing`, `tvd-schemes-and-flux-limiters`, `riemann-problem-and-godunov-method` | copy |
| Eymard, Gallouet and Herbin, "Finite Volume Methods", arXiv:2610.19748 | https://arxiv.org/abs/2610.19748 | arXiv non-exclusive distribution licence 1.0 (verified) | `finite-volume-method`, `convection-diffusion-equation` | cite |
| MIT OCW 2.29 (Spring 2015), lectures 14 and 20 (finite volume on complex geometries) | https://ocw.mit.edu/courses/2-29-numerical-fluid-mechanics-spring-2015/pages/lecture-notes-and-references/ | CC BY-NC-SA 4.0 (verified) | `finite-volume-method`, `gradient-reconstruction`, `non-orthogonal-and-skewness-correction` | cite |
| H. Jasak, "Error analysis and estimation for the finite volume method with applications to fluid flows", PhD thesis, Imperial College 1996, hdl:10044/1/8335 | https://spiral.imperial.ac.uk/handle/10044/1/8335 | conflicting: CC BY-ND 4.0 statement and "Author Permission: Not Granted" both reported (re-check) | `finite-volume-method`, `non-orthogonal-and-skewness-correction`, `tvd-schemes-and-flux-limiters`, `a-posteriori-error-indicators` | cite |
| Wikipedia: "Flux limiter", "Total variation diminishing", "MUSCL scheme" | https://en.wikipedia.org/wiki/Flux_limiter | CC BY-SA 4.0 (site licence) | `tvd-schemes-and-flux-limiters`, `linear-upwind-scheme` | copy |

Cited by the repo, no open copy found: Barth and Jespersen (1989); Venkatakrishnan (1993, 1995); Sweby (1984); Roe (1985); Darwish and Moukalled (2003); Jasak, Weller and Gosman (1999); Gaskell and Lau (1988). A search found no NTRS copy of Barth-Jespersen or Venkatakrishnan.

### Linear solvers

| Reference | URL | Licence | Nodes | raw/? |
|---|---|---|---|---|
| Y. Saad, *Iterative Methods for Sparse Linear Systems*, 2nd ed., author-hosted PDF | https://www-users.cse.umn.edu/~saad/IterMethBook_2ndEd.pdf | copyright SIAM 2003, free author copy (reported); all rights reserved | `sparse-linear-systems`, `stationary-iterative-methods`, `krylov-methods`, `preconditioning`, `eigenvalue-solvers` | cite |
| J. R. Shewchuk, "An Introduction to the Conjugate Gradient Method Without the Agonizing Pain", CMU-CS-94-125 (1994) | https://www.cs.cmu.edu/~quake-papers/painless-conjugate-gradient.pdf | author notice permitting free non-commercial duplication with the notice intact (reported; the PDF could not be read by the fetch tool) | `krylov-methods` | cite (until verified) |
| R. D. Falgout, "An Introduction to Algebraic Multigrid", UCRL-JRNL-220851, Computing in Science and Engineering 8(6) (2006) 24-33, OSTI 897960 | https://www.osti.gov/biblio/897960 | unknown (DOE-funded journal preprint, no rights statement seen) | `geometric-multigrid`, [[amg]] | cite |
| MIT OCW 2.29 (Spring 2015), lectures 6-8 (iterative methods, Jacobi, Gauss-Seidel, SOR, preconditioning, Krylov) | https://ocw.mit.edu/courses/2-29-numerical-fluid-mechanics-spring-2015/pages/lecture-notes-and-references/ | CC BY-NC-SA 4.0 (verified) | `stationary-iterative-methods`, `krylov-methods`, `preconditioning` | cite |
| Wikipedia: "Conjugate gradient method", "Multigrid method", "LU decomposition", "Newton's method" | https://en.wikipedia.org/wiki/Conjugate_gradient_method | CC BY-SA 4.0 (site licence) | `krylov-methods`, `geometric-multigrid`, `direct-solvers-lu-and-cholesky`, `newton-raphson-method` | copy |

Cited by the repo, no open copy found: Vanek, Mandel and Brezina (1996) Computing 56:179; Vanek, Brezina and Mandel (2001). No open source found for smoothed aggregation itself.

### Rotating frames and moving meshes

| Reference | URL | Licence | Nodes | raw/? |
|---|---|---|---|---|
| D. Cline, *Variational Principles in Classical Mechanics*, ch. 12 "Non-inertial Reference Frames" (LibreTexts) | https://phys.libretexts.org/Bookshelves/Classical_Mechanics/Variational_Principles_in_Classical_Mechanics_(Cline)/12%3A_Non-inertial_Reference_Frames | CC BY-NC-SA 4.0 (verified, page footer) | `rotating-reference-frame`, `rigid-body-dynamics-six-dof` | cite |
| R. L. Meakin, "The Chimera Method of Simulation for Unsteady Three-Dimensional Viscous Flow", NASA-CR-200828 (1996), NTRS 19960050498 | https://ntrs.nasa.gov/citations/19960050498 | "Work of the US Gov. Public Use Permitted." (verified) | `overset-interpolation`, [[overset-mesh]] | copy |
| H. Yao, "Arbitrary Lagrangian-Eulerian (ALE) Formulation: Theory, Numerical Methods, and Applications", Chalmers technical report 2026, doi:10.63959/m2.techreport/2026.1 | https://research.chalmers.se/en/publication/551352 | unknown (no licence on the record page) | `ale-formulation`, `geometric-conservation-law`, `mesh-motion-smoothing` | cite |
| Wikipedia: "Rotating reference frame", "Coriolis force", "Arbitrary Lagrangian Eulerian" | https://en.wikipedia.org/wiki/Rotating_reference_frame | CC BY-SA 4.0 (site licence) | `rotating-reference-frame`, `ale-formulation` | copy |

Cited by the repo, no open copy found: Thomas and Lombard (1979) AIAA J. 17:1030; Demirdzic and Peric (1988); Farhat, Geuzaine and Grandmont (2001); Jasak and Tukovic (2006/2007); Tukovic, Peric and Jasak (2018); Ferziger and Peric (moving grids).

### Compressible flow and acoustics

| Reference | URL | Licence | Nodes | raw/? |
|---|---|---|---|---|
| Ames Research Staff, "Equations, Tables, and Charts for Compressible Flow", NACA Report 1135 (1953), NTRS 19930091059 | https://ntrs.nasa.gov/citations/19930091059 | "Work of the US Gov. Public Use Permitted." (verified) | `ideal-gas-and-thermodynamics-basics`, `speed-of-sound-and-mach-number`, `shock-waves`, `characteristics-and-hyperbolic-systems` | copy |
| NASA Glenn, Beginner's Guide to Aeronautics: "Isentropic Flow Equations" and linked `sound.html`, `normal.html`, `mach.html`, `mass.html`, `conmo.html` | https://www.grc.nasa.gov/www/BGH/isentrop.html | no licence on the page (NASA web page; footer links the NASA disclaimer) | `speed-of-sound-and-mach-number`, `ideal-gas-and-thermodynamics-basics`, `conservation-law` | cite (copy only after checking NASA's media guidelines) |
| F. Farassat, "Derivation of Formulations 1 and 1A of Farassat", NASA/TM-2007-214853, NTRS 20070010579 (already used by the repo, ADR 0013 wave 13) | https://ntrs.nasa.gov/citations/20070010579 | "Work of the US Gov. Public Use Permitted." (verified) | `greens-function-and-retarded-time`, `lighthill-acoustic-analogy`, `acoustic-source-types`, [[fw-h]] | copy |
| H. H. Hubbard (ed.), *Aeroacoustics of Flight Vehicles: Theory and Practice*, Vol. 1 Noise Sources, NASA RP-1258 (1991), NTRS 19920001380 (Vol. 2 Noise Control: NTRS 19920005561, licence not checked) | https://ntrs.nasa.gov/citations/19920001380 | "Work of the US Gov. Public Use Permitted." (verified, Vol. 1) | `rotor-tone-noise`, `acoustic-source-types`, `lighthill-acoustic-analogy`, `duct-acoustic-modes` (Vol. 2) | copy (Vol. 1) |
| van Leer ICASE 92-43 (see discretisation) | https://ntrs.nasa.gov/citations/19930001620 | Public Use Permitted (verified) | `riemann-problem-and-godunov-method`, `approximate-riemann-solvers` | copy |
| Wikipedia: "Riemann solver", "HLL family", "Acoustic analogy", "Sound pressure", "A-weighting" | https://en.wikipedia.org/wiki/Riemann_solver | CC BY-SA 4.0 (site licence) | `approximate-riemann-solvers`, `lighthill-acoustic-analogy`, `sound-levels-and-metrics` | copy |

Cited by the repo, no open copy found: Toro (2009) *Riemann Solvers and Numerical Methods for Fluid Dynamics*; Roe (1981); Harten (1983); Liou (1996); Shu-Osher (1988); Brentner and Farassat (1998); Di Francescantonio (1997); Ffowcs Williams and Hall (1970); Gutin (1936); Lowson (1965); Pierce; Kinsler et al. The repo also links Royal Society pages for Lighthill (1952, rspa.1952.0154) and Curle (1955, rspa.1955.0191); their licence was not checked (cite).

### Multiphase and particles

| Reference | URL | Licence | Nodes | raw/? |
|---|---|---|---|---|
| Jaganathan, Prasath, Govindarajan and Vasan, "The Basset-Boussinesq history force: its neglect, validity, and recent numerical developments", Front. Phys. 11 (2023), doi:10.3389/fphy.2023.1167338 | https://www.frontiersin.org/articles/10.3389/fphy.2023.1167338 | CC BY 4.0 (verified) | `particle-equation-of-motion` (Maxey-Riley terms, history force) | copy |
| Talaei and Garrett, "On the Maxey-Riley equation of motion and its extension to high Reynolds numbers", arXiv:2006.16577 | https://arxiv.org/abs/2006.16577 | arXiv non-exclusive distribution licence 1.0 (verified) | `particle-equation-of-motion` | cite |
| Hysing et al., "Quantitative benchmark computations of two-dimensional bubble dynamics", IJNMF 60 (2009); preprint on the TU Dortmund FeatFlow site and MOX report 23/2008 | https://wwwold.mathematik.tu-dortmund.de/~featflow/en/benchmarks/cfdbenchmarking/bubble.html | unknown (not checked; journal version not open) | `two-phase-flow-basics`, `surface-tension-and-curvature` (benchmark context for [[vof]]) | cite |
| Review of interface-capturing methods (VOF, level set, diffuse interface, front tracking), Int. J. Multiphase Flow (2025), doi:10.1016/j.ijmultiphaseflow.2025.105285 | https://doi.org/10.1016/j.ijmultiphaseflow.2025.105285 | CC BY (reported by search; the publisher page did not load, re-check) | `two-phase-flow-basics`, `volume-fraction-transport`, `interface-compression`, `surface-tension-and-curvature` | copy (after re-check) |
| Wikipedia: "Volume of fluid method", "Stokes' law", "Drag coefficient", "Surface tension" | https://en.wikipedia.org/wiki/Volume_of_fluid_method | CC BY-SA 4.0 (site licence) | `volume-fraction-transport`, `particle-equation-of-motion`, `surface-tension-and-curvature` | copy |

Cited by the repo, no open copy found: Brackbill, Kothe and Zemach (1992); Schnerr and Sauer; Schiller and Naumann; Sommerfeld (rough walls); DNV RP O501.

### Heat transfer

| Reference | URL | Licence | Nodes | raw/? |
|---|---|---|---|---|
| OpenStax *College Physics* 1e, ch. 14 "Heat and Heat Transfer Methods" (LibreTexts copy: 14.5 Conduction, 14.6 Convection, 14.7 Radiation) | https://phys.libretexts.org/Bookshelves/College_Physics/College_Physics_1e_(OpenStax)/14:_Heat_and_Heat_Transfer_Methods | CC BY 4.0 (verified, LibreTexts footer) | `heat-conduction`, `convective-heat-transfer` (entry level), `thermal-radiation` | copy |
| J. H. Lienhard IV and J. H. Lienhard V, *A Heat Transfer Textbook*, 6th ed. (2024) | https://ahtt.mit.edu/ | copyrighted; free download for personal or non-profit instructional use (reported from the site) | `heat-conduction`, `convective-heat-transfer`, `boussinesq-buoyancy-approximation`, `thermal-radiation`, `heat-exchanger-effectiveness-ntu` | cite |
| Wikipedia: "NTU method" (already used by the repo for ADR 0039), "Nusselt number", "Rayleigh number", "Boussinesq approximation (buoyancy)", "Arrhenius equation" | https://en.wikipedia.org/wiki/NTU_method | CC BY-SA 4.0 (site licence) | `heat-exchanger-effectiveness-ntu`, `convective-heat-transfer`, `boussinesq-buoyancy-approximation`, `chemical-kinetics-arrhenius` | copy |

Cited by the repo, no open copy found: Kays and London, *Compact Heat Exchangers*; Incropera; Shah and London; Hausen (1943); Carslaw and Jaeger; de Vahl Davis (1983) tables; Pope (1997) ISAT.

### Structural mechanics and FEM

| Reference | URL | Licence | Nodes | raw/? |
|---|---|---|---|---|
| MIT OCW 2.092/2.093 Finite Element Analysis of Solids and Fluids I (K.-J. Bathe, Fall 2009), lecture notes | https://ocw.mit.edu/courses/2-092-finite-element-analysis-of-solids-and-fluids-i-fall-2009/pages/lecture-notes/ | CC BY-NC-SA 4.0 (verified, footer link) | `principle-of-virtual-work`, `finite-element-method-basics`, `structural-dynamics`, `modal-analysis`, `eigenvalue-solvers`, `nonlinear-fem-newton` | cite |
| MIT OCW RES.2-002 Finite Element Procedures for Solids and Structures (K.-J. Bathe, 2010), linear and nonlinear video lectures and notes | https://ocw.mit.edu/courses/res-2-002-finite-element-procedures-for-solids-and-structures-spring-2010/ | CC BY-NC-SA 4.0 (verified, footer link) | `isoparametric-elements-and-quadrature`, `finite-strain-kinematics`, `nonlinear-fem-newton`, `plasticity-j2`, `newmark-and-generalised-alpha` | cite |
| MIT OCW 3.11 Mechanics of Materials (D. Roylance, 1999), modules; LibreTexts copy "Mechanics of Materials (Roylance)" | https://ocw.mit.edu/courses/3-11-mechanics-of-materials-fall-1999/ | CC BY-NC-SA (reported, re-check) | `stress`, `strain-and-strain-rate`, `linear-elasticity`, `beam-theory`, `fracture-mechanics`, `fatigue`, `viscoelasticity-and-creep` | cite |
| Langtangen and Mardal, *Introduction to Numerical Methods for Variational Problems*, Springer TCSE 21 (2019), doi:10.1007/978-3-030-23788-2 | https://link.springer.com/book/10.1007/978-3-030-23788-2 | CC BY 4.0 (reported from a course copy, not shown on the Springer page; re-check) | `principle-of-virtual-work`, `finite-element-method-basics`, `isoparametric-elements-and-quadrature` | copy (after re-check) |
| Wikipedia: "Linear elasticity", "Finite element method", "Newmark-beta method", "Craig-Bampton method", "J-integral", "Neo-Hookean solid" | https://en.wikipedia.org/wiki/Linear_elasticity | CC BY-SA 4.0 (site licence) | `linear-elasticity`, `finite-element-method-basics`, `newmark-and-generalised-alpha`, `substructuring-and-craig-bampton`, `fracture-mechanics` | copy |

Cited by the repo, no open copy found: Bathe, *Finite Element Procedures* (2nd ed. 2014; only sample pages are free); Zienkiewicz and Zhu (1987, 1992); Hughes (selective integration; with Brezzi on drilling); Simo and Hughes (1998); Simo and Rifai; Simo and Armero; Simo, Taylor and Pister; Wilson, Taylor, Doherty and Ghaboussi (1973); Taylor, Beresford and Wilson (1976); Crisfield (1981, 1991); Riks (1979); Newmark (1959); Chung and Hulbert (1993); Craig and Bampton (1968); Dvorkin and Bathe (1984); Lee and Bathe (2014); Bathe and Wilson (1973); Shih, Moran and Nakamura; Sze, Liu and Lo (2004).

### Verification, validation and uncertainty

| Reference | URL | Licence | Nodes | raw/? |
|---|---|---|---|---|
| NPARC Alliance CFD V&V tutorial (NASA Glenn), "Examining Spatial (Grid) Convergence"; siblings `errors.html`, `tutorial.html`, `bibliog.html` | https://www.grc.nasa.gov/www/wind/valid/tutorial/spatconv.html | no licence on the page (NASA web page, curator J. W. Slater; footer links NASA notices) | `solution-verification`, `truncation-error-and-order-of-accuracy`, [[gci-richardson]] | cite (copy only after checking NASA's guidelines) |
| Oberkampf and Trucano, "Verification and Validation in Computational Fluid Dynamics", SAND2002-0529 (2002), OSTI 793406, doi:10.2172/793406 | https://www.osti.gov/biblio/793406 | unknown (DOE-sponsored report, no rights statement on the record) | `code-verification-and-manufactured-solutions`, `solution-verification`, `validation-comparison`, [[verification-vs-validation]] | cite |
| TMR verification cases (`flatplate.html` with grid-convergence studies) | https://tmbwg.github.io/turbmodels/flatplate.html | CC0 1.0 via the TMBWG repository (verified) | `solution-verification`, [[gci-richardson]] | copy |
| Wikipedia: "Richardson extrapolation", "Measurement uncertainty", "Method of manufactured solutions" | https://en.wikipedia.org/wiki/Richardson_extrapolation | CC BY-SA 4.0 (site licence) | `solution-verification`, `measurement-uncertainty`, `code-verification-and-manufactured-solutions` | copy |

Cited by the repo, no open copy found: Roache (1994) J. Fluids Eng. 116:405 and Roache's book; Celik et al. (2008) J. Fluids Eng. 130:078001. [[gci-richardson]] already records "The Roache and Celik sources have not been read".

### Optimisation and surrogates

| Reference | URL | Licence | Nodes | raw/? |
|---|---|---|---|---|
| N. Hansen, "The CMA Evolution Strategy: A Tutorial", arXiv:1604.00772 (already read by the repo for ADR 0222) | https://arxiv.org/abs/1604.00772 | arXiv non-exclusive distribution licence 1.0 (verified) | `evolutionary-algorithms` | cite |
| P. I. Frazier, "A Tutorial on Bayesian Optimization", arXiv:1807.02811 | https://arxiv.org/abs/1807.02811 | arXiv non-exclusive distribution licence 1.0 (verified) | `multivariate-normal-and-kernels`, `maximum-likelihood-estimation`, [[gaussian-process-surrogate]] (expected improvement) | cite |
| Rasmussen and Williams, *Gaussian Processes for Machine Learning*, MIT Press 2006, chapters online | https://gaussianprocess.org/gpml/chapters | "Copyright 2006 by Massachusetts Institute of Technology", web copy by permission of MIT Press (reported); a library record claims a CC licence (conflicting, re-check) | `multivariate-normal-and-kernels`, `maximum-likelihood-estimation`, `cross-validation`, [[gaussian-process-surrogate]] | cite |
| Martins and Ning, *Engineering Design Optimization*, Cambridge University Press 2022, free PDF | https://mdobook.github.io/ | "(c) Andrew Ning" on the site, no licence (verified) | `optimisation-basics`, `nelder-mead-simplex`, `evolutionary-algorithms`, `multi-objective-optimisation`, `design-of-experiments`, `multi-fidelity-modelling` | cite |
| Wikipedia: "Nelder-Mead method", "Latin hypercube sampling", "Sobol sequence", "Pareto efficiency", "Cross-validation (statistics)" | https://en.wikipedia.org/wiki/Nelder%E2%80%93Mead_method | CC BY-SA 4.0 (site licence) | `nelder-mead-simplex`, `design-of-experiments`, `multi-objective-optimisation`, `cross-validation` | copy |

Cited by the repo, no open copy found: Deb et al. (2002) NSGA-II; Jones, Schonlau and Welch (1998); Auger and Hansen (2005); Nelder and Mead (1965); Gao and Han; Stein (1999).

### Licence counts (rows in the tables above, Wikipedia rows counted once per branch)

| Branch | Refs | Public domain (US Gov) | CC0 | CC BY | CC BY-SA | CC BY-NC-SA | arXiv non-excl. | All rights reserved / free to read | Unknown or not on page | Copyable now (verified) |
|---|---|---|---|---|---|---|---|---|---|---|
| Shared roots | 4 | 0 | 0 | 1 (reported) | 1 | 2 | 0 | 0 | 0 | 1 |
| Turbulence | 5 | 1 | 1 | 0 | 1 | 1 | 0 | 0 | 1 | 3 |
| Pressure-velocity and time stepping | 5 | 0 | 0 | 2 (reported) | 1 | 1 | 0 | 0 | 1 | 1 |
| Discretisation | 5 | 1 | 0 | 0 | 1 | 1 | 1 | 0 | 1 (conflicting) | 2 |
| Linear solvers | 5 | 0 | 0 | 0 | 1 | 1 | 0 | 2 | 1 | 1 |
| Rotating frames and moving meshes | 4 | 1 | 0 | 0 | 1 | 1 | 0 | 0 | 1 | 2 |
| Compressible flow and acoustics | 6 | 4 | 0 | 0 | 1 | 0 | 0 | 0 | 1 | 5 |
| Multiphase and particles | 5 | 0 | 0 | 2 (1 verified, 1 reported) | 1 | 0 | 1 | 0 | 1 | 2 |
| Heat transfer | 3 | 0 | 0 | 1 | 1 | 0 | 0 | 1 | 0 | 2 |
| Structural mechanics and FEM | 5 | 0 | 0 | 1 (reported) | 1 | 3 (1 reported) | 0 | 0 | 0 | 1 |
| Verification and validation | 4 | 0 | 1 | 0 | 1 | 0 | 0 | 0 | 2 | 2 |
| Optimisation and surrogates | 5 | 0 | 0 | 0 | 1 | 0 | 2 | 2 | 0 | 1 |
| **Total** | **56** | **7** | **2** | **7** | **12** | **10** | **4** | **5** | **9** | **23** |

The van Leer report and the TMR site appear in two branches each and are counted in both. "Copyable now" counts only verified public-domain, CC0, CC BY and CC BY-SA rows (Wikipedia included).

## Evidence

- Licences were read from the fetched pages on 2026-11-22: NTRS records for 19930013620, 20070010579, 19920001380, 19930091059, 19930001620 and 19960050498 all show "Work of the US Gov. Public Use Permitted."; the TMBWG/turbmodels repository LICENSE shows "CC0 1.0 Universal"; OCW 2.29, 2.092 and RES.2-002 footers link CC BY-NC-SA 4.0; OpenStax Calculus Volume 3 and University Physics Volume 1 footers state CC BY-NC-SA 4.0; LibreTexts College Physics chapter 14 states CC BY 4.0 and the Cline chapter 12 CC BY-NC-SA 4.0; arXiv 1604.00772, 1807.02811, 2006.16577 and 2610.19748 link the arXiv non-exclusive licence; the Frontiers article states CC BY 4.0; the Wikipedia RANS article footer states CC BY-SA 4.0.
- The repo's existing citations were collected by searching `docs/reference-data.md`, `docs/validation-data.md` and `docs/adr/` (snapshot `2e94e5ef`) for the authors named in the job brief and for the URLs they already link.
- `turbulence.larc.nasa.gov` did not resolve and `turbmodels.larc.nasa.gov` failed its TLS check on 2026-11-22; the TMBWG GitHub Pages mirror carries the same pages (curators with nasa.gov addresses, "Last Updated: 10/04/2024" on `sst.html`), and the repo already cites it.

## Gaps

- **No verified copyable primary source** for: Rhie-Chow interpolation, SIMPLE/SIMPLEC/PISO (only CC BY-NC-SA course notes and Wikipedia); slope limiters on unstructured meshes (Barth-Jespersen, Venkatakrishnan); smoothed-aggregation AMG; MRF / frozen rotor / mixing plane; sliding non-conformal interfaces; immersed boundary methods; the geometric conservation law; VOF interface compression and CSF surface tension (one CC BY review reported, not verified); natural convection, conjugate heat transfer and radiation beyond entry level; FEM (only NC course material, plus one CC BY book reported); GCI per Celik et al. 2008; NSGA-II, Nelder-Mead and kriging (arXiv or all-rights-reserved only).
- **Branches whose copyable sources are only Wikipedia or entry-level texts:** heat transfer, structural mechanics and FEM, optimisation and surrogates, linear solvers.
- **Re-checks needed** before any copy: CFD Python (JOSE), Langtangen and Linge, Langtangen and Mardal, the IJMF 2025 review, Jasak's thesis, Shewchuk's notice, GPML's licence, Roylance's OCW licence.
- **Decision for the maintainer (not taken here):** whether CC BY-NC-SA material (MIT OCW, OpenStax, LibreTexts NC books) may be copied into `raw/` of an AGPL repository, or stays cite-only as this page assumes.

## Follow-ups

- Step 3 agents: start each branch with its copyable rows, make `ref-<short-name>` source pages (e.g. `ref-tmr-sst`, `ref-menter-1992-tm103975`, `ref-farassat-2007-tm214853`, `ref-naca-1135`, `ref-meakin-chimera`, `ref-van-leer-icase-92-43`, `ref-hubbard-rp1258`), and fall back to cite-only references with locators and own summaries.
- Search further, as a follow-up job, on NTRS for ICASE/NASA reports on pressure-velocity coupling, GCL and unstructured limiters, and on arXiv for CC BY reviews (check each paper's licence link).
