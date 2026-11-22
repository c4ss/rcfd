---
type: source
tags: [adr-thread, moving-mesh, overset, chimera, donor-interpolation, order]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: overset (chimera) coupling of a moving component mesh

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations, diagnoses with result addenda), 21 ADRs, 2026-11-07 to 2026-11-13.

F-134 (ADR 0219) built one rigidly moving component mesh over a static background in the library (`rcfd_mesh::overset`, `rcfd_fv::overset::OversetSolver`). Its acceptance (mass and drag of a moving cylinder) was met, but the Taylor-Green order checks missed. The thread that follows is a chain of diagnoses and opt-ins: the time error (outer iterations F-335, BDF2 F-338, F-345), the spatial order (the first-order term F-314, F-334, F-349; quadratic donors F-336, F-353; prescribed fringe flux F-445, F-450, F-479, F-503; pose-continuous donors F-468), the donor search (F-352, F-356) and warped three-dimensional meshes (F-357, F-359, F-368, F-472, F-487). See [[overset-mesh]].

## Current state

- **F-134 stays partial** (roadmap gap): "The case-tree, CLI and Studio wiring is not delivered, nor are several components, a moving background, hole cutting from wall geometry, non-orthogonal correctors and turbulence. Taylor-Green order misses with default linear donor stencils: 1.618 vs 1.7 (T1c), fine-level ratio 3.31 vs 2." The tree item F-312 and several components / moving background / hole cutting F-313 are todo.
- **Acceptance met** (ADR 0219): continuity `8.1e-17`, net outflow `7.5e-15` of $U D L$ over 400 steps, drag within `6.25e-3` of the body-fitted run at every step of `[2, 8]` (limit `5e-2`).
- **The misses are explained, not removed.** ADR 0232: T1 (`0.747`) and T2 at level 2 (`3.31`) are "a first-order time error of the library's moving-mesh PISO step (the lagged convecting flux ...), not of the overset coupling"; T1c (`1.618`) is "the truncation of the linear donor interpolation at one component fringe cell". ADR 0246 traces the first-order term of the calculated component cells to the same truncation; ADR 0340 names why the response stays below second order: the normal fringe velocity is imposed softly through a resistance $dJ = -\left(\dfrac{16 \nu}{3 h}\right) D$.
- **Opt-ins that restore order** (each default off, bit-identical): outer iterations `set_outer_iterations` (F-335: T2 at level 2 `1.862`, time error `1.69e-4`, not below `1.2e-4`); BDF2 `set_time_scheme(Bdf2)` (F-338 partial: ratio `2.46` against 4, T1c `1.664`); `DonorInterpolation::Quadratic` (F-336 done: order `1.993`, `2.000`); `DonorInterpolation::Continuous` (F-468 done); `set_fringe_flux(PrescribedOwnGradient)` (F-445 partial, "Not a default candidate") with `set_second_order_ring_gradient` (F-479, F-503 done: within `0.25` percent of the default coupling on RE and RL); `set_geometry_corrections` (F-368 done) and `set_consistent_geometry_corrections` (F-487 done: vortex on B3d `1.21` and `1.41` times its B3 error, limit 2).
- **Default questions are all open for the maintainer:** `Quadratic` as default (ADR 0251 Q1, ADR 0259 Q1: recommendation keep `Linear`), outer iterations by default (ADR 0238 Q5), BDF2 by default (ADR 0241 Q6), geometry corrections by default (ADR 0367 Q1), `Continuous` (ADR 0396 Q4: keep `Linear`), the ring gradient as G's behaviour (ADR 0418 Q1: recommends making it so, as a separate item).
- **The BDF2 remainder on a moving overset mesh** (kind changes next to the hole, ADR 0359 event 2) is accepted until a case needs it (ADR 0379 Q1, ADR 0396 Q1).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0219](../../docs/adr/0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md) | 2026-11-07 | registration (hub) | F-134 | Overset coupling of a moving component mesh, in the library | partial; A1, A2 met; T1 `0.747`, T2 level 2 `3.313`, T1c `1.618` missed | |
| [0232](../../docs/adr/0232-f314-registration-overset-observed-order-miss.md) | 2026-11-12 | diagnosis | F-314 | Cause of the observed-order miss | partial; causes named, none removed; first-order term of the calculated component cells found | 0246 |
| [0238](../../docs/adr/0238-f335-registration-overset-outer-iterations.md) | 2026-11-12 | registration | F-335 | Outer iterations on the overset path | done by branch (b) with one number missed (`1.69e-4` against `1.2e-4`); roadmap partial (Q1 the maintainer's) | 0241 |
| [0241](../../docs/adr/0241-f338-registration-overset-bdf2.md) | 2026-11-12 | registration | F-338 | BDF2 on the overset path | partial; D2 ratio `2.46` against 4, T1c-b `1.664` against `1.7`; remainder located in the library BDF2 step at rest (`5.13e-6`, ratio `2.30`) | 0379 |
| [0246](../../docs/adr/0246-f334-registration-overset-first-order-term.md) | 2026-11-12 | diagnosis | F-334 | First-order term of the calculated component cells | partial; RL orders `1.277`, `1.166`, `1.626` against `1.7`; source the linear donor truncation (TQ `1.922` to `1.950` with a test third-order interpolation) | 0340 |
| [0251](../../docs/adr/0251-f336-registration-overset-quadratic-donor-interpolation.md) | 2026-11-12 | registration | F-336 | Donor interpolation that reproduces quadratic fields (opt-in) | done; A1 `1.993`, `2.000`; A2 `1.916`, `1.924`, `1.952`; RL rms larger on coarse levels (`2.07`, `1.33`); found the non-planar-face donor gap | 0253, 0259 |
| [0253](../../docs/adr/0253-f352-registration-overset-donor-search-on-non-planar-faces.md) | 2026-11-12 | registration | F-352 | Donor search on cells with non-planar faces | done (kinematic, one warped family) | 0260 |
| [0255](../../docs/adr/0255-f357-registration-uniform-flow-on-a-warped-overset-mesh.md) | 2026-11-12 | registration | F-357 | Uniform flow on a warped 3-D overset mesh B3d | done; velocity `7.7e-15`, pressure `1.0e-13` | 0264 |
| [0259](../../docs/adr/0259-f353-registration-moving-cylinder-with-quadratic-donors.md) | 2026-11-12 | registration | F-353 | Moving cylinder TR with quadratic donors | done; drag within `5.86e-3` of body-fitted (linear `6.25e-3`); recommendation against the default | |
| [0260](../../docs/adr/0260-f356-registration-overset-donor-search-on-non-convex-cells.md) | 2026-11-12 | registration | F-356 | Donor search on non-convex cells with planar faces | done (kinematic, prism meshes) | |
| [0264](../../docs/adr/0264-f359-registration-linear-shear-flow-on-a-warped-overset-mesh.md) | 2026-11-12 | registration | F-359 | Linear shear flow on B3d | partial; ratio B3d / B3 `15.6` against 2; cause the single-mesh operators on warped cells, not the coupling | 0367 |
| [0340](../../docs/adr/0340-f349-registration-overset-background-fringe-response.md) | 2026-11-12 | diagnosis | F-349 | Response to the background-fringe truncation | partial; orders `1.198`, `1.068`, `1.598`, `1.469`; cause the soft imposition of the fringe velocity | 0347 |
| [0347](../../docs/adr/0347-f445-registration-overset-prescribed-fringe-flux.md) | 2026-11-12 | registration | F-445 | Prescribed flux on calculated-fringe faces (opt-in) | partial; R_b orders `1.962`, `1.789` on RE, but error `1.6` to `1.8` times the default with linear stencils; "not a candidate for a default" | 0376, 0409 |
| [0367](../../docs/adr/0367-f368-registration-overset-skewness-and-non-orthogonal-corrections.md) | 2026-11-12 | registration | F-368 | Opt-in skewness and non-orthogonal corrections in the overset step | done; no amendment | 0382 |
| [0376](../../docs/adr/0376-f450-registration-overset-prescribed-fringe-flux-with-a-moving-component.md) | 2026-11-12 | registration | F-450 | Prescribed fringe flux with a moving, turning component | partial (Rule I); level-1 error `1.2636` times the default against `1.2` | 0409 |
| [0379](../../docs/adr/0379-f345-registration-bdf2-for-the-library-moving-mesh-step-and-the-follow-conditions.md) | 2026-11-13 | registration | F-345 | BDF2 for the library's moving-mesh step and three-level follow conditions | partial; every clause met but A-TR `1.09` against 4 (kind changes next to the hole) | |
| [0382](../../docs/adr/0382-f472-registration-overset-geometry-corrections-with-a-varying-pressure.md) | 2026-11-13 | diagnosis | F-472 | Geometry corrections with a varying pressure on B3d | partial; ratios `3.31`, `6.80` against 2; cause the uncorrected Green-Gauss pressure gradient (`63 %` of the B3d error) | 0387 |
| [0387](../../docs/adr/0387-f487-registration-overset-consistent-cell-pressure-gradient.md) | 2026-11-13 | registration | F-487 | Consistent cell pressure gradient in the geometry corrections | done; `1.21`, `1.41` against 2; new entry point `set_consistent_geometry_corrections` | |
| [0396](../../docs/adr/0396-f468-registration-overset-pose-continuous-donor-stencil.md) | 2026-11-13 | registration | F-468 | Pose-continuous donor stencil (opt-in `Continuous`) | done; removes event 1 of ADR 0359, event 2 remains (`1.14e-5`, ratio `2.14`) | |
| [0409](../../docs/adr/0409-f479-registration-overset-second-order-ring-gradient.md) | 2026-11-13 | registration | F-479 | Second-order own-part ring gradient under the prescribed fringe flux | done; F-450's level-1 ratio falls from `1.2636` to `1.0000` | 0418 |
| [0418](../../docs/adr/0418-f503-registration-ring-gradient-on-re-and-rl.md) | 2026-11-13 | registration | F-503 | The ring gradient on RE and RL | done; rms within `0.25` percent of the default coupling at levels 1 to 3; R_b orders on RE `1.964`, `1.789` | |

## Key numbers and limits

- ADR 0219 TR: Re 40 cylinder ($D = 1$) translating 7.5 diameters through a static Cartesian background; `MassMode::Conservative`. T2c (dt proportional to $h^2$) `1.404`, `1.821` met.
- ADR 0238: three outer iterations cost "a factor 2.6 in wall time on TG_2" (Q5).
- ADR 0259 Q2: step-to-step drag oscillation, rms `1.8e-3` of $C_D$ with linear, `1.0e-3` with quadratic, body-fitted `7e-6`; heavy record 288 s ignored, fast test 10 s in the gate.
- ADR 0264 Q1: B3d is "a strong distortion ($0.15\,h$ random, faces warped up to $0.14\,h$)"; error of a linear field `1.4e-2`.
- ADR 0382: Green-Gauss gradient of a linear pressure on B3d off by $0.195\,\lvert c\rvert$; a pressure error of $4.3\times 10^{-2}\, V^2$ common to all runs is not diagnosed (Q3, recorded).
- ADR 0251 Q4: the two gate order tests take "about seven minutes on a loaded machine".

## Open questions

- ADR 0219 Q1: case-tree wiring (F-312, todo).
- ADR 0251 Q1 / ADR 0259 Q1 / ADR 0246 Q3: `Quadratic` as the default (recommendation keep `Linear`).
- ADR 0238 Q1, Q5; ADR 0241 Q6; ADR 0367 Q1; ADR 0396 Q4; ADR 0409 Q1 / ADR 0418 Q1: statuses and defaults, the maintainer's.
- ADR 0253 Q1 / ADR 0260 Q2: the triangulated donor test for every cell (recommendation close as "keep the plane test for convex flat cells").
- ADR 0260 Q1: refuse what the locator cannot triangulate (recommendation (b), refuse in `Overset::new`).
- ADR 0264 Q2: the coupling's share of the error on the planar B3 (F-369, todo).
- ADR 0376 Q1: F-450 done by the alternative clause or partial.

## Related

- [[overset-mesh]], [[dynamic-mesh]], [[piso]], [[mesh-quality]]
- [[adr-thread-immersed-solid]] (the other non-body-fitted moving-body path)
- [[roadmap-moving-meshes-rotors-and-coupling]]
- [[doc-dynamic-mesh]]
- [[adr-thread-bdf2-step-consistent-face-flux]] (F-344 and F-442, ADRs 0245 and 0359: the BDF2 remainder of the library step and of the motion through the coupling), [[adr-thread-amg-coarse-lu]] (ADR 0748: nine overset fingerprints moved with the corrected coarsest LU)
