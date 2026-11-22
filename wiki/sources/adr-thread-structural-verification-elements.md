---
type: source
tags: [adr-thread, structural, fem, verification, elements]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: structural solver, element verification and meshes

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 12 ADRs, 2026-11-12 to 2026-11-14.

Follow-ups of the basic tier (plan ADR 0270) and the intermediate tier (plan ADR 0290) of [[rcfd-fem]] that verify the solid elements Tet4, Hex8, Tet10 and Hex20: manufactured solutions on distorted meshes, the Hex8 patch test on moved meshes, the Hex20 face loads and closed-form benchmarks, the solid mesh from the repository's meshes, curved Hex20, locking-free Hex8, the static residual of the direct solve, and a patch-recovery error estimate.

## Current state

- The element library is verified against closed forms and observed orders, not against measurements. F-379 (ADR 0433, cantilever in bending and thick-walled cylinder) and F-390 (ADR 0493, opt-in B-bar and incompatible-mode Hex8 per element block) are **done** with every check met on a single registered run.
- Distorted meshes: F-459 (ADR 0355) met the observed orders of Tet4, Hex8 and Tet10 with moved interior nodes; F-461 (ADR 0358) met the Hex8 patch test on a moved mesh but stays **partial** (mutation M3, unmoved mesh, caught only by a rounding artifact); F-464 (ADR 0362) closed that with a check of the stored mesh (done). F-458 (ADR 0365) fixes the Tet4 traction-case L2 order 1.92 on the pair 16 -> 32 as a floor-protected regression check.
- Hex20: face loads through the model path F-484 (ADR 0383, done) and the cantilever with beam-theory support and refined bore F-485 (ADR 0385, done); curved Hex20 on the quarter annulus F-508 (ADR 0415) **partial** by one equivalent mutation.
- Meshes: F-371 (ADR 0413) builds solid meshes from the repository's meshes (hexahedra and tetrahedra from face lists, the tet mesher's output, patches as surface sets, mid-edge nodes projected): done.
- Open: F-455 (ADR 0397) static residual and one step of iterative refinement is **partial**: the registered 1e-14 lies below the floor of double precision; the maintainer must decide a re-wording (ADR 0397 Q1, Q2). F-432 (ADR 0478) patch recovery and energy-norm estimate is **partial** with three misses (follow-up F-568 in the other half of the category).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0355](../../docs/adr/0355-f459-registration-manufactured-solution-on-distorted-meshes.md) | 2026-11-12 | registration | F-459 | Orders of Tet4, Hex8, Tet10 with moved interior nodes, and the Hex8 stiffness rule | done; M1 caught narrowly (L2 1.7577 against 1.8) | |
| [0358](../../docs/adr/0358-f461-registration-hex8-patch-test-on-moved-mesh.md) | 2026-11-12 | registration | F-461 | Hex8 constant-strain patch test on a moved mesh | ADR done, item partial (M3 caught only by a rounding artifact of V1) | 0362 |
| [0362](../../docs/adr/0362-f464-registration-moved-mesh-stored-check.md) | 2026-11-12 | registration | F-464 | Moved-mesh check measures the stored mesh | done; S0 to S5 on all 18 meshes, MU1 to MU9 caught | |
| [0365](../../docs/adr/0365-f458-registration-tet4-traction-case-order-16-to-32.md) | 2026-11-12 | registration (regression record) | F-458 | Tet4 traction-case L2 order on 16 -> 32 | done; order 1.92 fixed as floor-protected check (value known before, disclosed) | |
| [0383](../../docs/adr/0383-f484-registration-hex20-face-loads-through-the-model-path.md) | 2026-11-13 | registration | F-484 | Hex20 face loads through the model path | done; one old test (si_hex20 L3) left to the maintainer | |
| [0385](../../docs/adr/0385-f485-registration-hex20-cantilever-beam-support-and-refined-bore.md) | 2026-11-13 | registration | F-485 | Hex20 cantilever with beam-theory support, bore stress on refined mesh | done; ADR 0292 known-miss records stay | |
| [0397](../../docs/adr/0397-f455-registration-static-residual-and-iterative-refinement.md) | 2026-11-13 | registration | F-455 | Residual of the direct static solve on slender cantilevers, one refinement step | partial; A2, A3 below double-precision floor, A5 known miss | |
| [0413](../../docs/adr/0413-f371-registration-solid-mesh-from-existing-meshes.md) | 2026-11-13 | registration | F-371 | Solid mesh from the repository's meshes | done; margins >= 300 on rounding limits | |
| [0415](../../docs/adr/0415-f508-registration-curved-hex20-quarter-annulus.md) | 2026-11-13 | registration | F-508 | Curved Hex20 on the quarter annulus (wall mid-edge nodes on the cylinders) | partial; mutation M4 equivalent, not caught | |
| [0433](../../docs/adr/0433-f379-registration-closed-form-cantilever-thick-cylinder.md) | 2026-11-14 | registration | F-379 | Cantilever in bending and thick-walled cylinder under pressure | done; single registered run, no amendment | |
| [0478](../../docs/adr/0478-f432-registration-patch-stress-recovery-and-energy-norm-error-estimate.md) | 2026-11-14 | registration | F-432 | Superconvergent patch recovery and Zienkiewicz-Zhu energy-norm estimate | partial; Hex20 effectivity 1.337 at n = 4 outside [0.8, 1.2] | (F-568, other half) |
| [0493](../../docs/adr/0493-f390-registration-hex8-b-bar-and-incompatible-modes.md) | 2026-11-14 | registration | F-390 | Hex8 without locking: B-bar (Hughes) and nine incompatible modes (Wilson et al.) | done | |

## Key numbers and limits

- F-378 (basic tier, not re-registered here): "The Tet4 case T L2 order is 1.7925 on the pair 8 to 16 against 1.8 (pre-asymptotic; 1.9176 one level finer by F-458, limit unchanged)" ([[roadmap-structural-solver]] gap column).
- F-459 M1 catch: "The L2 catch is narrow (1.7577 against 1.8)" (ADR 0355).
- F-455: "The 1e-14 relative residual after one refinement step is below the representability floor on all twelve cases (2.1e-13 to 1.8e-9). Strain energy against half the work misses 1e-12 on nine cases (1.6e-12 to 7.1e-10)" (roadmap gap, ADR 0397).
- F-432 misses: "Hex20 effectivity at n = 4 is 1.337 (outside [0.8, 1.2]); Tet10 distance from 1 grows from 0.0051 to 0.0083 between meshes 8 and 16; the tenth of largest indicators holds only 18 of 48 hole-edge elements. The recovered plate peak (1.41 percent) is met" (roadmap gap, ADR 0478).
- F-389 (Hex20, intermediate tier): two known misses, cantilever 1.8 percent and bore hoop stress 1.17 percent, met under the follow-up set-ups of F-485 (0.04 and 0.37 percent) (roadmap gap).

## Open questions

- ADR 0397 Q1, Q2: re-word F-455's clauses to the backward error and twice the rounding floor (maintainer).
- ADR 0383: the old test `si_hex20::l1_to_l3_face_loads_and_uniaxial_tension` (ADR 0292) now fails on L3, outside the standing permission to edit scaffold tests; left to the maintainer.
- F-461, F-508: "the maintainer decides done" by the roadmap gaps.

## Second half of the category (ADRs 0525 to 0627, job p3-structural-b)

The finite-strain incompatible-modes Hex8 and its stabilisation are on [[adr-thread-structural-incompatible-hex8-rule-l]].

### Current state (second half)

- **Hex8 formulations on the remaining linear paths** (F-588, ADR 0525, partial: ES3b known miss, MU12 not caught): the refusals of ADR 0493 D5 lifted in `rcfd_fem::hex8x`; free thermal expansion of the distorted patch stress-free to `1e-10` for BBar and Incompatible (`7.4e-11` worst at `nu 0.4999`), Euler buckling of a one-layer Incompatible strip within `5.1e-4` (Full control `1.5`). The ES3b miss is explained by F-614 (ADR 0530, done): with the centroid rule of Hex8 the enhanced strain is zero at the sample, so no field can separate Incompatible from the compatible strain at `element_samples`.
- **Error density** (F-568, ADR 0616, partial): `EnergyError` gives element volumes, densities `eta_e/|K|^{1/2}` and a ranking by density; the effectivity clause is met, P3 (the hole criterion) is not. On a plate graded uniformly along the hole the tenth of largest densities holds 28 of the 48 hole-edge elements, a miss (F-695, ADR 0627, partial).

### ADRs (second half)

| ADR | Date | Kind | Item | Question | Outcome | Superseded by / followed by |
|---|---|---|---|---|---|---|
| [ADR 0525](../../docs/adr/0525-f588-registration-hex8-formulations-on-the-remaining-linear-paths.md) | 2026-11-14 | registration | F-588 (partial) | B-bar and incompatible-modes Hex8 on the remaining linear paths | partial: ES3b known miss, MU12 not caught | ADR 0530 |
| [ADR 0530](../../docs/adr/0530-f614-registration-non-equilibrium-quadratic-displacement-test-for-the-estimate-samples.md) | 2026-11-14 | registration | F-614 (done) | Non-equilibrium quadratic field for the error-estimate samples of an opted Hex8 | done; explains ES3b of ADR 0525 (enhanced strain zero at the centroid sample) | — |
| [ADR 0616](../../docs/adr/0616-f568-registration-error-density-and-the-effectivity-criterion-near-one.md) | 2026-11-15 | registration | F-568 (partial) | Error indicator per unit volume, effectivity criterion near one | partial: P3 hole criterion | ADR 0627 |
| [ADR 0627](../../docs/adr/0627-f695-registration-hole-criterion-on-a-uniformly-graded-plate.md) | 2026-11-15 | registration | F-695 (partial) | Hole criterion of the error density on a uniformly graded plate | partial: 28 of 48 hole-edge elements | — |

## Related

- [[structural-solver]], [[rcfd-fem]], [[roadmap-structural-solver]]
- [[known-miss]], [[registered-limit]], [[mutation-testing]]
- [[adr-thread-structural-linear-solvers]]
