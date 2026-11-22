---
type: source
tags: [adr-thread, structural, fem, nonlinear, plasticity, creep, viscoelasticity, orthotropy]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: structural solver, materials and nonlinear statics and dynamics

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 20 ADRs, 2026-11-13 to 2026-11-14. The later hyperelastic, incompatible-mode finite-strain, rule-L punch-path, F-bar creep and Newton-residual ADRs (F-425, F-571, F-596, F-597, F-607, F-620, F-685, F-747 ...) are in the other half of the category and may be added here.

The nonlinear route of [[rcfd-fem]]: the total Lagrangian Newton driver with load steps (F-397), its roll-back and extension points (the advanced-tier scaffold F-408), orthotropic and temperature-tabulated materials on every path, J2 plasticity, linear viscoelasticity (Prony series), power-law creep, follower loads with arc-length continuation, the nonlinear transient (Newton inside Newmark and generalised-alpha) and the energy-momentum step, and locking-free Hex8 (B-bar / F-bar) in the nonlinear driver.

## Current state

- **Newton driver** (`nonlinear::solve`, F-397, ADR 0404): total Lagrangian Tet4, Hex8, Tet10, Hex20 with St. Venant-Kirchhoff, tangent `K_mat(F) + K_sigma(S)`, step halving. **Partial**: A5 (linear limit at load factor 1e-6) met only after amendment 1; the roll-back of a failed attempt was then made observable by F-497 (ADR 0406, done).
- **Advanced-tier scaffold** (F-408, ADR 0440, partial): modules, material variants with a point context and a state layout, Shell3, the layup and section-stiffness hook, two extension points of the Newton driver, the complex type; D6, R2 to R4 met only after set-up amendments.
- **Orthotropy and tables:** orthotropic elasticity from nine constants with fixed, cylindrical or per-element axes and temperature tables on the linear static solve (F-409, ADR 0452, done); on the modal and transformation paths (F-537, ADR 0457, partial by an equivalent mutant) and on the rotating, buckling and transient paths (F-542, ADR 0459, partial; transient miss settled by F-545).
- **Inelastic materials:** J2 with isotropic and kinematic hardening, radial return and consistent tangent (F-422, ADR 0475, partial: residual hoop stress 2.6 percent against 2 percent and Newton order 1.27 to 1.44 against 1.8); the sphere re-run on 8 then 16 radial Hex20 layers (F-561, ADR 0512, partial; F-602, ADR 0518, done with a thin 7 percent E1 margin); a convergence-order reading that fits an active-set change (F-560, ADR 0515, partial). Linear viscoelasticity (F-410, ADR 0485, done), in the direct harmonic solve (F-577, ADR 0491, done; harmonic stress F-583, ADR 0494, done), in the damped modes at a reference frequency (F-582, ADR 0495, partial by MU7), quasi-static viscoelastic time stepping (F-576, ADR 0499, done). Power-law creep with time hardening on a minimal time-stepping driver (F-411, ADR 0489, done).
- **Follower loads and arc length** (F-424, ADR 0480, done): follower pressure with its load stiffness, centrifugal follower, arc-length continuation, modes about a nonlinear equilibrium.
- **Nonlinear transient:** Newton inside Newmark and generalised-alpha (F-423, ADR 0479, partial: free spinning bar momentum check missed by the trapezoidal rule's nonlinear instability); the Simo-Tarnow energy-momentum step (F-570, ADR 0514, partial: case-file key open).
- **B-bar / F-bar Hex8 in the nonlinear driver** (F-587, ADR 0506, partial: creep cylinder growth bound C4 6.0e-3 against 2e-3).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0404](../../docs/adr/0404-f397-registration-geometric-nonlinearity-total-lagrangian-newton.md) | 2026-11-13 | registration (hub) | F-397 | Total Lagrangian solids, Newton with load steps | partial; A5 after amendment 1, M7 not caught | 0406 |
| [0406](../../docs/adr/0406-f497-registration-nonlinear-rollback-of-a-failed-attempt.md) | 2026-11-13 | registration | F-497 | Roll-back of a failed attempt as a registered check | done (formal; R3 test defect amended) | |
| [0440](../../docs/adr/0440-f408-registration-advanced-tier-scaffold.md) | 2026-11-14 | scaffold (hub) | F-408 | Advanced-tier modules, material variants, point context, Newton extension points | partial; set-up amendments after runs | |
| [0452](../../docs/adr/0452-f409-registration-orthotropic-elasticity-and-temperature-tables.md) | 2026-11-14 | registration | F-409 | Orthotropic elasticity, material axes, temperature tables | done; ten mutations caught | 0457, 0459 |
| [0457](../../docs/adr/0457-f537-registration-orthotropic-and-tabulated-materials-on-modal-paths.md) | 2026-11-14 | registration | F-537 | Orthotropic and tabulated materials on modal and transformation paths | partial; Mu7 equivalent | |
| [0459](../../docs/adr/0459-f542-registration-orthotropic-and-tabulated-materials-on-rotating-buckling-transient.md) | 2026-11-14 | registration | F-542 | Same on rotating, buckling, transient paths | partial; TR2 on BLK missed | 0463 |
| [0475](../../docs/adr/0475-f422-registration-j2-plasticity-radial-return.md) | 2026-11-14 | registration (hub) | F-422 | J2 plasticity, radial return, consistent tangent | partial; S4 and O1 known misses | 0512, 0515 |
| [0479](../../docs/adr/0479-f423-registration-nonlinear-transient-newton-in-newmark-generalised-alpha.md) | 2026-11-14 | registration | F-423 | Newton inside Newmark and generalised-alpha | partial; MU7 caught only by N2b | 0514 |
| [0480](../../docs/adr/0480-f424-registration-follower-pressure-arc-length-tangent-modes.md) | 2026-11-14 | registration | F-424 | Follower pressure, arc-length, modes about a nonlinear equilibrium | done | |
| [0485](../../docs/adr/0485-f410-registration-linear-viscoelasticity-prony-series.md) | 2026-11-14 | registration | F-410 | Linear viscoelasticity (Prony), complex modulus, time-temperature shift | done; V1 2.50 percent against 3 | |
| [0489](../../docs/adr/0489-f411-registration-power-law-creep-time-hardening.md) | 2026-11-14 | registration | F-411 | Power-law creep, backward Euler radial return, quasi-static driver | done | |
| [0491](../../docs/adr/0491-f577-registration-viscoelastic-material-in-the-direct-harmonic-solve.md) | 2026-11-14 | registration | F-577 | Viscoelastic material in the direct harmonic solve | done at the discretisation limit 4e-3 | 0494 |
| [0494](../../docs/adr/0494-f583-registration-harmonic-stress-of-a-viscoelastic-material-in-the-direct-harmonic-solve.md) | 2026-11-14 | registration | F-583 | Harmonic stress of a viscoelastic material | done | |
| [0495](../../docs/adr/0495-f582-registration-viscoelastic-material-in-the-damped-modes-at-a-reference-frequency.md) | 2026-11-14 | registration | F-582 | Viscoelastic material in the damped modes at a reference frequency | partial by the letter (MU7); ADR recommends done | (F-586, other half) |
| [0499](../../docs/adr/0499-f576-registration-quasi-static-viscoelastic-time-stepping-and-a-temperature-in-the-nonlinear-transient-integrator.md) | 2026-11-14 | registration | F-576 | Quasi-static viscoelastic time stepping, uniform temperature in the nonlinear transient | done | |
| [0506](../../docs/adr/0506-f587-registration-b-bar-and-f-bar-hex8-in-the-nonlinear-driver.md) | 2026-11-14 | registration | F-587 | B-bar Hex8 in the nonlinear driver (variational F-bar) | partial; C4 6.0e-3 against 2e-3 | (F-597, other half) |
| [0512](../../docs/adr/0512-f561-registration-sphere-refined-octant-residual-hoop-stress.md) | 2026-11-14 | registration | F-561 | Thick J2 sphere on a cubed-sphere octant, residual hoop stress | partial; R1, E1 known misses | 0518 |
| [0514](../../docs/adr/0514-f570-registration-energy-momentum-conserving-step-for-nonlinear-transient-dynamics.md) | 2026-11-14 | registration | F-570 | Simo-Tarnow energy-momentum step | partial; case-file key open, F1e corrected set-up | |
| [0515](../../docs/adr/0515-f560-registration-newton-order-reading-for-an-active-set-change.md) | 2026-11-14 | registration | F-560 | Newton order reading that fits an active-set change (J2) | partial; MR4 not caught (later caught by F-606) | |
| [0518](../../docs/adr/0518-f602-registration-sphere-16-radial-layers-residual-hoop-stress.md) | 2026-11-14 | registration | F-602 | The J2 sphere with 16 radial Hex20 layers | done; E1 margin 7 percent | |

## Key numbers and limits

- F-397 (ADR 0404 roadmap note): "Tangent against central differences 2e-11 to 8e-11 (limit 1e-6), rigid rotation 2e-16 E l^2". Gap: "The linear-limit clause (load factor 1e-6) missed in its registered run and was met only after amendment 1 (2e-7)" ([[roadmap-structural-solver]]).
- F-422 gap: "Residual hoop stress at the bore is 2.6 percent against 2 percent. The order of the last three residuals is 1.27 to 1.44 against 1.8. Both are known misses".
- F-561 gap: "residual hoop stress at the bore is 2.70e-2 against the 2e-2 limit, and the elastic Lame hoop stress in the inner layer is 1.162e-2 against 1e-2. Cause is radial resolution of 8 Hex20 layers (16 layers gave 4.5e-3 and 9.3e-3)".
- F-587 gap: "Growth bound C4 from 5 t* to 20 t* is missed at p: 6.0e-3 against 2e-3 (finite-strain drift of the bore); met only at p/10 (1.06e-4)".
- F-577 (ADR 0491): "the acceptance's `1e-6` is out of reach at 40 elements: the dispersion of the linear element with consistent mass, `2.47e-3` at the peak of rod A".

## Open questions

- ADR 0404 Q1: whether A5 counts as met after the amendment (maintainer, per the F-397 gap).
- F-570: case-file key `step = energy_momentum` under a nonlinear transient study (maintainer).
- F-587: case-file key `formulation = bbar` covering the nonlinear paths (maintainer).
- ADR 0499 Q1: a case-file key for a quasi-static time study, options (a) none yet, (b) `{ times, loads = [...] }`.
- F-537, F-542, F-560, F-582: "The maintainer decides done" (roadmap gaps).

## Second half of the category: driver records, residual references and the energy-momentum step (ADRs 0521 to 0625, job p3-structural-b)

The other second-half nonlinear ADRs have their own pages: [[adr-thread-structural-hyperelastic-and-follower]] (hyperelastic materials, follower loads, flutter), [[adr-thread-structural-incompatible-hex8-rule-l]] (finite-strain incompatible-modes Hex8, rule L, the punch path, the indefinite-tangent and retry opt-ins) and [[adr-thread-structural-creep-and-temperature]] (F-bar creep drift, temperature on the nonlinear paths).

### Current state (second half)

- **Active-set record** of the Newton driver (ADR 0515) checked against a count made independently from the states on the J2 load and unload sphere (F-606, ADR 0521, done); it catches MR4 of F-560.
- **Energy-momentum step for hyperelastic materials.** `NlTransientOptions::algorithmic_stress` with `AlgorithmicStress { Auto, DiscreteGradient, Average }`, the discrete gradient of Gonzalez (F-607, ADR 0523: done by its rule, proposed partial because a library defect was corrected by amendment 2; the roadmap lists F-607 partial). `AlgorithmicStress::MaterialDiscreteGradient` takes F-425's neo-Hookean and Mooney-Rivlin materials (F-611, ADR 0544, done).
- **Residual references.** An opt-in `ResidualReference::InertiaTerms` for the nonlinear transient in free vibration (no external load) (F-631, ADR 0603, done) and on the energy-momentum step (F-681, ADR 0609, partial: its negative control NZ converged); a negative control off rest closes that gap (F-691, ADR 0625, done) and ADR 0625 recommends that the lead move F-681 to done, the maintainer's call.
- **Rounding floor under large translation and rotation** (F-571, ADR 0604, done): the floor is not in the total Lagrangian internal force; it comes from representing the total displacement through the step tangent `K_eff = K_T + M/(beta h^2)`, so the residual of the nearest float vector reaches `(eps/2)|(|K_eff| |u|)|` while the reference `max(|f_int|, |g|)` passes through zero every half period.
- **Stagnation acceptance** at that rounding bound, opt-in `NewtonOptions::stagnation` (F-690, ADR 0619): done as an opt-in. **The default flip was held by the maintainer on 2026-11-18**: "it moves existing results, and it waits until the fan path needs it. Not to be issued" (ADR 0619 answers; F-694 todo).

### ADRs (second half)

| ADR | Date | Kind | Item | Question | Outcome | Superseded by / followed by |
|---|---|---|---|---|---|---|
| [ADR 0521](../../docs/adr/0521-f606-registration-active-set-record-on-a-load-and-unload-solve.md) | 2026-11-14 | registration | F-606 (done) | Active-set record of the Newton driver on a J2 load/unload solve vs an independent count | done; MR4 of F-560 caught | — |
| [ADR 0523](../../docs/adr/0523-f607-registration-energy-momentum-step-for-hyperelastic-materials.md) | 2026-11-14 | registration | F-607 (partial) | Energy-momentum step for hyperelastic materials (Gonzalez discrete gradient) | done by rule, proposed partial (library defect fixed by amendment 2) | ADR 0544 |
| [ADR 0544](../../docs/adr/0544-f611-registration-energy-momentum-step-with-f425-hyperelastic-materials.md) | 2026-11-15 | registration | F-425 (partial), F-611 (done) | Energy-momentum step with F-425's hyperelastic materials | done | — |
| [ADR 0603](../../docs/adr/0603-f631-registration-residual-reference-of-the-newton-driver-in-free-vibration.md) | 2026-11-15 | registration | F-631 (done) | Residual reference of the Newton driver in free vibration (inertia-term reference) | done (opt-in) | ADR 0609 |
| [ADR 0604](../../docs/adr/0604-f571-registration-rounding-floor-of-the-newton-residual-under-translation-and-rotation.md) | 2026-11-15 | registration | F-571 (done) | Rounding floor of the TL Newton residual under large translation/rotation | done (diagnosis: floor from K_eff representation) | ADR 0619 |
| [ADR 0609](../../docs/adr/0609-f681-registration-the-inertia-term-residual-reference-on-the-energy-momentum-step.md) | 2026-11-12 | registration | F-681 (partial) | Inertia-term residual reference on the energy-momentum step | partial: NZ converged | ADR 0625 |
| [ADR 0619](../../docs/adr/0619-f690-registration-early-acceptance-at-the-rounding-bound-a-stagnation-test.md) | 2026-11-15 | registration | F-690 (partial) | Early acceptance at the rounding bound (stagnation test) | done (opt-in); default flip held 2026-11-18 | — |
| [ADR 0625](../../docs/adr/0625-f691-registration-a-negative-control-of-the-energy-momentum-inertia-term-reference-off-rest.md) | 2026-11-15 | registration | F-691 (done) | Negative control of the energy-momentum inertia-term reference off rest | done | — |

### Key numbers and limits (second half)

- Rounding of the total Lagrangian internal force under translation: `3e-11` to `1.1e-8 N` on S3; the residual reaches `0.03` to `0.58` of `(eps/2)|(|K_eff| |u|)|` over 60 steps (ADR 0604).
- Negative control off rest: one step centred on a zero crossing at `h = T_1/2621440` (ADR 0625).

### Open questions (second half)

- A rounding bound on the energy-momentum step (ADR 0609 Q2) and `InertiaTerms` as the default (ADR 0609 Q3): open, the maintainer's decision; ADR 0625 Q3 says both are unchanged.
- Whether F-681 moves to done on the strength of F-691 (ADR 0625 Q2): the maintainer's call.
- The bitwise chatter at iteration 0 of the active-set record (ADR 0521 Q1): recommendation keep the definition.

## Related

- [[rcfd-fem]], [[structural-solver]], [[roadmap-structural-solver]]
- [[adr-thread-structural-contact]] (contact uses the Newton driver), [[adr-thread-structural-beams-shells]]
- [[known-miss]], [[mutation-testing]]
