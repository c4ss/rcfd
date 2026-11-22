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

## Second half of the category: shells (ADRs 0522 to 0716, job p3-structural-b)

The warping Beam2 chain (F-598 onward) is on its own page, [[adr-thread-structural-beam-warping]]; shells and beams in the cyclic constraint are added to [[adr-thread-structural-modal-buckling-cyclic]]; shell whirl to [[adr-thread-structural-rotordynamics]]; through-thickness temperature gradients (F-527, F-687) to [[adr-thread-structural-creep-and-temperature]]; the thin-plate multigrid (F-629, F-641, F-659) to [[adr-thread-structural-linear-solvers]].

### Current state (second half)

- **Corotated shells in arc length and the nonlinear transient** (F-585, ADR 0522, partial): `arclength::trace`, `solve_from` and the nonlinear transient take Shell4 and Shell3, with a constant consistent shell mass including the rotary inertia `rho t^3/12`; the hinged roof missed A1 (`2.53e-2`) and A1p (`2.61e-2`) against a solid reference with a singular hinge. The published reference closed it: Shell4 `1.8e-3` and Shell3 `2.3e-3` below the limit load of Sze, Liu and Lo (2004), Table 9a, `2226.3 N` (F-610, ADR 0716, done; "the paper itself read"). ADR 0716 Q2 leaves to the lead whether F-585 moves to done; the roadmap lists F-585 partial at the snapshot.
- **Shell3 on the remaining paths** (F-541, ADR 0545, partial): no path takes Shell4 and refuses Shell3; three rows (multigrid, `check_supported()`, cyclic) stayed refused for both, then opened by F-629/F-641 (multigrid) and F-628 (cyclic).
- **Warped Shell3 meshes:** an opt-in model check of the constant-strain Shell3 membrane (largest angle between facet normals, threshold `0.4` degrees, the message names the ANDES membrane of ADR 0466) (F-574, ADR 0537, done).
- **Shell pressure on curved unstructured meshes.** The angle-weighted pressure on an unstructured sphere octant converges at order `2.07`, `2.06` with radial error `8.0e-3` at `n = 128` (F-546, ADR 0550, done); the error is `~ c h^2/(R t)`, a bending response to the nodal unevenness of the load, documented in `docs/structural-solver.md` (F-634, ADR 0562, done). An opt-in `Load::shell_pressure_curvature` (Shell3, quadric fit on each node's k-ring) is exact for the uniform membrane state (F-633, ADR 0567, partial: S1 met only after amendment 1), and a boundary jet estimator `CurvatureBoundary::Jet` extends it to a non-quadric torus segment (F-656, ADR 0579, partial: the diff-scope check B4 missed as worded).

### ADRs (second half)

| ADR | Date | Kind | Item | Question | Outcome | Superseded by / followed by |
|---|---|---|---|---|---|---|
| [ADR 0522](../../docs/adr/0522-f585-registration-corotated-shells-in-arc-length-and-nonlinear-transient.md) | 2026-11-14 | registration | F-585 (partial) | Corotated shells in arc-length and the nonlinear transient | partial: roof A1 `2.53e-2`, A1p `2.61e-2` vs 2 % | ADR 0716 (published roof reference) |
| [ADR 0537](../../docs/adr/0537-f574-registration-warning-for-the-constant-strain-shell3-membrane-on-warped-meshes.md) | 2026-11-14 | registration | F-574 (done) | Opt-in model check for the constant-strain Shell3 membrane on warped meshes | done (threshold 0.4 degrees) | — |
| [ADR 0545](../../docs/adr/0545-f541-registration-shell3-on-the-remaining-paths.md) | 2026-11-15 | registration | F-541 (partial) | Shell3 on the remaining paths (N2 refusals of ADR 0454) | partial: KM1 (three rows wait for Shell4) | ADR 0552, ADR 0556, ADR 0603 |
| [ADR 0550](../../docs/adr/0550-f546-registration-angle-weighted-shell-pressure-on-an-unstructured-sphere-octant.md) | 2026-11-15 | registration | F-546 (done) | Angle-weighted shell pressure on an unstructured sphere octant | done: `8.0e-3` at n = 128, order 2.07 | ADR 0562, ADR 0567 |
| [ADR 0562](../../docs/adr/0562-f634-registration-shell-documentation-note-on-pressure-loaded-thin-curved-shells.md) | 2026-11-15 | registration | F-634 (done) | Docs note on pressure-loaded thin curved shells on unstructured meshes | done (`docs/structural-solver.md`) | — |
| [ADR 0567](../../docs/adr/0567-f633-registration-shell3-pressure-load-exact-for-the-uniform-membrane-state.md) | 2026-11-15 | registration | F-633 (partial) | Shell3 pressure from a local curvature estimate, exact for uniform membrane state | partial: S1 met only after amendment 1 | ADR 0579 |
| [ADR 0579](../../docs/adr/0579-f656-registration-curvature-shell-pressure-on-a-non-quadric-shell.md) | 2026-11-15 | registration | F-656 (partial) | Curvature shell pressure on a non-quadric shell (torus segment), boundary estimator | partial | — |
| [ADR 0716](../../docs/adr/0716-f610-registration-hinged-cylindrical-roof-limit-load-against-a-published-reference.md) | 2026-11-17 | registration | F-610 (done) | Hinged cylindrical roof limit load against Sze, Liu and Lo (2004) | done: Shell4 `1.8e-3`, Shell3 `2.3e-3` below 2226.3 N | — |

### Key numbers and limits (second half)

- Hinged cylindrical roof (`R 2540`, edges `508`, `0.1 rad`, `t 12.7`, `E 3102.75`, `nu 0.3`): published limit load `2226.3 N`; Shell4 `1.8e-3`, Shell3 `2.3e-3` below it at `16 x 16`, within `3.3e-3` at `8 x 8`; the Hex20 sequence `-2.64e-2` at `n = 16` (ADR 0716).
- Sphere octant (`R = 1`, `t = 0.01`): errors `1.40e-1`, `3.33e-2`, `7.99e-3` at `n = 32, 64, 128`; 2 % needs `n = 128` at `t/R = 0.01` (ADR 0550, ADR 0562).
- Curvature pressure: A1 `3.0e-14`, A2 `7.8e-14`, A3 `2.0e-10` (ADR 0567); torus segment `4.1e-4` and `5.3e-4` (orders `2.5`, `1.8`) with the boundary estimator, `3.7e-3` and `2.1e-3` with the ADR 0567 load (ADR 0579).

### Open questions (second half)

- The thin roof `t = 6.35` with snap-back against the paper's table (ADR 0716 Q1): recommended as a follow-up.
- Whether F-585 moves to done on the strength of ADR 0716 (ADR 0716 Q2): the lead's call.
- Shell case-file key for corotated shells (ADR 0522 Q3) and for the Shell3 warp check (ADR 0537 Q1): open, schema golden.
- The `0.4` degree threshold rests on one geometry; a second family is recommended before wider use (ADR 0537 Q2).

## Related

- [[rcfd-fem]], [[structural-solver]], [[roadmap-structural-solver]], [[mpc-transformation]]
- [[adr-thread-structural-rotordynamics]] (Beam2 shafts of the flexible rotor)
- [[adr-thread-structural-materials-nonlinear]] (Newton driver used by corotational shells and follower loads)
