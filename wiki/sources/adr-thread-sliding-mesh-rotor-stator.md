---
type: source
tags: [adr-thread, moving-mesh, sliding-mesh, rotor-stator, periodic, case-tree]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: sliding (rotor-stator) interface, library to case tree

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 12 ADRs, 2026-11-07 to 2026-11-16.

The transient rotor-stator path of rcfd: a rotor block turned against a stator block across a cylinder, the non-conformal interface rebuilt every step (F-250), its periodic sector and planar forms (F-304, F-704), the case-tree wiring of the full annulus (F-303) and the sector (F-711), and the defects found on the way (node coincidences F-710, F-720, F-721, F-724, F-730; symmetry planes F-712; sector against full annulus F-725). The follow-up on the solver's coupled periodic faces is [[adr-thread-coupled-periodic-faces]]. See [[sliding-mesh]].

## Current state

- **Sliding solves run from a case file.** F-303 (ADR 0637): the optional table `[mesh.sliding]` (`angular_velocity`, `rotor_walls = "rotating" | "rest"`) on `mesh.generator = "annulus_3d_cylindrical_interface"` runs from `rcfd run`; c1 is met ("the rotor turning 6 pitches and the interface rebuilt each step (36 topology changes), and the binary's numbers equal the in-process run's bits"), c2 is met (optional table, no hash-table or golden edit) and c3 is met by NA1c at `6.8e-4` of `U0`. F-711 (ADR 0646, **done**): the same table on the new generator `annulus_3d_sector_cylindrical_interface` turns a rotor sector past one pitch through `mesh_turned`, checked against the full-annulus tree run.
- **F-303 is partial by the rule only.** ADR 0637 decision: "NA2c (a negative control's threshold) and U1c missed: partial". The roadmap gap names the one miss left: NA2c reads `3.2e-2` against its registered threshold `5e-2`; U1c (coincidences) is met since F-710 and the symmetry-plane miss since F-712 (ADR 0650). ADR 0637 Q6 recommends done ("the acceptance's three clauses being met (NA1c sees the mesh flux by a factor 48)"), for the lead to confirm.
- **Limits of the tree path** (ADR 0637 "Limits", ADR 0646 "Limits"): one rotor, axis `z` through the origin, constant angular velocity; "no turbulence, energy, scalars, sources, body force, BDF2"; expression boundary values refused; "no restart"; not the compressible solver; no parallel runs; forward rotation only on the sector (ADR 0634 Q6); sector pitch below 180 deg, both blocks of one pitch; the first-order interface interpolation of ADR 0109 unchanged. The planar slider has no tree key (F-729, todo).
- **Coincidences:** at a whole rotor pitch the interface graph falls into parts. Full annulus: fixed in `sliding_step` since F-710 (ADR 0638, done). Sector: the base periodic step keeps a uniform flow (F-720, done) but leaves a latent part-mean defect for flows without the parts' symmetry (ADR 0640 N1, `4e-4` to `1e-3`); the coupled step `periodic_sliding_step_coupled` (F-721, opt-in) fixes it, and in the tree it is the opt-in `[mesh.sliding.coincidences] coupled = true` (F-730, ADR 0654, done). F-724 (ADR 0659, partial) adds the opt-in threshold rule `PartCoupling::FluxScale`.
- **Defaults held by the maintainer.** "Answers of the maintainer (2026-11-18)" in ADR 0641: making the coupled path the default of `periodic_sliding_step` is "Held. The default change is not approved now: it would move existing results and waits until the fan path needs it"; ADR 0659 Q4 and Q5 (F-724) are "held with this one".
- **Sector against full annulus:** F-725 (ADR 0661, partial): within `1.981e-2 U_in` (limit `3e-2`) over 400 steps with a non-axisymmetric inlet; the difference is "the lagged partner value of the momentum equation on the coupled faces", which opened F-743 and F-744 ([[adr-thread-coupled-periodic-faces]]).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0216](../../docs/adr/0216-f250-registration-sliding-interface-on-a-cylinder.md) | 2026-11-07 | registration (hub) | F-250 | Sliding non-conformal interface on a cylinder, in the library (`rcfd_mesh::sliding`, `sliding_step`, `step_sliding`) | partial; S1 interface mass `1.51e-10` against `1e-10` with two non-orthogonal correctors (`3.00e-15` with six), S3 continuity `1.78e-9` against `1e-9`, R1b missed; torques T1 to T3 met; Q1 (tree) and Q7 (a check that sees the rotor mesh flux) open | tree: 0637 |
| [0629](../../docs/adr/0629-f304-registration-planar-and-sector-sliding-interfaces.md) | 2026-11-16 | registration | F-304 | Planar and sector sliding with periodic seams kept by the join | done in the library, limit one period of offset; Q4 (orthogonal-only coupling of wrap faces) left open | 0634 (beyond one period) |
| [0634](../../docs/adr/0634-f704-registration-sliding-beyond-one-period-re-imaging-of-the-rotor.md) | 2026-11-16 | registration | F-704 | Sector and planar sliding beyond one period (re-imaging of the rotor) | proposed done (by the amended rule; partial as first registered, Q5); roadmap partial | tree: 0646 |
| [0637](../../docs/adr/0637-f303-registration-sliding-interface-in-the-case-tree.md) | 2026-11-16 | registration (hub) | F-303 | Full-annulus sliding interface in the case tree, loop and a non-axisymmetric check | partial; NA1c `6.8e-4` of `U0` met, control `3.2e-2` (48 times larger) under its own threshold `5e-2`; U1c `2e-4` at coincidences (later F-710); symmetry planes `2.4e-2` (later F-712) | |
| [0638](../../docs/adr/0638-f710-registration-sliding-interface-at-coinciding-nodes.md) | 2026-11-16 | registration | F-710 | Uniform flow at coinciding nodes on the full annulus | done; 24 parts at a whole pitch, old-flux divergence `2.1e-3` diagnosed; since then `6.0e-15` | |
| [0640](../../docs/adr/0640-f720-registration-periodic-sliding-at-coinciding-nodes.md) | 2026-11-16 | registration | F-720 | Periodic sliding at coinciding nodes | done, no fix needed for a uniform flow (`5.8e-15`); latent N1 defect `4e-4` to `1e-3` named | 0641 |
| [0641](../../docs/adr/0641-f721-registration-periodic-sliding-at-coinciding-nodes-without-the-parts-symmetry.md) | 2026-11-16 | registration | F-721 | Coincidences for flows without the parts' symmetry | partial; opt-in `periodic_sliding_step_coupled`; F1 `3.49e-2 U_in` against `3e-2` (start transient at the rotor seam); default held 2026-11-18 | 0659, 0661 |
| [0646](../../docs/adr/0646-f711-registration-periodic-sector-sliding-in-the-case-tree.md) | 2026-11-16 | registration (hub) | F-711 | Periodic sector sliding in the case tree, sector generator, sector against full annulus | done; within `1.15e-2 U0` and `1.01e-2 Omega r_in` of the full annulus over 2.1 pitches | |
| [0650](../../docs/adr/0650-f712-registration-symmetry-planes-and-a-uniform-in-plane-flow-on-the-annulus-interface-pair.md) | 2026-11-16 | registration (diagnosis) | F-712 | Symmetry planes and a uniform in-plane flow on the annulus pair | partial; cause the averaged component diagonal (spurious drag $k = 4\nu / (3 l_z^2) = 13.3\ \mathrm{s}^{-1}$); `corrected` keeps `7.9e-15` through `[mesh.sliding]`; default flip is the maintainer's | |
| [0654](../../docs/adr/0654-f730-registration-sector-sliding-in-the-tree-at-node-coincidences.md) | 2026-11-16 | registration | F-730 | Sector sliding in the tree at node coincidences | done; `[mesh.sliding.coincidences] coupled = true`; residual `1.0e-13` (base `1.4e-4`); within `1.25e-2 U0` of the annulus | |
| [0659](../../docs/adr/0659-f724-registration-threshold-scale-of-the-coupled-interface-solves.md) | 2026-11-16 | registration (2 addenda) | F-724 | Threshold scale of the coupled interface solves | partial; opt-in `FluxScale`; Q4 (periodic default) and Q5 (`sliding_step` default) need the maintainer, held 2026-11-18 | |
| [0661](../../docs/adr/0661-f725-registration-sector-against-full-annulus-with-a-non-axisymmetric-inlet.md) | 2026-11-16 | registration (diagnosis) | F-725 | Sector against full annulus with a non-axisymmetric inlet | partial; G1 `1.981e-2 U_in` met (limit `3e-2`); c3 undecided ($q = 0.302$, lag $l = 1.017$) | 0664, 0667 |

## Key numbers and limits

- ADR 0216 acceptance results (rows "S1 (acceptance A1)", "R1", "T1" to "T3"): interface mass `1.51e-10` against `<= 1e-10` (missed), `3.00e-15` with six correctors (SF6, met); R1 `5.939e-4` against `8.902e-4`; rotor wall torque `+1.410e-2`, stator `-1.344e-2`, through the interface `-1.033e-3`, each against `<= 2e-2`.
- ADR 0637 NA1c (the $m = 2$ vortical field, every wall at rest, sliding run against the static join after whole pitches): `6.8e-4` of `U0`; with the rotor mesh flux dropped `3.2e-2`. Uniform flow without coincidences (U1d) `7e-15`; at coincidences `2e-4` (before F-710).
- ADR 0637 D3: a step whose interface non-orthogonality exceeds 70 deg (ADR 0112's measure) stops the run with a located message (`mesh.sliding`). New monitors with the table: `sliding_angle`, `sliding_interface_flux`, `sliding_gcl`, `sliding_non_orthogonality`.
- ADR 0646: uniform axial flow `2.4e-14` through both re-imaging steps; sector rms against the annulus `3.5e-4` and `1.3e-4` at the end; leaving the rotor vectors in the old image raises the differences to `7.8e-2` and `0.46`.
- ADR 0654: base sector at coincidences leaves `1.4e-4` (at least `6.9e-3` of the speed) in the remapped flux; the coupling changes the velocity by about `1e-3 U0`.
- ADR 0650: default step 1 deviation `2.3588e-2` in a wall cell; `corrected` on the frozen join `7.8e-16`; flipping the default moves only the `packed_bed` row of `baseline_hashes` ("the other 12 rows unchanged").
- ADR 0661: G2 rms at step 400 `8.65e-3` (limit `1.5e-2`); ADR 0641's start reproduced `3.494135e-2`.

## Open questions

- ADR 0637 Q6: F-303 done or partial (recommendation done, the lead's to confirm).
- ADR 0634 Q5: F-704 done with the amended controls or partial (the lead's).
- ADR 0650 Q1: `corrected` component diagonal as the default (recommendation (c), guarded transient split; follow-up "Corrected component diagonal as the default").
- ADR 0659 Q4, Q5 and ADR 0641 Q1: coupled / `FluxScale` defaults; held on 2026-11-18 "until the fan path needs it".
- ADR 0646 Q6, ADR 0654 Q6: the Studio preview counts the sector generator's cells as unknown.
- ADR 0646 Q4: planar sliding in the tree (F-729, todo).
- ADR 0661 question 1: c3's reading (accept the decomposition and record done, or keep partial).

## Related

- [[sliding-mesh]], [[dynamic-mesh]], [[mrf]], [[fan-01]], [[component-diagonal]], [[piso]]
- [[adr-thread-coupled-periodic-faces]], [[adr-thread-fan01-rotating-solve]]
- [[roadmap-moving-meshes-rotors-and-coupling]]
- [[doc-dynamic-mesh]], [[doc-readme]], [[doc-roadmap-overview]]
