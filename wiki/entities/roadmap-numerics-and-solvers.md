---
type: entity
tags: [roadmap, area]
sources: [doc-roadmap-json, doc-roadmap-overview, adr-thread-piso-stability-consistent-correctors, adr-thread-bdf2-step-consistent-face-flux, adr-thread-amg-coarse-lu, adr-thread-gradient-options, adr-thread-steady-kernel-warped-mesh, adr-thread-setup-warnings-and-time-step]
created: 2026-11-22
updated: 2026-11-25
status: draft
---

# Roadmap area: Numerics and solvers

**What it is:** the 'Numerics and solvers' line of work (Solver family) of the [[roadmap]], with every [[f-item]] in it ([[doc-roadmap-json]]).
**Where:** `docs/roadmap.json` (source of truth), categories in `scripts/roadmap_progress.py`.

## Synthesis

The P3 ADRs of this area (24, 2026-11-05 to 2026-11-20) are almost all diagnoses of the incompressible kernels followed by opt-in remedies that leave the defaults bit for bit; only one default moved.

- **Transient stability (the largest thread).** The default PISO/PIMPLE step has an alternating pressure mode above a cell diffusion number of about 2.5 to 4 (F-299 thin gap, F-530 channel). The remedy is the opt-in consistent correctors with a case key `[solver.consistent_correctors]`, extended to periodic faces and one settings value, plus outer-iteration control and an onset refusal; the default flip is "Held" by the maintainer (2026-11-18) until the fan path needs it ([[adr-thread-piso-stability-consistent-correctors]]).
- **Time order.** The step-size dependence of the Rhie-Chow face flux (F-344) is removable by the opt-in `[solver.rhie_chow] step_consistent` (merged as opt-in by the maintainer in person, 2026-11-12); the fractional step got its own opt-in (F-454); on a moving overset mesh a remainder from cells changing kind stays ([[adr-thread-bdf2-step-consistent-face-flux]]).
- **Linear solver.** The one default change: the corrected order of the coarsest dense LU of the scalar AMG (F-471, ADR 0743), after AMG-CG lost definiteness on an immersed run (F-474); every moved fingerprint was approved one by one ([[adr-thread-amg-coarse-lu]], [[amg]]). The F-474 gap still reads as if the default were not switched (contradiction noted on the thread page).
- **Non-orthogonal meshes.** The gradient options reach every equation through `solver.gradient_*` and `[solver.gradient_equations]` with no accuracy benefit claimed ([[adr-thread-gradient-options]]); the steady kernel's divergence on the warped mesh W is fixed by the opt-in `implicit_non_orth_pressure` (F-285), while the pressure-interpolation variants of F-246 stay partial ([[adr-thread-steady-kernel-warped-mesh]]).
- **Pattern.** Most partials are partial by the registered rule (a mutation caught only after an amendment, a supporting check missed) rather than by a missed acceptance; the open decisions are defaults that would move existing results ([[adr-thread-piso-stability-consistent-correctors]], [[adr-thread-bdf2-step-consistent-face-flux]]).
- **Early waves (ADRs 0010 to 0105, added by p3-records-a).** The component-consistent diagonal became an option, not the default, because it moves the `packed_bed` hash (ADR 0010). Residual floors were "a normalisation artefact" with `residual_norm` as the remedy (ADR 0023). The Taylor-Green BDF2 floor on the iterative path is the commutator of the variable `rAU` with the face interpolation (ADR 0073), and the fixes of waves 34B to 36A stayed library opt-ins or were removed (ADRs 0069, 0073, 0076, 0080). F-209 adaptive stepping stays partial through four close-outs (ADRs 0050, 0094, 0101, 0105) ([[adr-thread-early-numerics-transient-accuracy]]).
- **Steady robustness (added by p3-records-a).** A diverging steady run now stops (ADR 0077); the opt-ins `linear_upwind_limiter = "barth_jespersen"` (ADR 0081), `velocity_correction = "face_flux"` (ADR 0084) and `simplec_boundary_compensation` (ADR 0102, F-248 done) each answer one located divergence ([[adr-thread-steady-solver-robustness]]).

## ADR threads

- [[adr-thread-piso-stability-consistent-correctors]] — F-299, F-301, F-322, F-367, F-530, F-531, F-535, F-536, F-538, F-550 (ADRs 0214, 0223, 0258, 0360, 0451, 0455, 0458, 0471, 0613, 0617)
- [[adr-thread-bdf2-step-consistent-face-flux]] — F-344, F-443, F-454, F-442 (ADRs 0245, 0352, 0356, 0359)
- [[adr-thread-amg-coarse-lu]] — F-474, F-471 (ADRs 0369, 0743, 0747, 0748)
- [[adr-thread-gradient-options]] — F-256, F-291 (and F-289, F-298 of heat) (ADRs 0190, 0204, 0202, 0221)
- [[adr-thread-steady-kernel-warped-mesh]] — F-246, F-285 (ADRs 0193, 0196, 0198)
- [[adr-thread-immersed-solid]] — F-309 (ADR 0234) added to the moving-meshes thread
- [[adr-thread-early-numerics-transient-accuracy]] — F-106, F-200, F-201, F-202, F-204, F-205, F-208, F-209, F-210, F-211, F-301, F-344 (18 ADRs, 0010 to 0105)
- [[adr-thread-steady-solver-robustness]] — F-248 and the unnumbered robustness waves (ADRs 0042, 0062, 0077, 0081, 0084, 0095, 0102, 0105)
- [[adr-thread-setup-warnings-and-time-step]] — end-time remainder step, time step at run start, SV301 wiring and settled step, SV302, SV202 text, `rcfd map --warm-start` (ADRs 0145, 0153, 0160, 0172, 0235; F-328, F-329, F-269; added by p3-records-b)
- The wave-48 adaptive correctors (ADRs 0154, 0156, 0158, 0162; F-301, F-321) added as a section to [[adr-thread-piso-stability-consistent-correctors]] (p3-records-b)

## Items

<!-- generated by tools/gen_roadmap_pages.py: begin (do not edit inside) -->

Generated from `docs/roadmap.json` and `scripts/roadmap_progress.py` at 21e04d2e on 2026-11-25.

**53 items:** 29 done, 19 partial, 5 not started.

| Item | Title | Status | ADRs | Milestones | Gap (what is missing now) |
|---|---|---|---|---|---|
| F-003 | Core types: scalar precision (f64 default, generic f32), units/quantities, error type, logging | done |  |  |  |
| F-020 | Field storage and boundary-condition framework (fixed value, zero gradient, mixed, symmetry, periodic) | done |  | vortex-shedding, fan, aerofoil-lift, lid-driven-cavity, backward-facing-step, flat-plate, heated-cavity, packed-bed |  |
| F-021 | Gradient schemes: Green-Gauss (cell, node), least squares; limiters (Barth-Jespersen, Venkatakrishnan) | done |  | vortex-shedding, fan, aerofoil-lift, lid-driven-cavity, backward-facing-step, flat-plate, heated-cavity, packed-bed |  |
| F-022 | Convection schemes: upwind, central, linear-upwind, QUICK/MUSCL, TVD limiters, deferred correction | done |  | vortex-shedding, fan, aerofoil-lift, lid-driven-cavity, backward-facing-step, flat-plate, heated-cavity, shock-tube-and-wedge, packed-bed |  |
| F-023 | Diffusion with non-orthogonal correction; Laplacian operator | done |  | vortex-shedding, fan, aerofoil-lift, lid-driven-cavity, backward-facing-step, flat-plate, heated-cavity, packed-bed |  |
| F-024 | Time integration: implicit Euler, BDF2/Crank-Nicolson, CFL-based adaptive time step | done |  | vortex-shedding |  |
| F-025 | Sparse matrix (CSR/LDU) and assembly infrastructure with rayon parallelism | done |  | vortex-shedding, fan, aerofoil-lift, lid-driven-cavity, backward-facing-step, flat-plate, heated-cavity, packed-bed, fan-structure |  |
| F-026 | Linear solvers: CG, BiCGStab, GMRES with Jacobi/ILU(0)/DIC preconditioners | done |  | vortex-shedding, fan, aerofoil-lift, lid-driven-cavity, backward-facing-step, flat-plate, heated-cavity, packed-bed |  |
| F-027 | Algebraic multigrid (aggregation AMG) as preconditioner and solver | partial |  | vortex-shedding | The 1M-cell Poisson result uses W-cycles (14 cycles to 1e-6, limit under 15, marginal) in an ignored test; plain V-cycles were not shown to meet the criterion (rate degrades from 0.19 at 4k to 0.45 at 1M). Needs a V-cycle result or a changed criterion, plus a gate in the default test run. |
| F-028 | Incompressible steady laminar SIMPLE/SIMPLEC (collocated, Rhie-Chow) | done |  | vortex-shedding, fan, aerofoil-lift, lid-driven-cavity, backward-facing-step, flat-plate, heated-cavity, packed-bed |  |
| F-029 | Transient incompressible PISO/PIMPLE | done |  | vortex-shedding |  |
| F-030 | Residual monitors, convergence criteria, under-relaxation controls, checkpoint/restart | done |  | fan, aerofoil-lift, lid-driven-cavity, backward-facing-step, flat-plate, heated-cavity, packed-bed |  |
| F-031 | Porous media, momentum sources, MRF (rotating frame) source terms | done |  | fan, packed-bed |  |
| F-032 | Coupled pressure-velocity solver (block-coupled) with AMG | done |  |  |  |
| F-105 | Profile and table inputs: file/CSV profile boundary conditions and 1D/3D interpolated property tables | done |  |  |  |
| F-106 | Pseudo-transient (local time-stepping) steady solver option with automatic timescale selection | partial |  | fan | Not fully met: plain SIMPLEC now also converges on the cavity, and the Re_h = 400 step still needs a specific scheme and coupling combination (pseudo local cfl 5 does not converge with vanLeer or linearUpwind). The automatic ser and ser2 pseudo-step controllers do not converge the registered cavity and step cases. |
| F-107 | Periodic boundaries: translational and rotational periodicity (conformal) in mesh and solver | done |  | fan |  |
| F-135 | Viscoelastic constitutive models (Oldroyd-B, Giesekus, PTT, FENE-P) with log-conformation stabilisation | partial |  |  | The planar contraction comparison with analytic Oldroyd-B is not attempted. Not delivered: case-tree wiring, stress inflow and outflow boundary conditions, second-order stress transport, 3-D, high-Wi stability, and a verified FENE-P normalisation. Known misses: V1 instantaneous metric 8.4e-9 vs 1e-9, recalled Giesekus closed form wrong (5.1e-3), P3 centre-velocity sign. |
| F-201 | Zone source terms and fixed values for every transported quantity (mass, momentum, energy, species, scalars, turbulence quantities) and fixed-value cells | done |  |  |  |
| F-202 | User-defined transport equations: diffusive, Poisson, algebraic and vector variants with expression-defined coefficients | done |  |  |  |
| F-204 | Wall motion and shear conditions: translating and counter-rotating walls, specified shear, partial slip (specularity), Marangoni stress | done |  |  |  |
| F-205 | Initialisation methods: potential-flow initial solve, coarse-level pre-solve, region patching by registers, localised turbulence initialisation | done |  |  |  |
| F-208 | Numerics robustness options for poor meshes: warped-face gradient correction, secondary gradient limiting, boundary-gradient treatments, pressure-interpolation variants, solution limits | done |  |  |  |
| F-209 | Adaptive time stepping and convergence accelerators: error-estimator steps, period-based steps, RMS/MAX Courant criteria, local timescale factors, residual smoothing, extrapolated step initialisation, nonlinear (Aitken-type) acceleration, stretched-mesh pseudo-time acceleration | partial |  |  | The flat-plate clause is open: a high-aspect-ratio boundary-layer mesh converging to Blasius and turbulent skin friction within the unaccelerated tolerance is not shown. Residual smoothing, local timescale factors, stretched-mesh pseudo-time acceleration and Aitken in the outer loop are not delivered. The case path still needs solver.dt, so error control free of a chosen step is shown only in the library. |
| F-210 | Non-iterative time advancement (fractional-step) option for transient flow | done |  | vortex-shedding |  |
| F-211 | Advection and transient scheme additions: high-resolution (bounded blended), compressive, specified blend factor, bounded central differencing, bounded second-order time scheme | done |  |  |  |
| F-246 | Pressure-interpolation variants for poor meshes | partial | 0193 |  | Known misses with limits unchanged: the default and both variants diverge on F-208's warped mesh W from n = 32, so the warped-mesh clause is not met (a diagnosis item F-285 was opened). body_force_weighted raises near-orthogonal velocity error 1.342x (G) and 1.150x (G3) vs limit 1.10. Open ADR questions Q1 to Q7 remain. |
| F-248 | SIMPLEC robustness: velocity inlet with a forward vent (K > 0) diverges from the first iteration | done |  |  |  |
| F-256 | Case-tree wiring of the gradient options | done | 0190 |  |  |
| F-285 | Default steady kernel diverging on F-208's warped mesh W from n = 32: diagnose and fix | done | 0198 |  |  |
| F-291 | Gradient options on turbulence transport, momentum, pressure and the electric potential | done | 0204 |  |  |
| F-299 | PISO/PIMPLE instability at large cell diffusion number across a thin gap | partial | 0214 |  | The default path is still unstable: the known miss of F-244 still fails by construction, because the removal is only the library opt-in consistent_correctors (ADR 0214 Q1). Open: a refusal on the moving-mesh default path (Q3) and a case key (Q4). |
| F-301 | PISO consistent correctors: moving-mesh lag estimate, case key and benchmarks | partial | 0223 | vortex-shedding | Missing: case-level wiring of the moving-mesh lag estimate (SV301 and adaptive count for dynamic meshes; ADR 0156's refusal test A9b would change, Q2); the periodic pressure identity S1p3 (2.49e-10 of the range against 1e-10); known misses DC4 and DC5; the decision on the default (Q1), one flag (Q3), the remaining refusals (Q5). |
| F-309 | Second-order immersed forcing: fluid-solid face flux at the surface crossing | done | 0234 |  |  |
| F-321 | Consistent correctors on the moving-mesh case path | todo |  |  |  |
| F-322 | Consistent correctors: fold the flags into one key and extend the refused combinations | partial | 0258 |  | Missing: the case table has only enabled (schema golden and docs/case-schema.md are the maintainer's); sim.rs still writes its own errors instead of calling conflicts; overset.rs reads the older field. Still refused: new flags with periodic faces or a moving mesh (F-321), non-orthogonal correctors with pressure gradient options, the fractional step. |
| F-344 | BDF2 PISO: the step-size-dependent remainder of the library's BDF2 step | partial | 0245 |  | Missed: D2 on TG_2 is 2.454 against 4 (the moving coupled meshes carry another term, named in ADR 0359); R2b on SECT-M is 0.578 against the default 0.538 (the seam floor of ADR 0076). The opt-in is not the default; step_sliding, the fractional step and coupled faces keep the default. ADR 0245 Q1 and Q3 are open. |
| F-366 | Consistent correctors: the case table carries every flag | todo |  |  |  |
| F-367 | Consistent correctors: the new combinations with periodic faces | partial | 0360 |  | The opt-in consistent_periodic_extensions is not yet folded into ConsistentCorrectors (ADR 0360 Q1) and there is no case key. The component-diagonal and all-together channel checks met only after amendment 1. |
| F-442 | Overset BDF2: the step-size-dependent remainder made by the motion through the coupling | partial | 0359 |  | The remainder is traced to two events, but no opt-in reaches D2 of at least 4 on TG_2: the most continuous stencil gives 1.845 because the cell kind change next to the hole (event 2) remains. The note says F-468 removes event 1 (ratio 5.07) and event 2 remains (ratio 2.14). The events were confirmed only through an amendment after the first runs. |
| F-443 | Transient step: the step-consistent face coefficient on the remaining paths | done | 0352 |  |  |
| F-454 | Fractional step: a step-consistent face flux for the projection | partial | 0356 |  | The note says every acceptance clause is met. Mutation MF5 is equivalent by construction and cannot be caught. Making the opt-in the default (Q3) and done are the maintainer's call. The Chorin form is not changed, and no case key exists. |
| F-467 | Consistent correctors: fold the periodic opt-in into the flags | todo |  |  |  |
| F-471 | Scalar AMG coarsest dense LU: row interchanges applied before the forward substitution | done | 0743, 0747, 0748 |  |  |
| F-474 | AMG pressure preconditioner: loss of definiteness on the immersed T system | partial | 0369 |  | The default is not switched: AMG-CG converges on the stopping system only with the opt-in AmgSettings::coarse_lu_interchanges_first, so the default AMG is not repaired. Open: switching the default (maintainer; recommended); with it on a scratch copy no registered hash moves. |
| F-530 | Stability of the non-iterated PISO step at large diffusion numbers | partial | 0451 |  | The default step still diverges: removal is only by the opt-in consistent correctors with 8 outer iterations (a count taken from a diagnosis, matched only after amendment 1). Registered misses: the 16-cell rate estimate and growth measurement, the impedance with the option alone, one periodicity tolerance. Open: ADR 0451 Q1 to Q3. |
| F-531 | Viscous flux of the Symmetry condition for flow tangential to the plane | partial | 0455 |  | Registered check Q4 (the default's excess with ten outer iterations against the infinite-corrector sink 4 nu/(3 h^2)) missed on a defective set-up and its corrected run by 7.9e-2 against 5e-2. The default is unchanged; removal is only through the opt-in component_diagonal corrected. Open: ADR 0455 Q1, Q2. Follow-up F-538. |
| F-535 | Opt-in refusal of the PISO step at the pressure-lag onset, naming the diffusion number | partial | 0471 |  | The agent did not finish the complete rcfd-fv suite, and one mutation (the empty-direction filter) was caught only by check L3b added by amendment. Limits: the estimate depends on the iteration count (0.962 at 20, 1.0003 at 200), first step only, not for moving meshes or the fractional step, no case key. Open: ADR 0471 Q1 to Q4. |
| F-536 | Outer-iteration criterion for the consistent-corrector step at large diffusion numbers | partial | 0458 |  | Known miss K1: 10 of 250 steps at nu dt/dy^2 = 100 and at 50 reached the cap of 50 outer iterations, because the criterion's scale nearly vanishes at the pressure extremes. F-550 (ADR 0617) later removed this with a 18-step window. Open: ADR 0458 Q1 to Q4. |
| F-538 | The averaged component diagonal in the relaxed PIMPLE loop | partial | 0613 |  | Open: ADR 0613 Q1 (the transient default, as ADR 0455 Q1). The registered model omitted c_x and missed C1 by 1.07e-2 (limit 5e-3) and the Symmetry parts of C3, C4, C6; the limits were met only after amendment 1, and the N 50 case is recorded as a known miss. |
| F-550 | A non-vanishing scale for the outer-iteration criterion | partial | 0617 |  | Every acceptance clause is reported met. Partial only because one mutation (the window with None) was equivalent as applied and its corrected form, caught by W1, was registered after the run. Open: ADR 0617 Q1 to Q4. Case-file keys are in F-692. |
| F-692 | Case-file keys for the outer-iteration criterion and its window | todo |  |  |  |
| F-738 | Corrected component diagonal as the default | todo |  |  |  |

<!-- generated by tools/gen_roadmap_pages.py: end -->
