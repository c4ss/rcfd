---
type: source
tags: [adr-thread, structural, fem, beams, shells, laminate, joints]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: structural solver, beams and shells (Beam2, Shell4, Shell3, laminates, ties and joints)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 25 ADRs, 2026-11-13 to 2026-11-14. Many later beam and shell ADRs (warping Beam2 F-598, F-626, F-627; Shell3 on the remaining paths F-541; curvature-based shell pressure F-546, F-633, F-656; shells in the cyclic constraint F-628, F-648, F-662; through-thickness gradients F-527, F-687) are in the other half of the category.

Structural elements of [[rcfd-fem]] with six unknowns per node: the shear-flexible beam Beam2 (F-391), the flat-facet shells Shell4 (MITC4, F-392) and Shell3 (MITC3 with stabilisation, F-419), their loads, output, use on the [[mpc-transformation]], drilling membranes, stress stiffening, follower pressure, corotational nonlinearity, laminated Shell4 (F-428), and ties of non-matching meshes and joints of beams and shells to solids (F-426, F-543).

## Current state

- **Beam2** is in: six unknowns per node in the DofMap, nodal moments and moment reactions, exact prismatic Timoshenko stiffness (F-391, ADR 0417, done; a mesh without beams keeps `DofMap::uniform(n, 3)`, so `baseline_hashes` are unchanged). Line and gravity loads (F-514, ADR 0422, partial: one defective reference amended); output as VTK line cells (F-512, ADR 0423, partial: MU7 caught only by a check added after the run); on the transformation (F-513, ADR 0426, partial: MU7); entry and output refusals (F-518, ADR 0429; F-516, ADR 0430; done).
- **Shell4** (F-392, ADR 0432, done): bilinear membrane, Reissner-Mindlin bending with MITC4 shear, drilling penalty `alpha G t (theta_z - omega)^2` with `alpha = 1e-3`, rigid offsets for warped nodes (warp ratio above 0.05 refused). On the transformation and with body loads (F-522, ADR 0435, done), VTK output (F-521, ADR 0437, done), the rotating plane-stress disc (F-524, ADR 0439, done), nodal body force and thermal loads (F-525, ADR 0445, partial: rounding rigid rotation 2.1e-10 against 1e-12).
- **Shell3** (F-419, ADR 0454, partial): CST membrane, MITC3 with Lyly-Stenberg-Vihinen stabilisation (alpha 0.1); pressure on curved facet meshes by an opt-in angle-weighted split (F-540, ADR 0461, done). Drilling membranes by ANDES OPT: Shell3 (F-539, ADR 0466, partial: mixed Shell3 + Shell4 patch test misses 2.1e-1, so mixed meshes are refused) and Shell4 (F-549, ADR 0470, done, lifting that refusal for ANDES edges).
- **Stress stiffening of beams and shells** (F-420, ADR 0473, partial: Shell3 twisted strip 4.05e-2 against 4 percent, diagnosed by F-556, ADR 0483, as Shell3 rest stiffness); pre-stress on mixed solid + structural models (F-557, ADR 0486, partial); beam moment, shear and Wagner terms (F-558, ADR 0508, partial: I-section lateral-torsional buckling -19.2 percent, needs a warping unknown, follow-up F-598).
- **Nonlinear shells:** follower pressure on shells (F-573, ADR 0487, partial, its cylinder clause met by F-578); corotational facets in the Newton driver (F-578, ADR 0490, partial: a library defect fixed by amendment 2 after run 1).
- **Laminates:** laminated Shell4 by classical lamination theory, ply stresses and failure criteria (F-428, ADR 0496, done); under body loads and pre-stress (F-591, ADR 0502, done).
- **Ties and joints:** tied non-matching meshes and beam/shell joints to solids (F-426, ADR 0456, partial: J1, J2 missed with unit weights); consistent face weights (F-543, ADR 0460, done).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0417](../../docs/adr/0417-f391-registration-six-unknowns-and-beam.md) | 2026-11-13 | registration (hub) | F-391 | Six unknowns per node, Beam2 (exact Timoshenko) | done; margins above 100 on 1e-10 checks | |
| [0422](../../docs/adr/0422-f514-registration-beam-line-and-gravity-loads.md) | 2026-11-13 | registration | F-514 | Line and body loads on beams | partial; gravity row reference amended | |
| [0423](../../docs/adr/0423-f512-registration-beam-results-in-solid-output.md) | 2026-11-13 | registration | F-512 | Beam results in the solid output | partial; MU7 caught only by N2b | 0430 |
| [0426](../../docs/adr/0426-f513-registration-beams-on-the-transformation.md) | 2026-11-14 | registration | F-513 | Beams on the transformation (rigid sets to solids, springs, rotation constraints) | partial; MU7 not caught | 0429 |
| [0429](../../docs/adr/0429-f518-registration-beam-entry-refusal.md) | 2026-11-14 | registration | F-518 | Observe the beam entry refusal before the transformation is built | done | |
| [0430](../../docs/adr/0430-f516-registration-beam-output-layout-refusal.md) | 2026-11-14 | registration | F-516 | Beam output refuses a result with a different unknowns-per-node count | done | |
| [0432](../../docs/adr/0432-f392-registration-flat-facet-shell.md) | 2026-11-14 | registration (hub) | F-392 | Four-node flat-facet shell Shell4 (MITC4) | done; patch tests margin above 1e4 | |
| [0435](../../docs/adr/0435-f522-registration-shells-on-the-transformation-and-body-loads.md) | 2026-11-14 | registration | F-522 | Shells on the transformation, body loads on shells | done | |
| [0437](../../docs/adr/0437-f521-registration-shell-solid-output.md) | 2026-11-14 | registration | F-521 | Shell results in the solid output | ADR partial (N6b after amendment); item done | |
| [0439](../../docs/adr/0439-f524-registration-rotating-shell-disc.md) | 2026-11-14 | registration | F-524 | Rotating plane-stress disc of Shell4 facets against the closed form | done | |
| [0445](../../docs/adr/0445-f525-registration-shell-nodal-body-force-and-thermal-loads.md) | 2026-11-14 | registration | F-525 | Nodal body-force fields and thermal loads on Shell4 | partial; TH3a, TH3d missed | |
| [0454](../../docs/adr/0454-f419-registration-three-node-flat-shell.md) | 2026-11-14 | registration (hub) | F-419 | Three-node flat shell Shell3 (MITC3 + LSV stabilisation) | partial; .vtu check after amendment 1 | 0461, 0466 |
| [0456](../../docs/adr/0456-f426-registration-tied-meshes-joints-cyclic-extensions.md) | 2026-11-14 | registration | F-426 | Tied non-matching meshes, beam/shell joints, non-matching cyclic boundaries | partial; J1, J2 missed | 0460 |
| [0460](../../docs/adr/0460-f543-registration-consistent-joint-couplings.md) | 2026-11-14 | registration | F-543 | Joint couplings with consistent face weights | done | |
| [0461](../../docs/adr/0461-f540-registration-angle-weighted-shell-pressure.md) | 2026-11-14 | registration (diagnosis + opt-in) | F-540 | Shell3 pressure on curved facets, angle-weighted split | done | (F-546, other half) |
| [0466](../../docs/adr/0466-f539-registration-drilling-membrane-for-shell3.md) | 2026-11-14 | registration | F-539 | ANDES OPT drilling membrane for Shell3 (Allman 1984 beside it) | partial; mixed patch test 2.1e-1 | 0470 |
| [0470](../../docs/adr/0470-f549-registration-drilling-membrane-for-shell4.md) | 2026-11-14 | registration | F-549 | Shell4 drilling membrane matching the ANDES edge | done | |
| [0473](../../docs/adr/0473-f420-registration-stress-stiffening-of-beams-and-shells.md) | 2026-11-14 | registration | F-420 | Geometric stiffness, centrifugal load, spin softening of Beam2, Shell4, Shell3 | partial; E2 4.05e-2 | 0483 |
| [0483](../../docs/adr/0483-f556-registration-shell3-twisted-strip-rest-stiffness-diagnosis.md) | 2026-11-14 | diagnosis | F-556 | Shell3 rest stiffness of the twisted strip against Hex20 | partial; C3 3.7e-8, C7/N3 9.3e-2 known misses | |
| [0486](../../docs/adr/0486-f557-registration-prestress-of-mixed-solid-and-structural-models.md) | 2026-11-14 | registration | F-557 | Pre-stress of a model with both solid and structural elements | partial; hub checks after amendment 1 | |
| [0487](../../docs/adr/0487-f573-registration-follower-pressure-on-shells.md) | 2026-11-14 | registration | F-573 | Follower pressure on shells with its load stiffness | partial; cylinder clause met by F-578 | |
| [0490](../../docs/adr/0490-f578-registration-corotational-shells-newton-driver.md) | 2026-11-14 | registration | F-578 | Geometrically nonlinear shells (corotational facets) | partial; library defect fixed by amendment 2 | (F-585, other half) |
| [0496](../../docs/adr/0496-f428-registration-laminated-shell-layup-ply-stresses-failure.md) | 2026-11-14 | registration | F-428 | Laminated Shell4, ply stresses, failure criteria | done | |
| [0502](../../docs/adr/0502-f591-registration-laminated-shell-body-loads-and-prestress.md) | 2026-11-14 | registration | F-591 | Laminated Shell4 under body loads and pre-stress | done | |
| [0508](../../docs/adr/0508-f558-registration-beam-moment-shear-wagner-and-shell-rotation-geometric-terms.md) | 2026-11-14 | registration | F-558 | Beam moment, shear, Wagner terms; shell `t^2/12` rotation terms | partial; I-section LTB -19.2 percent | (F-598, other half) |

## Key numbers and limits

- F-419 (ADR 0454): "Patch tests to 1e-13, SS plate 0.998 and 0.997 of Navier at t/a 0.01 and 0.001 (plain MITC3 locks at 0.899), cantilever 0.99986".
- F-392 margins (ADR 0432): "patch tests above 1e4, SS1 13, SS2 12, SS3 113, CT1 4.8, CT2 above 1e8, RG1 14, CY1 17, FQ1 2.5, FQ2 1.9 at (2,2), FR1 40".
- F-539 gap: "The mixed Shell3 plus Shell4 membrane patch test misses (2.1e-1 ...) ... The cantilever strip (0.99648 of beam theory) and Shell3 patch tests are met" ([[roadmap-structural-solver]]).
- F-525 gap: "rounding rigid in-plane rotation of 2.1e-10 alpha dT L against the limit 1e-12 (the soft drilling mode at the clamped node; no strain or stress)".
- F-426 gap: "beam continuing a solid bar 2.25 percent (limit 1), shell continuing a solid plate 2.36 percent (limit 2). The node-to-surface tie's local stress error (0.149) does not fall; only the mortar tie meets the 3 percent clause".
- F-573 gap: cylinder clause "met by F-578 (5.36e-4 for Shell4, 5.40e-4 for Shell3 to stretch 1.51)".
- F-558 gap: "lateral-torsional buckling of an I section is -19.2 percent off the closed form, because it needs a warping degree of freedom".

## Open questions

- F-558: the case-file key `[structural] geometric_terms = full` (maintainer, roadmap gap).
- F-578: partial because amendment 2 fixed a library defect after run 1, "which the status rule of 2026-11-18 does not cover".
- F-512, F-513, F-557: "The maintainer decides done" (roadmap gaps).
- ADR 0437 Q1: close F-521 on N6b (the item is done in the roadmap).

## Related

- [[rcfd-fem]], [[structural-solver]], [[roadmap-structural-solver]], [[mpc-transformation]]
- [[adr-thread-structural-rotordynamics]] (Beam2 shafts of the flexible rotor)
- [[adr-thread-structural-materials-nonlinear]] (Newton driver used by corotational shells and follower loads)
