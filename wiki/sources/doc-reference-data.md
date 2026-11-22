---
type: source
tags: [reference-data, validation, hub]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data audit (hub)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md) at origin/main 2e94e5ef (2459 lines, generated from `docs/reference-data.d/*.part`).
**Kind:** repo doc: lookup record of where every benchmark value or published constant in a test, case or data file comes from and how well it was checked. Split into 147 section pages, one per top-level `##` section.

## Summary

The page started as the wave 6a audit: every benchmark value or published constant that earlier waves typed from memory was checked against a citable public source. Counted by table row, wave 6a found "about 34 verified, 3 mismatched, about 8 unverifiable or not re-checked". The three mismatches were "one sign error in a test helper (Taylor-Green initial pressure) and two citation or label errors (Darwish-Moukalled journal; flat-plate "Prandtl-Schlichting" label)"; nothing was loosened and "no benchmark number in a data file or assertion turned out to be wrong". Wave 13 re-checked items marked unverified or from memory; "One code constant differs from the original (Realizable A0 4.04 vs 4.0)". Later waves appended their own sections in the order the work was done, ending with the fan sections (F-868, F-869, F-870).

Label vocabulary used throughout: "Verified" means the repo value agrees with the source; "secondary" means the source is a reputable paper or repository that reproduces the primary table, not the original printed table. Later sections also use "NOT read", "from memory", "UNVERIFIED", "derived" and "data not obtained". For downloadable datasets, checksums and licences the doc points to [[doc-validation-data]]; the per-item re-check of wave 13 is in [[doc-verification-report]].

## Key points

- Benchmark sections back the validation cases: [[lid-driven-cavity]], [[differentially-heated-cavity]], [[cylinder-re100]], [[backward-facing-step]], [[flat-plate]], [[naca0012]]; the immersed-solid sections back [[dfg-cylinder-benchmark]].
- "Known misses" lists registered criteria the code does not meet, each with an ignored known-miss test ([[doc-reference-data-known-misses]], [[known-miss]]).
- "Not verifiable online" lists what still rests on secondary copies ([[doc-reference-data-not-verifiable-online]]).
- The structural (FEM) sections mostly say the formula is derived in the ADR and the named literature was "NOT read" or recalled from memory.
- The last three sections record the milestone headline data: Ladson NACA 0012 maximum lift (F-868, ADR 0760), the cylinder mean drag band at Re 100 (F-869, ADR 0761) and the FAN-01 design point (F-870, ADR 0762).

## Section pages

### Benchmarks and model constants

- [[doc-reference-data-lid-driven-cavity]] — Ghia, Ghia and Shin (1982) centreline velocities used for the lid-driven cavity, verified against secondary copies, with a recorded misprint in the Re 400 v table.
- [[doc-reference-data-heated-cavity]] — de Vahl Davis (1983) benchmark values for the differentially heated cavity at Ra 1e3 to 1e6, read in the primary Table V scan in wave 30C, plus other values read in that wave.
- [[doc-reference-data-riemann-sod-oblique-shock]] — Exact Riemann solver, Sod problem and oblique-shock reference values; a Toro table attribution removed and the Darwish-Moukalled citation corrected.
- [[doc-reference-data-taylor-green]] — Taylor-Green vortex test helper: the initial-pressure sign mismatch found in wave 6a and the corrected kinetic-energy error.
- [[doc-reference-data-turbulence-constants]] — k-epsilon (standard, RNG, Realizable) and wall-function constants with their verification state, including the Realizable A0 4.04 vs 4.0 difference.
- [[doc-reference-data-sst-wall]] — k-omega SST constants and wall treatment, the NASA TMR flat-plate comparison and the wall-law formulas.
- [[doc-reference-data-channel-dns]] — Moser, Kim and Mansour (1999) channel DNS at Re_tau 395 values read from chan395.means.
- [[doc-reference-data-cylinder-wake]] — Cylinder wake at Re 100: Strouhal reference values, the Williamson 1989 fit and the recorded run.
- [[doc-reference-data-backward-step]] — Driver and Seegmiller (1985) turbulent backward-facing step reattachment length and conditions.
- [[doc-reference-data-laminar-step]] — Laminar backward-facing step with expansion ratio 2: Erturk and Gartling reattachment lengths and the measured rcfd values.
- [[doc-reference-data-flat-plate]] — Flat-plate skin-friction correlations (Blasius, power laws, White) and the corrected Prandtl-Schlichting label.
- [[doc-reference-data-naca0012-thickness]] — NACA four-digit thickness coefficients and the thin-aerofoil lift slope used by the NACA 0012 case.

### Literature and data

- [[doc-reference-data-other-literature]] — Bibliographic checks of other cited literature (Patankar, Darwish-Moukalled, Issa, Choi, Ericson).
- [[doc-reference-data-not-verifiable-online]] — The list of sources that could not be reached online and what still rests on secondary copies.
- [[doc-reference-data-validation-pack]] — The public validation-data pack of F-125 as fetched on 2026-10-13: what each fetched file showed and what stayed unresolved.

### Models and formats, by feature

- [[doc-reference-data-fluent-mesh-format]] — Fluent mesh-format details (F-118): boundary type ids typed from memory and the elbow.msh check.
- [[doc-reference-data-viscosity-laws]] — Viscosity laws of F-110 (Carreau-Yasuda, Cross, WLF, Papanastasiou and others) with their verification state.
- [[doc-reference-data-material-library]] — Material property library (F-111): IAPWS-IF97, NASA polynomials, air viscosity, Sutherland, atomic weights, dry air, cubic EOS constants.
- [[doc-reference-data-variable-density]] — Variable-density validation of wave 10 (ADR 0005) against Le Quere et al. (2005) values.
- [[doc-reference-data-transient-variable-density]] — Transient variable-density checks of ADR 0009: closed forms and unverified acoustic and Couette citations.
- [[doc-reference-data-known-misses]] — Table of registered criteria the code does not meet, with their ignored known-miss tests, after the 2026-10-14 audit.
- [[doc-reference-data-compressible-bcs]] — Compressible boundary conditions (F-062, ADR 0006): citations confirmed, formulas unverified.
- [[doc-reference-data-real-gas-eos]] — Real-gas equation of state (F-061, ADR 0007): NIST isotherm comparisons and the density and enthalpy known misses.
- [[doc-reference-data-mrf-zones]] — MRF zones (F-031, F-114, ADR 0008): analytic Taylor-Couette checks M1 to M6 and their misses.
- [[doc-reference-data-interchange-formats]] — Interchange formats, registers and GLB validation (F-119, F-124, F-117, ADR 0011): VTK and Khronos oracles.
- [[doc-reference-data-lumped-radiation]] — Lumped zones and radiation (F-113, F-130, ADR 0012): Milne value, known misses, WSGG not implemented.
- [[doc-reference-data-reactors-fwh]] — 0D reactors and FW-H acoustics (F-112, F-136, ADR 0013): Robertson digits, flame temperature conflict, Formulation 1A.
- [[doc-reference-data-spalart-allmaras]] — Spalart-Allmaras constants and the TMR flat-plate skin friction (F-041, ADR 0014).
- [[doc-reference-data-turbulence-finishing]] — Turbulence finishing (F-042, F-043, F-203, ADR 0015): backward-step and flat-plate results against their windows.

### Wave sections (waves 13 to 32)

- [[doc-reference-data-wave13-numerics]] — Wave 13 numerics and wall motion references (ADR 0016), all from memory.
- [[doc-reference-data-wave14-mesh-quality]] — Wave 14 mesh quality and editing references (ADR 0018): default limits from memory.
- [[doc-reference-data-wave15b-partitioning]] — Wave 15B partitioning, reordering, inflation and derived-field references (ADR 0020).
- [[doc-reference-data-wave16w-wiring]] — Wave 16W wiring (ADR 0021): no new reference value.
- [[doc-reference-data-wave16b-bc-catalogue]] — Wave 16B boundary-condition catalogue references (ADR 0022), including the Poiseuille check.
- [[doc-reference-data-wave17c-gui-assistant]] — Wave 17C GUI, assistant and renderer references (ADR 0025), with wave 17A known misses.
- [[doc-reference-data-wave17b-cht]] — Wave 17B conjugate heat transfer references (ADR 0024): Stefan-Boltzmann constant and Nusselt values.
- [[doc-reference-data-wave18c-particles]] — Wave 18C particle tracking references (ADR 0028): Schiller-Naumann drag and its switch.
- [[doc-reference-data-algebraic-sgs]] — Algebraic subgrid and mixing-length model constants (ADR 0026), unverified.
- [[doc-reference-data-wave18b-combustion]] — Wave 18B combustion constants (ADR 0027), EDC constants from a secondary source.
- [[doc-reference-data-wave19b-meshing]] — Wave 19B meshing references (ADR 0030), unverified.
- [[doc-reference-data-wave20c-study]] — Wave 20C study references (ADR 0034).
- [[doc-reference-data-wave20a-rsm-des]] — Wave 20A transposed stress, SA-based DES and Reynolds-stress model references (ADR 0032).
- [[doc-reference-data-wave20b-dynamic-mesh]] — Wave 20B dynamic mesh and six-degree-of-freedom references (ADR 0033), unverified.
- [[doc-reference-data-wave21b-properties-actuator]] — Wave 21B tabulated properties, actuator disk and region references (ADR 0036).
- [[doc-reference-data-wave21c-fractional-step]] — Wave 21C run-loop wiring and fractional-step references (ADR 0037).
- [[doc-reference-data-wave21a-thermal]] — Wave 21A thermal extras references (ADR 0035).
- [[doc-reference-data-wave23c-population-balance]] — Wave 23C population balance references (ADR 0041), all unverified.
- [[doc-reference-data-wave23b-mass-transfer-waves]] — Wave 23B interfacial mass transfer and wave references (ADR 0040).
- [[doc-reference-data-wave23a-mhd-heat-exchanger]] — Wave 23A MHD Lorentz force and heat-exchanger references (ADR 0039).
- [[doc-reference-data-wave25b-reference-verification]] — Wave 25B re-check of earlier reference values (ADR 0044 D5) and the discrepancies it found.
- [[doc-reference-data-wave25c-transient-sources]] — Wave 25C transient momentum source references (ADR 0045).
- [[doc-reference-data-wave26c-post-ensight]] — Wave 26C post-object catalogue and EnSight export references (ADR 0048).
- [[doc-reference-data-wave26b-sweep-cartesian]] — Wave 26B sweep and Cartesian-core meshing references (ADR 0047).
- [[doc-reference-data-wave27b-robustness]] — Wave 27B robustness and adaptive time stepping references (ADR 0050).
- [[doc-reference-data-wave27c-viscoelasticity]] — Wave 27C viscoelasticity and structural export references (ADR 0051).
- [[doc-reference-data-wave27a-vof]] — Wave 27A volume of fluid and surface tension references (ADR 0049), including Martin-Moyce and Hysing.
- [[doc-reference-data-wave28c-adaptation-turbo]] — Wave 28C adaptation and turbomachinery post-processing references (ADR 0054).
- [[doc-reference-data-wave28a-vof-wiring]] — Wave 28A VOF wiring references (ADR 0052).
- [[doc-reference-data-wave28b-chemistry]] — Wave 28B chemistry acceleration and combustion extension references (ADR 0053).
- [[doc-reference-data-wave29d-distributed]] — Wave 29D distributed-memory layer references (ADR 0058).
- [[doc-reference-data-wave29c-lagrangian]] — Wave 29C Lagrangian extension references (ADR 0057).
- [[doc-reference-data-wave29a-rotor-interfaces]] — Wave 29A rotor interfaces and guarded secant references (ADR 0055).
- [[doc-reference-data-wave29b-cavitation]] — Wave 29B mixture and cavitation references (ADR 0056).
- [[doc-reference-data-wave30a-coupling-mixing-plane]] — Wave 30A two-way particle coupling and mixing plane references (ADR 0059).
- [[doc-reference-data-wave30b-incompressible-validation]] — Wave 30B: incompressible validation data actually read on 2026-10-17 (ADR 0060).
- [[doc-reference-data-wave31c-reference-reading]] — Wave 31C: sources read on 2026-10-18 for the validation cases (ADR 0064), with a discrepancy list.
- [[doc-reference-data-wave31b-turbulence-wall]] — Wave 31B: turbulence wall diagnosis (ADR 0063) and the TMR flat-plate grid convergence.
- [[doc-reference-data-wave31a-solver-defects]] — Wave 31A: solver defects found by the wave 30 validation (ADR 0062).
- [[doc-reference-data-wave32b-aeroacoustics]] — Wave 32B: aeroacoustics references for fan noise (ADR 0066).
- [[doc-reference-data-wave32c-design-loop]] — Wave 32C: design-loop infrastructure references and measured optimiser results (ADR 0067).
- [[doc-reference-data-wave32a-fan-geometry]] — Wave 32A: fan geometry and sector mesh references (ADR 0065).

### F-item sections (particles, immersed solid, design loop, multiphase)

- [[doc-reference-data-f253-wall-film]] — F-253 particle wall film references (ADR 0192).
- [[doc-reference-data-f255-particle-dispersion]] — F-255 particle dispersion and surface reaction references (ADR 0208).
- [[doc-reference-data-f254-particle-wall]] — F-254 particle wall interaction references (ADR 0194).
- [[doc-reference-data-f249-nsga2-nelder-mead]] — F-249 parametric study references: NSGA-II, Nelder-Mead options, extended warm start (ADR 0209).
- [[doc-reference-data-f219-immersed-solid]] — F-219 immersed solid with modified forcing (ADR 0218): DFG 2D-1 and 2D-2 reference values.
- [[doc-reference-data-f309-immersed-second-order]] — F-309 second-order immersed forcing (ADR 0234).
- [[doc-reference-data-f337-immersed-moment]] — F-337 immersed solid second-order moment of the forcing (ADR 0239).
- [[doc-reference-data-f346-immersed-unsteady-force]] — F-346 immersed solid force and moment in unsteady flow (ADR 0249).
- [[doc-reference-data-f355-immersed-piso]] — F-355 second-order immersed forcing in transient PISO (ADR 0254).
- [[doc-reference-data-f354-dfg-2d2-extrema]] — F-354 immersed solid DFG 2D-2 force extrema under refinement (ADR 0268).
- [[doc-reference-data-f297-nsga2-crossover]] — F-297 NSGA-II crossover read against the papers (ADR 0220).
- [[doc-reference-data-f260-cma-es]] — F-260 CMA-ES read in Hansen's tutorial and the restart paper (ADR 0222).
- [[doc-reference-data-f221-condensing-steam]] — F-221 non-equilibrium condensing steam references (ADR 0217).
- [[doc-reference-data-f245-tab-breakup]] — F-245 droplet evaporation and TAB breakup constants (ADR 0224).
- [[doc-reference-data-f306-tet-mesher]] — F-306 folded edge removal of the tetrahedral mesher (ADR 0228).
- [[doc-reference-data-f302-seam-diffusivity]] — F-302 periodic seam pair diffusivity (ADR 0227).
- [[doc-reference-data-f300-transient-rans]] — F-300 transient RANS follow-ups (ADR 0225).

### Structural solver (FEM) sections

- [[doc-reference-data-f431-sparse-ldlt]] — F-431 sparse LDL^T with a fill-reducing ordering (ADR 0447).
- [[doc-reference-data-f414-eigensolver]] — F-414 general real eigensolver, quadratic eigenproblem, damped modes (ADR 0450).
- [[doc-reference-data-f413-fatigue]] — F-413 high-cycle fatigue references (ADR 0449).
- [[doc-reference-data-f409-orthotropic]] — F-409 orthotropic elasticity references (ADR 0452).
- [[doc-reference-data-f419-shell3]] — F-419 three-node flat shell Shell3 (ADR 0454).
- [[doc-reference-data-f426-tied-meshes]] — F-426 tied non-matching meshes and joints (ADR 0456).
- [[doc-reference-data-f539-drilling-shell3]] — F-539 drilling-unknown membranes for Shell3 (ADR 0466).
- [[doc-reference-data-f549-drilling-shell4]] — F-549 drilling membrane for Shell4 (ADR 0470).
- [[doc-reference-data-f418-radiated-sound]] — F-418 radiated sound of a vibrating surface (ADR 0468).
- [[doc-reference-data-f551-radiation-reactance]] — F-551 radiation reactance of a baffled plane (ADR 0488).
- [[doc-reference-data-f579-mutual-radiation]] — F-579 mutual radiation impedance and light-fluid modes (ADR 0538).
- [[doc-reference-data-f430-substructuring]] — F-430 substructuring and Craig-Bampton reduction (ADR 0469).
- [[doc-reference-data-f415-coriolis-campbell]] — F-415 Coriolis term on the modal subspace, split Campbell table (ADR 0472).
- [[doc-reference-data-f555-cyclic-campbell]] — F-555 split Campbell table of a cyclic sector (ADR 0481).
- [[doc-reference-data-f569-static-correction]] — F-569 static correction vectors for sector split modes (ADR 0482).
- [[doc-reference-data-f575-ritz-error]] — F-575 second-order Ritz error of corrected sector split modes (ADR 0492).
- [[doc-reference-data-f416-rotor-bearings]] — F-416 flexible rotor on bearings in the stationary frame (ADR 0476).
- [[doc-reference-data-f420-stress-stiffening]] — F-420 stress stiffening of beams and shells (ADR 0473).
- [[doc-reference-data-f552-residual-vectors]] — F-552 residual vectors of the interior load (ADR 0474).
- [[doc-reference-data-f422-j2-plasticity]] — F-422 J2 plasticity with hardening (ADR 0475).
- [[doc-reference-data-f417-random-vibration]] — F-417 random vibration by modal superposition (ADR 0477).
- [[doc-reference-data-f432-stress-recovery]] — F-432 stress recovery and energy-norm error estimate (ADR 0478).
- [[doc-reference-data-f423-nonlinear-transient]] — F-423 nonlinear transient dynamics (ADR 0479).
- [[doc-reference-data-f424-follower-arc-length]] — F-424 follower pressure, arc-length continuation (ADR 0480).
- [[doc-reference-data-f556-shell3-twisted-strip]] — F-556 Shell3 rest stiffness of the twisted strip against Hex20 (ADR 0483).
- [[doc-reference-data-f410-viscoelasticity]] — F-410 linear viscoelasticity (ADR 0485).
- [[doc-reference-data-f577-viscoelastic-harmonic]] — F-577 viscoelastic material in the direct harmonic solve (ADR 0491).
- [[doc-reference-data-f583-viscoelastic-stress]] — F-583 harmonic stress of a viscoelastic material (ADR 0494).
- [[doc-reference-data-f573-follower-shells]] — F-573 follower pressure on shells (ADR 0487).
- [[doc-reference-data-f557-prestress]] — F-557 pre-stress of a static result (ADR 0486).
- [[doc-reference-data-f411-creep]] — F-411 power-law creep with time hardening (ADR 0489).
- [[doc-reference-data-f578-nonlinear-shells]] — F-578 geometrically nonlinear shells (ADR 0490).
- [[doc-reference-data-f390-hex8-locking]] — F-390 Hex8 without locking (ADR 0493).
- [[doc-reference-data-f576-viscoelastic-time-stepping]] — F-576 quasi-static viscoelastic time stepping (ADR 0499).
- [[doc-reference-data-f592-temperature-history]] — F-592 temperature history on the quasi-static and transient drivers (ADR 0526).
- [[doc-reference-data-f585-corotated-shells]] — F-585 corotated shells in arc-length and nonlinear transient (ADR 0522).
- [[doc-reference-data-f610-hinged-roof]] — F-610 hinged cylindrical roof limit load against the published value (ADR 0716).
- [[doc-reference-data-f607-gonzalez-gradient]] — F-607 discrete gradient of Gonzalez in the energy-momentum step (ADR 0523).
- [[doc-reference-data-f588-hex8-linear-paths]] — F-588 Hex8 formulations on the remaining linear paths (ADR 0525).
- [[doc-reference-data-f425-hyperelastic]] — F-425 hyperelastic materials with mean-dilatation Hex8 (ADR 0541).
- [[doc-reference-data-f601-rigid-contact]] — F-601 rigid sphere and cylinder contact targets (ADR 0540).
- [[doc-reference-data-f598-warping-beam2]] — F-598 warping DOF of Beam2 and lateral-torsional buckling (ADR 0546).
- [[doc-reference-data-f627-monosymmetric-sections]] — F-627 monosymmetric thin-walled sections (ADR 0553).
- [[doc-reference-data-f643-load-height]] — F-643 load height and non-uniform moment for warping beams (ADR 0582).
- [[doc-reference-data-f663-distributed-load-height]] — F-663 distributed loads at a height on warping beams (ADR 0588).
- [[doc-reference-data-f670-prestressed-warping]] — F-670 load height on pre-stressed warping modes (ADR 0594).
- [[doc-reference-data-f673-monosymmetric-prestress]] — F-673 load height on pre-stressed modes of a monosymmetric beam (ADR 0597).
- [[doc-reference-data-f429-fracture]] — F-429 fracture parameters (ADR 0547).
- [[doc-reference-data-f632-elliptical-front]] — F-632 domain integral on an elliptical front (ADR 0605).
- [[doc-reference-data-f682-curved-fronts]] — F-682 interaction integral on curved crack fronts (ADR 0610).
- [[doc-reference-data-f689-closed-fronts]] — F-689 interaction integral on closed fronts (ADR 0618).
- [[doc-reference-data-f580-creep-bbar]] — F-580 creep on the B-bar Hex8 against a thick cylinder (ADR 0549).
- [[doc-reference-data-f581-arrhenius-creep]] — F-581 Arrhenius temperature factor for creep (ADR 0558).
- [[doc-reference-data-f653-beck-column]] — F-653 flutter of follower loads, Beck's column (ADR 0580).
- [[doc-reference-data-f695-plate-hole]] — F-695 converged peak stress of the two-layer plate with a hole (ADR 0627).

### Design loop, NACA 0012, cylinder and fan sections (latest)

- [[doc-reference-data-f698-gaussian-process]] — F-698 Gaussian-process surrogate and expected improvement sources and test functions (ADR 0621).
- [[doc-reference-data-f868-ladson-naca0012]] — ADR 0760 (F-868): Ladson NACA 0012 maximum lift for the 120-grit and 180-grit zones and the three-zone scatter.
- [[doc-reference-data-f869-cylinder-mean-drag]] — F-869 cylinder mean drag coefficient at Re 100 and the band of ADR 0761.
- [[doc-reference-data-f870-fan01-design-point]] — F-870 FAN-01 design point: the measurement, what its sources state, and the band of ADR 0762.

## Contradictions with the wiki

None with existing wiki pages. Contradictions and conflicts recorded within the docs themselves:

> [!warning] Contradiction
> The material-library section says "NIST WebBook data was deliberately not used (its terms were not reviewed)", while the real-gas EOS section embeds WebBook isotherm values; the doc itself adds a pointer to this, and [[doc-verification-report]] §1.6 notes NIST SRD 69 is copyrighted and that `realgas_nist.rs` and ADR 0007 copy about 60 numbers from the WebBook ([[doc-reference-data-material-library]], [[doc-reference-data-real-gas-eos]]). Within the docs.

> [!warning] Contradiction
> Gartling (1990) laminar step at Re 800: the in-repo 12.20 / 9.70 / 20.96 (MFIX-Exa tutorial, arXiv:2507.16509 Table 2) against a row labelled Gartling of 11.9 / 9.5 / 20.6 in Teruel and Fogliatto (AMCA 2013): "unresolved" ([[doc-reference-data-wave31c-reference-reading]], [[doc-reference-data-laminar-step]]). Within the cited literature, recorded by the docs.

> [!warning] Contradiction
> Cylinder Strouhal number at Re 100: the CSV row "Williamson (experiment) 0.165" is the 1996 review value as quoted in arXiv:2303.09262; "it differs from the 1989 fit by 0.4 %" (St(100) = 0.164335) ([[doc-reference-data-cylinder-wake]]). Within the docs, recorded as such.

> [!warning] Contradiction
> Taylor-Green kinetic-energy error: "The F-029 roadmap note quotes 0.38 percent, which matches neither" the old nor the corrected value ([[doc-reference-data-taylor-green]]). Within the docs (roadmap note stale).

> [!warning] Contradiction
> Adiabatic flame temperature: the reactor section compares 2225 K against CEA 2224.25 K, and the same secondary source also gives 2156.25 K ([[doc-reference-data-reactors-fwh]], [[doc-verification-report]] §2.4). Within the cited literature.

## Pages this source informs

- [[lid-driven-cavity]], [[differentially-heated-cavity]], [[cylinder-re100]], [[backward-facing-step]], [[flat-plate]], [[naca0012]], [[dfg-cylinder-benchmark]]
- [[fan-01]], [[validation-data-pack]], [[design-loop]], [[gci-richardson]], [[verification-vs-validation]]
- [[known-miss]], [[unverified]], [[provenance]], [[benchmark-descriptor]]
