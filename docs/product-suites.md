# rcfd product suites backlog: Ansys capabilities outside CFD

Trademarks and independence: all product and company names in this document are trademarks or registered trademarks of their respective owners and are used only to identify the products compared. rcfd is an independent open-source project, not affiliated with, endorsed by or sponsored by any of them.

Status: first-pass backlog, 2026-10-17. This is a log, not a plan. Nothing here is scheduled, nothing is added to `docs/roadmap.json`, and the priority classes are rough sizing hints for later triage. The CFD-class inventory lives in `docs/parity-matrix.md` (F-244..F-267); this file covers the rest of the vendor catalogue, grouped by product category.

Clean-room: all capability names are short paraphrases of public product pages and of one documentation contents page from the vendor's online help (P21, which normally needs a login); the [provenance record](provenance.md) says what was consulted and for what. No vendor text or code was copied; no vendor binary or install was inspected. Candidate algorithms and standards named below are public textbook or published-paper methods, not vendor internals. See `CONTRIBUTING.md` for the clean-room policy that applies to any future implementation.

This page is for whoever triages work beyond CFD. It lists the sources read, a legend, then one entry per suite (S-numbered, grouped by category A to J), each with the same fields: purpose, capabilities, sources read, CFD overlap and dependencies, size and priority, open route and first slice. The cross-suite section at the end gives the shared infrastructure, the dependency summary on CFD and the coverage statement, which says how weak the evidence is for each area; read that before relying on any entry.

## Sources actually read

All fetched with WebFetch, which returns a model-condensed extraction of the page, not raw HTML; the extraction can omit or reword items. Product pages were reached on `ansys.synopsys.com` (the vendor catalogue now redirects there). Release labels (2026 R1 etc.) are as stated by those pages and were not checked against release notes.

| Code | URL | Notes |
|---|---|---|
| P1 | https://ansys.synopsys.com/products | Catalogue index: 16 categories. Product names listed for only 5 of them; the extractor's one-line descriptions were inferred from names, so P1 is evidence of names only |
| P2 | https://ansys.synopsys.com/products/structures/ansys-mechanical | Mechanical feature list |
| P3 | https://ansys.synopsys.com/products/electronics/ansys-hfss | HFSS feature list |
| P4 | https://ansys.synopsys.com/products/optics/ansys-zemax-opticstudio | OpticStudio feature list |
| P5 | https://ansys.synopsys.com/products/structures/ansys-ls-dyna | LS-DYNA feature list |
| P6 | https://ansys.synopsys.com/products/electronics/ansys-maxwell | Maxwell feature list |
| P7 | https://www.synopsys.com/implementation-and-signoff/signoff/redhawk-sc.html | RedHawk-SC feature list (redirect target of the Ansys page) |
| P8 | https://ansys.synopsys.com/products/electronics/ansys-motor-cad | Motor-CAD feature list |
| P9 | https://ansys.synopsys.com/products/electronics/ansys-icepak | Icepak feature list |
| P10 | https://ansys.synopsys.com/products/structures/ansys-sherlock | Sherlock feature list |
| P11 | https://ansys.synopsys.com/products/3d-design/ansys-discovery | Discovery feature list (short) |
| P12 | https://ansys.synopsys.com/products/optics/ansys-speos | Speos feature list |
| P13 | https://ansys.synopsys.com/products/materials | Granta / materials products |
| P14 | https://ansys.synopsys.com/products/embedded-software | SCADE family and TPT |
| P15 | https://ansys.synopsys.com/products/fluids/ansys-rocky | Rocky (particle dynamics) feature list |
| P16 | https://ansys.synopsys.com/products/structures/ansys-motion | Motion (multibody) feature list |
| P17 | https://ansys.synopsys.com/products/digital-twin/ansys-twin-builder | Twin Builder feature list |
| P18 | https://ansys.synopsys.com/products/platform/ansys-optislang | optiSLang feature list |
| P19 | https://ansys.synopsys.com/products/safety-analysis | Safety analysis products |
| P20 | https://ansys.synopsys.com/products/acoustics-analysis | Sound / acoustics products |
| P21 | vendor online help (login required; not linked) | Mechanical APDL Theory Reference 2025 R1: chapter and section headings only, not the theory text |

WebSearch snippets (not full pages) were also seen and are marked "(snippet)" where used:

| Code | Topic | Notes |
|---|---|---|
| Q1 | Additive Suite / Print / Science | Snippets of vendor pages and Mechanical additive help: metal powder-bed process simulation, distortion compensation, build-failure prediction, melt pool / porosity / microstructure, G-code driven modelling in 2026 R1 |
| Q2 | optiSLang + Minerva | Snippets of webinar and blog pages: Minerva described as an enterprise simulation process and data management system; optiSLang as process integration and design optimisation |
| Q3 | EnSight / PyEnSight | Snippets: result visualisation tool with GUI, C/C++/Python store API and gRPC; Python wrapper |

Fetches that failed (HTTP 404, no content obtained): Lumerical FDTD, Additive landing page, Digital Twins category page, Composite PrepPost, Workbench platform page, Autonomous Vehicles category, Workbench meshing user guide. Content for those areas below is general knowledge and marked GK.

## Legend

- Priority class: P2 = natural next step after the CFD core, shares much infrastructure; P3 = substantial standalone suite; P4 = long-term, large, or low leverage for an open-source CFD-first project. Relative to the rest of this file only.
- Open route: whether a first-principles open-source implementation is plausible from public algorithms and standards.
- Roadmap ids are from `docs/roadmap.json` (F-001..F-243); F-244..F-267 are from `docs/parity-matrix.md`.
- GK = general knowledge, no source read for that line.

---

## A. Structures

### S-01 Structural mechanics, implicit FE (linear and nonlinear statics, buckling, thermal-structural)

- Purpose: finite-element stress, deformation and thermal-structural analysis of solid, shell and beam models.
- Capabilities (P2, P21): linear statics; geometric nonlinearity (large strain and rotation, stress stiffening, spin softening, mixed u-P formulations); plasticity with hardening; creep and viscoplasticity; hyperelasticity, viscoelasticity, Mullins effect; gaskets; shape-memory, soil and concrete material models; user-defined materials; pre-stressed analyses; buckling; substructuring; inertia relief; cyclic symmetry; thermal conduction/convection/radiation with load import from CFD or EM; coupled-field (thermoelastic, piezoelectric, thermoelectric, magnetoelastic and others, P21); element birth and death; adaptive nonlinear meshing; journaling and scripting.
- Sources read: P2, P21.
- CFD overlap: F-051 (CHT, thermal side), F-260 (intrinsic structural solver for FSI), F-237 (one-way structural load export), F-139 (co-simulation participant interface), F-025 (sparse assembly), F-026/F-027 (linear solvers, AMG), F-010 (mesh data structure; needs element types beyond polyhedra), F-013 (Gmsh reader), F-070 (VTK writer).
- CFD dependencies: receives pressure and temperature loads from the flow solver (F-237 one-way, F-139/F-260 two-way); mesh motion (F-063) is the receiving end for structural displacements.
- Size/priority: P2 (the one suite with an already-registered CFD hook, F-260).
- Open route: yes. Total/updated Lagrangian finite elements (Bathe, Zienkiewicz textbooks), Newton-Raphson with line search and arc-length, J2 plasticity with radial return (Simo and Hughes), Neo-Hookean/Mooney-Rivlin/Ogden hyperelasticity, mixed u-p elements, Newmark and generalised-alpha integration, Lanczos/LOBPCG eigensolvers, Craig-Bampton reduction (GK for the method list).
- First slice: small-strain linear-elastic tet/hex solid elements on the existing mesh container, direct and CG solve, patch tests and a clamped-plate analytic deflection benchmark (also the F-260 acceptance case).

### S-02 Structural dynamics (modal, harmonic, spectrum/random vibration, transient)

- Purpose: frequency and time response of structures to dynamic loads.
- Capabilities (P2, P21): modal analysis (also pre-stressed); harmonic response; response spectrum; random vibration (PSD); mode-superposition and full transient; damping models; vibroacoustics (see S-19); linear perturbation; MAC and FRF correlation.
- Sources read: P2, P21.
- CFD overlap: F-256 (modal force and forced response), F-138 (harmonic balance, transient blade row), F-024 (time integration).
- CFD dependencies: aeromechanics needs modal data from here (F-256 lists "structural model source" as a dependency).
- Size/priority: P2, built on S-01.
- Open route: yes. Generalised eigenproblem solvers (Lanczos, subspace iteration, shift-invert), Rayleigh damping, modal superposition, PSD response via Miles/Rice formulations, Newmark-beta.
- First slice: modal analysis of a beam and plate against closed-form frequencies, then harmonic response by mode superposition.

### S-03 Contact mechanics

- Purpose: bonded, sliding, frictional and separating contact between bodies.
- Capabilities (P2, P21): bonded contact, sliding/separating contact with optional friction; contact/target element pairs (P21 element-library section); cohesive zone models.
- Sources read: P2, P21.
- CFD overlap: F-215 (thermal contact resistance on interfaces); F-263 shares only the name.
- CFD dependencies: none.
- Size/priority: P3.
- Open route: yes. Penalty, augmented Lagrangian, node-to-segment and mortar discretisation (Wriggers, Laursen), Coulomb friction with return mapping, semi-smooth Newton.
- First slice: 2D Hertz contact (cylinder on plane) against the analytic contact width and pressure.

### S-04 Explicit dynamics, impact, crash, blast

- Purpose: nonlinear fast transient events with an explicit integration scheme.
- Capabilities (P5, P2): explicit and implicit nonlinear structural analysis with switching; impact and crash including occupant safety; Lagrange, Euler and ALE formulations; structured ALE meshing for blast; SPH coupled to FEM and DEM; DEM; an eddy-current EM solver (magnetic forming, induction heating, battery abuse); fluid-structure interaction; incompressible (ICFD) and compressible (CESE) fluid solvers; failure analysis; forming; frequency-domain analysis; isogeometric analysis. Mechanical also lists explicit analysis (P2).
- Sources read: P5, P2.
- CFD overlap: F-060 (compressible solver, for blast/ALE flavour), F-219 (immersed solid), F-249 (DEM), F-056/F-128 (particles), F-229 (6-DOF), F-134 (overset).
- CFD dependencies: coupled Euler-Lagrange FSI uses the compressible solver; SPH overlaps the particle toolkit.
- Size/priority: P3 (large; contact search and failure models dominate).
- Open route: yes. Central-difference explicit integration, hourglass control (Flanagan-Belytschko), Johnson-Cook plasticity and damage, Mie-Gruneisen EOS, penalty contact with spatial hashing, SPH (Monaghan), Courant-limited time step (GK for method list).
- First slice: 3D explicit solid elements with Johnson-Cook, Taylor-bar impact benchmark against published length/diameter ratios.

### S-05 Composites

- Purpose: layered and short-fibre composite modelling and failure.
- Capabilities (P2, P21): short-fibre composite materials; layered composites via a PrepPost-class tool; composite cure simulation; failure criteria section in theory (P21). Further detail GK (PrepPost page fetch failed).
- Sources read: P2, P21 (headings).
- CFD overlap: none. Resin-transfer moulding would combine F-055 (VOF) and F-110 (non-Newtonian laws) with a cure heat source (GK).
- CFD dependencies: none.
- Size/priority: P3.
- Open route: yes. Classical laminate theory, first-order shear deformation shells, Tsai-Wu, Hashin and Puck failure criteria, kinematic (fishnet) draping, cure kinetics (Kamal-Sourour) (GK).
- First slice: classical laminate theory calculator plus layered shell element; Tsai-Wu first-ply-failure on a published laminate example.

### S-06 Fracture and fatigue

- Purpose: crack growth and fatigue life prediction.
- Capabilities (P2, P21, P10, P16): crack and fracture modelling (adaptive crack growth), fatigue life analysis; cohesive zone and Gurson damage models; fatigue from load and stress histories in multibody; solder and board fatigue in S-11.
- Sources read: P2, P21, P10, P16.
- CFD overlap: none directly; F-256 for vibration-driven loads.
- CFD dependencies: none.
- Size/priority: P3.
- Open route: yes. Stress-life, strain-life (Coffin-Manson), rainflow counting (ASTM E1049), Miner damage accumulation, J-integral and interaction integral, extended FEM, Paris law (GK).
- First slice: rainflow counter plus S-N damage post-processor on a stress-history file (no FE needed); then Paris-law growth on an edge-cracked plate.

### S-07 Structural optimisation (topology, shape, parametric, lattice)

- Purpose: find lighter or stiffer designs automatically.
- Capabilities (P2, P11, Q1): topology optimisation; shape optimisation by mesh morphing; parametric optimisation; lattice optimisation for additive parts; topology optimisation in the real-time modelling tool.
- Sources read: P2, P11, Q1 (snippet).
- CFD overlap: F-137 (adjoint sensitivities, shape morphing), F-257 (optimisation drivers), F-095 (parametric studies).
- CFD dependencies: shares the optimiser driver; adjoint shape morphing is the CFD analogue.
- Size/priority: P3, needs S-01.
- Open route: yes. SIMP with density filtering, OC/MMA updates, level-set methods (Bendsoe-Sigmund; published compact educational codes).
- First slice: 2D SIMP compliance minimisation on a rectangular grid (MBB beam benchmark).

### S-08 Multibody dynamics

- Purpose: motion and loads of assemblies of rigid and flexible bodies with joints and contacts.
- Capabilities (P16): rigid-body analysis with joints, constraints, bushings, contacts, user expressions; rigid plus flexible systems; modal flexible bodies from FE mode shapes; flexible bodies without meshing; eigenvalue analysis; load/stress histories for fatigue; drivetrain NVH (gears); chains, belts, tracks; suspension/steering templates; MATLAB and FMI co-simulation; SMP and MPP solvers; bearing libraries; CAD translators.
- Sources read: P16.
- CFD overlap: F-229 (6-DOF), F-063 (dynamic mesh), F-242 (FMI import), F-139 (co-simulation).
- CFD dependencies: FMI and the co-simulation interface; F-229 is a single-body special case.
- Size/priority: P3.
- Open route: yes. Newton-Euler and Lagrange-multiplier DAE formulations, index reduction (Baumgarte, GGL), generalised-alpha for DAEs, floating-frame flexible bodies, FMI 2/3 public standard.
- First slice: planar rigid-body DAE solver with revolute joints (double pendulum, slider-crank) against analytic or published trajectories, exported as an FMI co-simulation unit.

### S-09 Hydrodynamics (offshore and marine)

- Purpose: wave loads and motions of floating structures.
- Capabilities (P2, one line only): offshore/marine analysis through an Aqwa-type tool. Everything further is GK.
- Sources read: P2.
- CFD overlap: F-227 (wave generation and absorption), F-229 (6-DOF), F-055 (VOF).
- CFD dependencies: CFD is the high-fidelity alternative to the potential-flow tool.
- Size/priority: P4.
- Open route: yes (GK). Linear potential-flow boundary element methods, RAO calculation, mooring line models.
- First slice: strip-theory or panel-method RAO for a box barge against published heave response.

### S-10 Additive manufacturing process simulation

- Purpose: predict distortion, residual stress and build failure of printed parts; explore laser powder-bed process parameters.
- Capabilities (Q1 snippet, P13): metal powder-bed process simulation; distortion compensation; build-failure prediction; melt-pool, porosity and microstructure prediction; process-parameter exploration; G-code driven geometry and thermal build control (2026 R1); topology and lattice optimisation; property-process data capture.
- Sources read: Q1 (snippet only), P13.
- CFD overlap: F-212 (solidification and melting), F-051, F-213 (Joule heating), F-055 (VOF/surface tension, melt pool), F-204 (Marangoni), F-110.
- CFD dependencies: melt-pool scale uses CFD solver features.
- Size/priority: P3 (inherent-strain distortion is small; melt-pool scale is large).
- Open route: yes. Inherent-strain method, layer-by-layer element activation with thermo-elastoplastic FE (needs S-01), Rosenthal solution, Goldak double-ellipsoid heat source (GK).
- First slice: moving Goldak heat source on a solid-zone conduction case (reuses the CFD energy equation) validated against Rosenthal's analytic solution.

### S-11 Electronics reliability (PCB life prediction)

- Purpose: predict lifetime of electronic assemblies under thermal cycling, shock, vibration and humidity-driven failure.
- Capabilities (P10): component/board/system life prediction; thermal cycling, drop and shock, vibration, bending, natural frequency, thermal derating, accelerated life, conductive anodic filament risk; 1-D/3-D solder fatigue including BGA with Darveaux and Syed models; IC wear-out; capacitor life; ECAD import producing FE and CFD models; parts and materials libraries; heatsink editor; coatings, potting, underfill; thermal-cycle counting; lifetime curves, reports, warranty estimates; links to Mechanical, LS-DYNA, Icepak.
- Sources read: P10.
- CFD overlap: F-265 (PCB/ECAD thermal import), F-051.
- CFD dependencies: temperature fields from CFD/thermal solver drive wear-out models.
- Size/priority: P3.
- Open route: partial. Physics-of-failure models are public (Arrhenius, Coffin-Manson, Engelmaier, Darveaux, Basquin, Steinberg vibration criterion) (GK); parts libraries are the hard part and licence-limited; ECAD import via IPC-2581 or ODB++ (spec availability varies, GK).
- First slice: rainflow thermal-cycle counter plus Engelmaier solder-joint life estimator as a pure post-processor with published test vectors.

---

## B. Electronics and electromagnetics

### S-12 Low-frequency electromagnetics (magnetostatic, eddy-current, machines, transformers)

- Purpose: magnetic and electric fields in motors, transformers, inductors, busbars and actuators.
- Capabilities (P6, P8, P21): 2D and 3D static, frequency-domain and transient magnetic/electric solvers; motion; A-phi frequency-domain solver for AC loss; electric-machine workflows (hairpin windings, PWM drive excitation); nonlinear, anisotropic, temperature-dependent magnetic materials; adaptive meshing; circuit and system coupling (circuit editor, ROMs, state-space models); wide-bandgap converter and PCB power modelling; optimisation and scripting; GPU sweeps. Motor-CAD-class: fast 2D FEA plus analytical methods, efficiency/loss maps, torque-speed range with field weakening, thermal and drive-cycle simulation, magnetic force extraction for NVH. P21 lists magnetic potentials, stranded coils, inductance, capacitance, Hall effect, magnetoelasticity.
- Sources read: P6, P8, P21.
- CFD overlap: F-213 (electric potential and Joule heating), F-214 (MHD Lorentz force), F-105 (profile inputs), F-124 (regions).
- CFD dependencies: losses become heat sources in thermal/CFD (F-201); forces go to S-01/S-08; machine cooling uses F-051.
- Size/priority: P2 for 2D magnetostatic and transient; P3 for full 3D with motion.
- Open route: yes. Vector-potential A formulation with edge (Nedelec) elements, A-phi and T-Omega eddy-current formulations, moving-band or sliding-mortar interface (cf. F-114), Maxwell stress tensor and virtual-work force, Steinmetz/Bertotti loss separation, Jiles-Atherton hysteresis (GK).
- First slice: 2D nodal-FE magnetostatics with nonlinear B-H, validated on the public TEAM benchmark problems (GK).

### S-13 High-frequency electromagnetics (full-wave, antenna, RF)

- Purpose: Maxwell's equations at RF and microwave frequencies for antennas, waveguides, connectors and packages.
- Capabilities (P3): full-wave 3D frequency-domain FEM with adaptive meshing; mesh-fusion style handling of large assemblies; antenna and installed-antenna/cosite analysis; finite and infinite phased arrays; RF systems and circuits; chip-package-system 3D-IC analysis; encrypted component sharing; multipaction in vacuum; STL import; GPU sweeps; Python API.
- Sources read: P3.
- CFD overlap: F-130 (radiation, distant), F-262 (ROM export), F-103.
- CFD dependencies: none (thermal back-coupling of losses via F-201).
- Size/priority: P3.
- Open route: yes. Vector FEM (Nedelec elements), FDTD (Yee), method of moments, PML, port mode solvers, Floquet ports, S-parameters (Touchstone is a public format) (GK).
- First slice: Yee-grid FDTD with PML producing S-parameters for a rectangular waveguide, checked against analytic cutoff and TE10 behaviour.

### S-14 Signal/power integrity and EMI/EMC (board, package, chip)

- Purpose: signal and power integrity of interconnects, packages and boards; emissions, susceptibility and crosstalk compliance; chip-level power-grid reliability.
- Capabilities (P3, P7): interconnect analysis across IC, package, connector and PCB; broadband 3D power-integrity analysis; rigid-flex PCB meshing; EMI/EMC emission, susceptibility, crosstalk; RF interference via an EMIT-type integration. Chip level (P7): voltage drop and EM/IR signoff for full-chip and multi-die, dynamic voltage-drop analysis, vector-based and vectorless activity, in-rush analysis, electrothermal co-simulation, chip thermal models, statistical electromigration budgets with self-heating, reduced-order hierarchical models, per-bump multi-die analysis, foundry-certified accuracy on advanced nodes.
- Sources read: P3, P7.
- CFD overlap: F-265, F-051, F-213.
- CFD dependencies: electrothermal feedback to the thermal solver.
- Size/priority: P3 for board/package SI/PI (S-parameters, PDN impedance); P4 for foundry-signoff chip-level IR/EM (needs PDK and foundry data that are not public).
- Open route: partial. Transmission-line and planar solvers, cavity models for PDN, Touchstone S-parameters, vector fitting, IBIS (GK); IR-drop via resistive-network solve with Black's equation for electromigration. Foundry-certified signoff is not reproducible from public data.
- First slice: PDN resistive-grid IR-drop solver (reuses sparse CG, F-025/F-026) with Black's-equation electromigration flagging.

### S-15 Electronics thermal (cooling of electronics)

- Purpose: thermal and flow design of electronics, from packages to cabinets.
- Capabilities (P9): GPU/HPC acceleration; liquid cooling, multi-fluid flow, thermoelectric coolers; conduction/convection/radiation, solar load, DC Joule heating, package thermal characterisation; coupled thermal-mechanical; unified 3D interface with MCAD/ECAD exchange and automated meshing; dynamic thermal management, network modelling, ROMs, parametric/optimisation; thermal component library.
- Sources read: P9.
- CFD overlap: F-051, F-052, F-113 (lumped components), F-130 (solar load), F-213, F-217, F-262 (ROM), F-265, F-131/F-132 (meshing).
- CFD dependencies: mostly a task-specific front end on the CFD core; needs component libraries and ECAD import.
- Size/priority: P2 (largest overlap with existing work; a workflow and library layer).
- Open route: yes. Thermal resistance networks (JEDEC JESD51 definitions, two-resistor and DELPHI compact models, GK), fan curves, conjugate solves.
- First slice: JEDEC-style two-resistor package model as a lumped zone component, with a heatsink-on-board conjugate benchmark.

### S-16 Semiconductor power and thermal (chip-package-system)

- Purpose: chip-level power-delivery and thermal signoff. Covered in S-14 where read (P7); kept as a separate entry because the vendor sells it as its own product line.
- Capabilities (P7): see S-14 chip-level entries. Note P1 does not list a semiconductors category; RedHawk-SC was reached through its Synopsys page, so catalogue structure for this area is unverified.
- Sources read: P7.
- CFD overlap: F-051 (on-chip conduction).
- CFD dependencies: electrothermal loop with a thermal solver.
- Size/priority: P4 (foundry data not public).
- Open route: no for signoff grade; yes for a research-grade IR-drop and thermal-map tool on public power-grid benchmarks (GK).
- First slice: none beyond the S-14 PDN solver.

---

## C. Optics and photonics

### S-17 Optical system design and stray light (ray optics)

- Purpose: lens, illumination and imaging system design and analysis.
- Capabilities (P4, P12): sequential, non-sequential and mixed ray tracing; physical optics propagation and diffractive optics; wavefront error, MTF, PSF; sensitivity and Monte Carlo tolerancing, freeform tolerancing; stray light, ghosts, veiling glare, scattering; polarisation ray tracing and thin-film coatings; glass catalogues, GRIN; thermal-structural-optical coupling; freeform and metasurface modelling; scripting API; optomechanical tolerancing. Speos-class adds: ray tracing in CAD assemblies, photometric targets, measured material (BSDF) data, camera/sensor and human-vision models, glare, HDR/VR review, lidar simulation, aero-optical and GRIN media, GPU previews, shareable precomputed light-field subsystems.
- Sources read: P4, P12.
- CFD overlap: F-052 and F-130 (radiation ray-tracing infrastructure), F-238, F-264. Aero-optics takes a CFD density field as input.
- CFD dependencies: density fields from compressible CFD (F-060); thermal gradients for thermo-optics.
- Size/priority: P3.
- Open route: yes. Snell/Fresnel ray tracing, Sellmeier dispersion with public coefficient catalogues, Seidel and Zernike aberrations, Fourier optics (angular spectrum, Fresnel/Fraunhofer), Monte Carlo scatter with Harvey-Shack or Torrance-Sparrow BSDFs, Mueller/Jones polarisation, damped-least-squares optimisation (GK).
- First slice: sequential ray tracer with spot diagram and Seidel check on a Cooke triplet from public prescription data.

### S-18 Photonics (nanophotonics, photonic integrated circuits)

- Purpose: sub-wavelength and waveguide-scale light simulation.
- Capabilities: P1 names a Lumerical FDTD-class product; its page failed to fetch. GK: FDTD solver, finite-difference eigenmode solver for waveguides, rigorous coupled-wave analysis for gratings, photonic circuit simulation, laser/photodetector device solvers, metasurface workflows. P4 mentions metasurface modelling in the lens-design tool.
- Sources read: P1 (name only), P4 (one line).
- CFD overlap: none.
- CFD dependencies: none.
- Size/priority: P3 (shares an FDTD core with S-13).
- Open route: yes. Yee FDTD with PML, dispersive-material models (Drude-Lorentz), vector mode solvers, RCWA (Moharam-Gaylord), transfer-matrix, coupled-mode theory (GK).
- First slice: share the S-13 FDTD kernel; add a slab-waveguide mode solver and a thin-film transfer-matrix reflectance calculator checked against analytic stacks.

---

## D. Acoustics and NVH

### S-19 Structural acoustics, sound quality, NVH

- Purpose: radiated sound from structures, and perceived sound quality.
- Capabilities (P20, P2, P21): sound analysis, psychoacoustic metrics, active sound design for EV/ICE, jury testing, 3D sound simulator, listening to CFD noise, structural vibro-acoustics, e-motor noise from EM forces, gear whine and rattle, frequency-domain NVH in explicit tool. P21 lists acoustic theory: wave propagation, fluid-structure interaction, room acoustics, full linear Navier-Stokes, poroelastic and nonlinear acoustics.
- Sources read: P20, P2, P21 (headings).
- CFD overlap: F-136 (FW-H), F-243 (acoustic boundaries and wave-equation solver), F-256, `docs/reactor-and-acoustics.md`.
- CFD dependencies: CFD noise sources (F-136) feed propagation; shares the wave-equation solver planned in F-243.
- Size/priority: P3 (FEM Helmholtz is P2-sized once S-01 exists; psychoacoustics is small and standalone).
- Open route: yes. FEM/BEM Helmholtz, perfectly matched layers, acoustic-structure coupling, ISO 532-1 loudness (Zwicker), sharpness and roughness models, A-weighting (IEC 61672) (GK).
- First slice: ISO 532-1 loudness and A-weighted level calculator as a signal library with the standard's test signals; later 2D Helmholtz FEM for a duct.

---

## E. Particle and granular (non-fluid part)

### S-20 Particle dynamics (DEM) and coupling

- Purpose: bulk-material and particle-system simulation with non-spherical particles, breakage, wear, cohesion, and fluid/structure coupling.
- Capabilities (P15, P5): multi-GPU solving, huge particle counts; non-spherical shapes, shells, fibres, 3D-scan shapes; breakage, wear, cohesion; coupling to CFD, FEA, EM and multibody; SPH for fluid-particle problems; scripting; DEM also in the explicit tool.
- Sources read: P15, P5.
- CFD overlap: F-249 (collision, coalescence, DEM), F-247 (granular kinetic theory), F-056/F-128, F-139.
- CFD dependencies: CFD-DEM two-way coupling; largely subsumed by F-249 if built, so do not implement twice.
- Size/priority: P3.
- Open route: yes. Hertz-Mindlin contact, spring-dashpot, rolling resistance, JKR cohesion, multisphere and superquadric shapes, cell-list neighbour search (GK).
- First slice: none beyond F-249; if split out, a hopper-discharge DEM kernel validated against Beverloo's correlation.

---

## F. Materials

### S-21 Materials data and selection

- Purpose: curated materials property data, selection and simulation-ready export; sustainability and compliance screening.
- Capabilities (P13, P2): enterprise materials database with CAD/CAE/PLM links, curated data library, sustainability and ML add-ons; lighter database edition; selection tool (property trade-offs, similarity search, simulation-ready export); education edition; restricted-substance screening; property-process relationships for additive; first-principles property prediction product; connected materials-to-particle workflow for battery calendaring; materials data available inside the structural tool.
- Sources read: P13, P2.
- CFD overlap: F-111, F-220, F-110, F-105, F-224.
- CFD dependencies: shared property schema; the CFD library is the seed.
- Size/priority: P2 (cheap, high leverage: structural, EM and optics suites all consume it).
- Open route: yes for schema, selection (Ashby charts, merit indices) and open-licence datasets only (NASA polynomials, NIST data; check licences). Curated commercial libraries are not reproducible.
- First slice: versioned materials-record schema (units, temperature dependence, provenance and licence per datum) with a CSV/JSON loader and an Ashby-style selection query, populated from open-licence data.

### S-22 Atomistic and ab initio materials modelling

- Purpose: predict material properties from first principles.
- Capabilities (P13, one line): electronic, structural, thermal, magnetic and mechanical property prediction. Detail GK.
- Sources read: P13.
- CFD overlap: none.
- CFD dependencies: none.
- Size/priority: P4 (out of scope for an engineering simulation project).
- Open route: yes in principle (DFT, molecular dynamics, tight binding), but mature open codes exist; recommended action is interoperability not reimplementation.
- First slice: none; define an import path for externally computed properties into S-21.

---

## G. Systems, embedded software, digital twin, safety

### S-23 System-level modelling and digital twins

- Purpose: multi-domain system models, ROMs from 3D physics, and deployed twins connected to live data.
- Capabilities (P17, P6): hierarchical multi-domain schematics; Modelica standard library and third-party libraries (hydraulics, pneumatics, cooling, heat exchangers, thermal power); ROMs from structural, fluids, EM and semiconductor tools or third-party metamodels; twin validation and deployment to cloud, edge or offline; IIoT connectivity; web-app prototype export; FMI and state-space models in EM tools.
- Sources read: P17, P6, P1 (category name).
- CFD overlap: F-262 (ROM export), F-242 (FMI import), F-113 (lumped components, 1D networks), F-139, F-122, F-085.
- CFD dependencies: ROM and FMI export from the CFD core are the main interfaces; a 1D network solver is shared.
- Size/priority: P2 for FMI/ROM interfaces and a 0D/1D network solver; P4 for the full IIoT deployment story.
- Open route: yes. FMI 2/3, Modelica Language Specification, Modelica Standard Library (open licence), POD/DMD/Galerkin ROMs, Kalman-filter estimation, SSP standard (GK).
- First slice: export a CFD run as an FMI 2 co-simulation FMU (stepping through the F-085 service) and import a simple FMU as a boundary condition (F-242).

### S-24 Embedded software engineering (model-based)

- Purpose: model-based design, verification and certified code generation for safety-critical embedded software and HMI.
- Capabilities (P14): model-based embedded control/mission/autonomy platform; safety-critical design and verification with certified code generation; embedded graphics/HMI with code generation; requirement-based testing with model and code coverage; architecture modelling with interface-control-document generation; avionics cockpit-display (ARINC 661) design; automotive test tool with CI automation; qualification against DO-178C, ISO 26262, IEC 61508, EN 50128.
- Sources read: P14.
- CFD overlap: none.
- CFD dependencies: none; only link is closed-loop testing with S-23 models.
- Size/priority: P4 (certification and qualification is a business and legal matter more than an algorithm one).
- Open route: partial. Synchronous dataflow languages (Lustre, public papers), MC/DC coverage (DO-178C definition) (GK); qualified code generators cannot be self-certified.
- First slice: none recommended; logged only.

### S-25 Safety, reliability and cybersecurity analysis

- Purpose: functional-safety and risk analysis methods and documentation.
- Capabilities (P19): hazard analysis and risk classification (HARA), fault-tree analysis, FMEA, FMEDA, ISO 26262, IEC 61508, ARP4761, SOTIF (ISO 21448) cause-effect and event trees, threat analysis and risk assessment (TARA) with component catalogues and vulnerability monitoring, safety-case generation, semiconductor FMEDA, avionics architecture packages (ARINC, AADL, FACE).
- Sources read: P19.
- CFD overlap: none.
- CFD dependencies: none.
- Size/priority: P4.
- Open route: yes for the methods (fault-tree minimal cut sets, Birnbaum importance, Markov reliability models, FMEA/FMEDA tables) (GK). Standards texts are paywalled and must not be copied.
- First slice: fault-tree evaluator (minimal cut sets, top-event probability) with JSON in/out.

### S-26 Autonomous-vehicle and mission-level simulation

- Purpose: scenario-based simulation of driving automation, sensors and mission-level systems engineering.
- Capabilities: P1 lists the categories "Autonomous Vehicles" and "Digital Mission Engineering" with no products; the AV category fetch failed. Anything further is GK: scenario generation, camera/radar/lidar models, driving simulators, system-of-systems mission models. Lidar and camera sensor models appear in P12.
- Sources read: P1 (names only), P12.
- CFD overlap: none.
- CFD dependencies: none.
- Size/priority: P4.
- Open route: yes (GK). ASAM OpenSCENARIO/OpenDRIVE, physics-based radar equation, ray-traced lidar.
- First slice: none; logged only.

---

## H. Geometry, meshing, pre/post tools

### S-27 Geometry and CAD tools (direct modelling, repair, simulation prep)

- Purpose: build, repair and defeature CAD for simulation; interactive real-time simulation front end.
- Capabilities (P11, P2): interactive 3D direct modelling for simulation; real-time structural, thermal and fluid analysis; topology and parametric studies; opening, repairing and editing CAD including faceted models; CAD connection in the structural tool; scripting. Defeaturing, midsurface extraction and parametric CAD links are GK; the SpaceClaim-class documentation was not read.
- Sources read: P11, P2.
- CFD overlap: F-245 (CAD import via open kernel), F-231, F-232, F-131, F-124.
- CFD dependencies: shared geometry pipeline; every structural, EM and optics suite needs the same kernel.
- Size/priority: P2 for import and repair (F-245 first); P4 for an interactive direct-modelling GUI.
- Open route: yes. Open CAD kernel (OpenCASCADE is LGPL; verify licence compatibility), STEP AP203/214/242 and IGES, mid-surface extraction (medial axis), feature-size defeaturing, B-rep Booleans (GK).
- First slice: F-245 as specified; later add midsurface extraction for thin parts (needed by shells in S-01).

### S-28 Meshing for structures and electromagnetics

- Purpose: element meshes for FE (tetra, hexa, shell, beam, edge-element) as opposed to finite-volume polyhedra.
- Capabilities (P2, P21): CAD-driven automatic meshing with adaptivity; shape-function and element library (line, shell, solid, EM vector elements); mesh-quality checks; nonlinear adaptive re-meshing. Specific mesher method names (patch-conforming tetra, sweep, hex-dominant, MultiZone, inflation) are GK because the meshing guide fetch failed.
- Sources read: P2, P21 (headings).
- CFD overlap: F-010, F-011, F-132, F-235, F-234, F-108, F-102, F-233.
- CFD dependencies: extend the F-010 mesh container with higher-order elements, element sets and node sets so FV and FE share it.
- Size/priority: P2.
- Open route: yes. Delaunay and advancing-front tet meshing, octree, hex-dominant via paving, second-order element generation, Jacobian-based quality metrics (Knupp); Gmsh as external tool (GK).
- First slice: element-type extension of the mesh container (tet4, tet10, hex8, tri3 shell) with Jacobian-quality checks and Gmsh read/write for those types (builds on F-013).

### S-29 Results viewing, post-processing and reporting

- Purpose: visualise and compare results from many physics, interactively or scripted.
- Capabilities (Q3 snippet, P21): GUI plus C/C++/Python store API and gRPC; Python wrapper for scripted sessions; read solver output and build post-processing; extract or embed visualisations; use inside the workflow tool; structural post (stress, strain, energy, linearisation, time-history post, P21 headings).
- Sources read: Q3 (snippet), P21 (headings).
- CFD overlap: F-070, F-071, F-072, F-073, F-103, F-117, F-119, F-143, F-151, F-238, F-264.
- CFD dependencies: same results container (F-015) and viewer (F-151); tensor and complex results (stress, modal, harmonic) need extending.
- Size/priority: P2.
- Open route: yes. VTK, XDMF/HDF5, glTF, Exodus II, CGNS; stress invariants, through-thickness linearisation, complex-result phase animation (GK).
- First slice: tensor and complex fields in the results container and VTK/XDMF writer, with von Mises, principal stress and modal-shape animation.

---

## I. Platform: workflow, optimisation, HPC, cloud, data management

### S-30 Workflow, DOE, optimisation and uncertainty (PIDO)

- Purpose: connect tools into automated processes and drive design studies.
- Capabilities (P18, Q2 snippet, P6, P2): sensitivity analysis; optimisation including calibration; robustness and reliability analysis; metamodels for fields and signals; reusable workflows; publishing a process as a web app; parametric optimisation in EM and structural tools; scripting and journaling.
- Sources read: P18, Q2 (snippet), P6, P2.
- CFD overlap: F-257 (DOE and optimisation drivers), F-095, F-120, F-262, F-137, F-094.
- CFD dependencies: consumes the same case/run API (F-085, F-082).
- Size/priority: P2 (F-257 is already a parity item; extend to robustness and metamodels).
- Open route: yes. Latin hypercube and Sobol sampling, Sobol and Morris sensitivity indices, Gaussian-process and polynomial-chaos metamodels, NSGA-II and CMA-ES, FORM/SORM and subset simulation (GK).
- First slice: the F-257 acceptance case plus Sobol indices and a Gaussian-process metamodel on the analytic test problem.

### S-31 HPC, cloud and job management

- Purpose: run solvers at scale locally, on clusters and in the cloud.
- Capabilities (P2, P3, P5, P15, P16, mentions only): on-demand cloud burst compute; GPU resource recommendation; SMP/MPP solvers; multi-GPU. The Cloud product pages and a platform page were not read (P1 gives the category name only); further detail GK.
- Sources read: mentions in P2, P3, P15, P16; P1 (category name).
- CFD overlap: F-100 (MPI), F-101 (GPU), F-236, F-267, F-266, F-121.
- CFD dependencies: the parallel strategy in ADRs (F-004) governs every other suite; structural solvers need distributed sparse direct or iterative solvers.
- Size/priority: P2 shared infrastructure.
- Open route: yes. MPI, Slurm/PBS templates, container images, distributed Krylov/AMG designs from papers, multifrontal sparse direct algorithms (GK).
- First slice: none new; ensure F-100 and F-267 interfaces do not assume a flow-solver-only data model.

### S-32 Simulation process and data management

- Purpose: store, version, trace and share simulation models, results and decisions across a team.
- Capabilities (Q2 snippet, P13): enterprise simulation process and data management with PLM-style integration; publishing optimisation processes as web apps; traceability; materials data management as a specialised case.
- Sources read: Q2 (snippet), P13.
- CFD overlap: F-015, F-125, F-006, F-084, F-241.
- CFD dependencies: shared provenance record (input hashes, solver version, mesh hash, settings tree).
- Size/priority: P3.
- Open route: yes. Content-addressed storage, W3C PROV, SQLite or git-style object stores, OpenAPI services (GK).
- First slice: a provenance manifest written with every run (input hashes, versions, host, command line) and a `runs list/compare` command.

---

## J. Other catalogue items noted

- AI and engineering assistants (P1 category; P15 and P19 mention an assistant inside their tools): rcfd already has the assistant sidebar and provider layer (F-145..F-150); no separate suite. Surrogate models fall under S-30.
- Battery, fuel-cell, aeromechanics and combustion items belong to the CFD matrix (F-246..F-267), not here.
- "Connect" (P1 category name only; no content read): interpreted as interoperability, covered by S-23, S-30, S-31 (GK).

---

## Cross-suite section

### Shared infrastructure that several suites need

| Infrastructure | Suites that need it | Existing rcfd base | Gap |
|---|---|---|---|
| FE assembly and element library (shape functions, quadrature, element tools) | S-01..S-07, S-10, S-12, S-19 | F-025 sparse assembly, F-010 mesh | Isoparametric elements, DOF numbering, constraint handling (Lagrange, penalty, elimination), non-polyhedral element types |
| Structural solver core (static, nonlinear Newton, eigen, transient) | S-01..S-04, S-06, S-07, S-10, S-11, S-19 | F-026, F-027, F-260 (intent) | Direct sparse solver, Lanczos eigensolver, Newton with line search/arc length, Newmark/generalised-alpha |
| Curl-conforming FE and time-domain Yee kernels | S-12, S-13, S-14, S-18 | none | New discretisation family; complex-valued sparse algebra |
| Material database and constitutive interface | S-01, S-03..S-06, S-10, S-12, S-17, S-21 | F-111, F-220, F-110, F-105 | Unified schema with temperature/frequency/direction dependence, provenance and licence per datum; tensor properties; B-H curves; optical dispersion |
| Co-simulation and coupling interface | S-01, S-08, S-12, S-15, S-20, S-23 | F-139 (todo), F-242 (todo), F-237 | Conservative and consistent load/displacement mapping, coupling-step control, FMI 2/3 |
| Mesh container with FE element types and sets | S-01..S-07, S-12, S-13, S-28 | F-010, F-013, F-015 | Higher-order elements, element/node sets, per-region element types |
| Results formats and viewer | all | F-070, F-103, F-117, F-143, F-151 | Tensor, complex, modal and time-history results; XDMF/Exodus II |
| Parametric/optimisation driver and metamodels | S-07, S-12, S-15, S-23, S-30 | F-095, F-257 (parity), F-104 | Robustness, sensitivity, surrogates |
| Geometry kernel and CAD import | S-15, S-17, S-27, S-28 | F-245 (parity) | Midsurface, defeaturing, parametric CAD |
| Ray-tracing core | S-17, radiation (F-052/F-130) | F-052, F-130 | BVH acceleration, spectral/polarised rays, scatter models |
| Signal/time-series library (rainflow, PSD, FFT, loudness) | S-02, S-06, S-11, S-19 | none | Small standalone crate |
| Provenance and run registry | S-32 and all | F-125, F-084 | Manifest and compare |

No ordering is proposed. The common prerequisites are FE element infrastructure with the mesh-container extension, the material schema, and the co-simulation interface; each is cheaper than any one suite and most are needed by three or more.

### Dependency summary on CFD

- Receive from CFD: pressure, shear and temperature loads (S-01, S-02, S-11), heat-transfer coefficients (S-11, S-15), density fields (S-17 aero-optics), noise sources (S-19), particle forces (S-20).
- Give to CFD: displacements and mesh motion (S-01, S-08), Joule and magnetic losses as heat sources (S-12, S-14), Lorentz forces (S-12), rigid-body motion (S-08).
- Essentially standalone: S-03, S-05, S-06, S-18, S-22, S-24, S-25, S-26.

### Honest coverage statement

- Read with a tool: 20 public vendor pages (P1..P20) and one online-help contents page (P21, login required), all through WebFetch's model-condensed extraction. P1 descriptions were inferred from names by the extractor and count as names only. P21 gives chapter and section headings only, not theory.
- Weak evidence: three WebSearch snippet sets (Q1..Q3), marked "(snippet)".
- Failed fetches (404): Lumerical FDTD, Additive landing page, Digital Twins category, Composite PrepPost, Workbench platform page, Autonomous Vehicles category, Workbench meshing guide. S-05 (detail), S-09, S-16 (catalogue structure), S-18, S-26, S-28 (mesher method names) and S-31 therefore rest partly or mostly on general knowledge, marked GK.
- Not examined at all: Aqwa, AVxcelerate, Cloud/HPC product pages, EnSight product page (snippets only), Discovery/SpaceClaim documentation, LS-DYNA keyword manual, Motion theory, Maxwell/HFSS user guides, Lumerical, Granta technical docs, Minerva product page, medini and SCADE docs, release notes. The P1 catalogue lists 16 categories with products for only 5, so the product inventory is likely incomplete.
- The "Open route" algorithm and standard names are general knowledge from public literature, not read in this pass. Standards named (ISO 26262, IEC 61508, DO-178C, ARP4761, FMI, Modelica, JEDEC JESD51, IPC, ISO 532, IEC 61672, ASAM OpenSCENARIO) are named only; texts were not read and several are paywalled.
- Priority and size classes are judgement, not measured effort.
- Nothing from any vendor install or binary was touched.
