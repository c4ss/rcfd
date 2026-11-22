---
type: source
tags: [adr-thread, structural, fem, nonlinear, hyperelastic, follower-load, flutter, buckling]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: structural solver, hyperelastic materials, follower loads, flutter and linear paths about a nonlinear state

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda, two approved default changes), 17 ADRs, 2026-11-15 to 2026-11-19. Job p3-structural-b. Follower pressure with arc length (F-424, ADR 0480) and the Newton driver are on [[adr-thread-structural-materials-nonlinear]]; the energy-momentum step with hyperelastic materials (ADR 0523, ADR 0544) is added there too.

Two chains that meet at "a linear path about a nonlinear state" in [[rcfd-fem]]: (1) compressible neo-Hookean and Mooney-Rivlin materials (F-425) carried from the total Lagrangian driver to every linear path and to the structural elements, ending in an approved default change (F-666); (2) follower loads, from an arc-length trace with a follower-only reference load (F-572) to the unsymmetric pencil eigenproblem of flutter (Beck's column, F-653) and flutter about a nonlinear state (F-676).

## Current state

- **Hyperelastic materials** are in: `rcfd_fem::mat_hyper`, `W = c10 (Ibar1 - 3) + c01 (Ibar2 - 3) + (kappa/2)(J - 1)^2`, on the total Lagrangian driver with the F-bar Hex8 of ADR 0506 as the mean-dilatation element (F-425, [ADR 0541](../../docs/adr/0541-f425-registration-hyperelastic-materials-with-the-mean-dilatation-hex8.md), partial: the small-strain check SS1 is a known miss). The case-file material kind `hyperelastic` and the block key `formulation = "bbar"` were approved on 2026-11-18 as F-621 (ADR 0541 answers); F-621 is todo at the snapshot.
- **Locking control:** on the inflated sphere (864 Hex8, `kappa/mu = 1e4`) the Full Hex8 is `15.6` off the closed-form pressure, the mean-dilatation Hex8 `4.1e-3` (F-622, ADR 0557, partial by two uncaught mutations); the arc-length trace passes the pressure maximum (F-649, ADR 0576, done, test only).
- **On the linear paths and the Method step:** the nonlinear transient Method step takes the material; the linear paths take the linearised material, the isotropic elasticity of the small-strain moduli (F-620, ADR 0555, partial); the modal miss is a rounding floor of the subspace iteration, `c eps lambda_max/lambda_1`, about `1e-9` at `K/G = 1e4` (F-644, ADR 0581). On Beam2, Shell4 and Shell3 the material enters as the isotropic element of `E = 9KG/(3K + G)`, `nu = (3K - 2G)/(2(3K + G))`, first behind `set_hyperelastic_structures(true)` (F-645, ADR 0584, done), then on the cyclic, rotating, cms and mpc paths (F-666, ADR 0593).
- **Latest decision: hyperelastic structural elements by default.** The maintainer approved the default lift on 2026-11-18 (ADR 0593 answers). ADR 0746 (2026-11-19) made it: the refusal of ADR 0555 D3 is lifted, `set_hyperelastic_structures` kept as a documented no-op, results against the isotropic twin `0` (limit `1e-10`), every model accepted before keeps its bits; "Open questions for the maintainer: None". The roadmap still lists F-666 as partial: paths never run with a hyperelastic structural element (whirl, Campbell, full rotor, cms assembly, beam warping) are open by default and unchecked ([[roadmap-structural-solver]]).
- **About a nonlinear state:** modes about a pre-stressed hyperelastic state (F-646, ADR 0586, done); `follower::tangent_buckling_factors` and `DirectHarmonic::about_state` with the driver's tangent (F-669, ADR 0590); with the contributions' tangents (F-671, ADR 0596, partial by an amended mutation catch); a blind 3-D check on a quarter tube under external follower pressure at 0.69 of the linear critical pressure (F-677, ADR 0599, done).
- **Follower loads and flutter:** `ArcLengthOptions::contribution_load` traces a follower-only reference load, `NewtonOptions::unsymmetric` factors the full load stiffness (F-572, ADR 0560, done). `rcfd_linalg::eigen_pencil` (shift-invert Arnoldi on a real unsymmetric pencil) with `follower::tangent_modes_full` and `buckling_factors_full` gives Beck's flutter load within `1.87e-3` of `20.0509536` (F-653, ADR 0580, partial: the residual measure missed); the normwise backward error `eta` replaced that measure (F-664, ADR 0587, done). A Beam2 tangential follower tip force gives Beck's column within `8.87e-4` (F-665, ADR 0589, done). Flutter about a nonlinear state with the full unsymmetric contribution tangent (F-676, ADR 0600, done).
- All the new functions are opt-in beside unchanged symmetric functions; no case-file keys exist for follower or flutter analyses (ADR 0560 Q1, ADR 0580 Q3, ADR 0589 Q3).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by / followed by |
|---|---|---|---|---|---|---|
| [ADR 0541](../../docs/adr/0541-f425-registration-hyperelastic-materials-with-the-mean-dilatation-hex8.md) | 2026-11-15 | registration | F-425 (partial) | Neo-Hookean and Mooney-Rivlin on the TL driver; F-bar Hex8 as mean-dilatation element | partial; case-file keys approved 2026-11-18 | ADR 0555 |
| [ADR 0555](../../docs/adr/0555-f620-registration-hyperelastic-materials-on-the-method-step-and-the-linear-paths.md) | 2026-11-15 | registration | F-620 (partial) | Hyperelastic materials on the nonlinear transient Method step and linear paths | partial | ADR 0581, ADR 0584 |
| [ADR 0557](../../docs/adr/0557-f622-registration-full-hex8-locking-control-of-the-inflated-sphere.md) | 2026-11-15 | registration | F-622 (partial) | Full Hex8 locking control of the inflated sphere with a converging set-up | partial: Full 15.6 off, mean-dilatation `4.1e-3` | ADR 0576 |
| [ADR 0560](../../docs/adr/0560-f572-registration-arc-length-with-a-follower-reference-load-and-an-unsymmetric-factor.md) | 2026-11-15 | registration | F-572 (done) | Arc-length with a follower reference load; unsymmetric factor | done | ADR 0580 |
| [ADR 0576](../../docs/adr/0576-f649-registration-arc-length-trace-of-the-inflated-sphere-through-its-pressure-maximum.md) | 2026-11-15 | registration | F-649 (done) | Arc-length trace of the inflated hyperelastic sphere through its pressure maximum | done (test only) | — |
| [ADR 0580](../../docs/adr/0580-f653-registration-flutter-of-follower-loads-the-unsymmetric-eigenproblem.md) | 2026-11-15 | registration | F-653 (partial) | Flutter of follower loads: unsymmetric eigenproblem (Beck's column) | partial: F3, F8 residual measure missed; flutter load `1.87e-3` of 20.0509536 | ADR 0587 |
| [ADR 0581](../../docs/adr/0581-f644-registration-hyperelastic-linear-paths-near-incompressible-modes-and-two-materials.md) | 2026-11-15 | registration | F-644 (done) | Hyperelastic linear paths, near-incompressible modes, two materials | partial (test only) | — |
| [ADR 0584](../../docs/adr/0584-f645-registration-hyperelastic-materials-on-beam2-shell4-and-shell3.md) | 2026-11-15 | registration | F-645 (done) | Hyperelastic materials on Beam2, Shell4, Shell3 on the linear paths (opt-in) | done | ADR 0593 |
| [ADR 0586](../../docs/adr/0586-f646-registration-linear-paths-about-a-pre-stressed-hyperelastic-state.md) | 2026-11-15 | registration | F-646 (done) | Linear paths about a pre-stressed hyperelastic state (stretched bar) | done (test only) | ADR 0590 |
| [ADR 0587](../../docs/adr/0587-f664-registration-backward-error-residual-of-the-pencil-eigensolver.md) | 2026-11-15 | registration | F-664 (done) | Backward-error residual of the pencil eigensolver | done | — |
| [ADR 0589](../../docs/adr/0589-f665-registration-beam2-tangential-follower-tip-force-and-a-linear-pre-stress-flutter-path.md) | 2026-11-15 | registration | F-665 (done) | Beam2 tangential follower tip force, linear pre-stress flutter path | done: Beck `8.87e-4` vs `2e-3` | — |
| [ADR 0590](../../docs/adr/0590-f669-registration-buckling-and-harmonic-response-about-a-nonlinear-hyperelastic-state.md) | 2026-11-15 | registration | F-669 (done) | Buckling and harmonic response about a nonlinear hyperelastic state | partial by rule (item done) | ADR 0596 |
| [ADR 0593](../../docs/adr/0593-f666-registration-hyperelastic-structural-elements-on-the-cyclic-rotating-cms-and-mpc-paths.md) | 2026-11-15 | registration | F-666 (partial) | Hyperelastic structural elements on cyclic, rotating, cms, mpc paths | partial: path clauses met, default lift not taken | ADR 0746 |
| [ADR 0596](../../docs/adr/0596-f671-registration-contributions-about-a-nonlinear-state-on-the-buckling-and-direct-harmonic-paths.md) | 2026-11-15 | registration | F-671 (partial) | Contributions about a nonlinear state on buckling and direct harmonic paths | partial | ADR 0599, ADR 0600 |
| [ADR 0599](../../docs/adr/0599-f677-registration-state-dependence-of-contribution-tangents-a-blind-3-d-check.md) | 2026-11-15 | registration | F-677 (done) | State dependence of contribution tangents, blind 3-D check | done (test only) | — |
| [ADR 0600](../../docs/adr/0600-f676-registration-flutter-about-a-nonlinear-state-with-the-full-contribution-tangent.md) | 2026-11-15 | registration | F-676 (done) | Flutter about a nonlinear state with the full contribution tangent | done | — |
| [ADR 0746](../../docs/adr/0746-f666-registration-hyperelastic-structural-elements-by-default.md) | 2026-11-19 | amendment (default change) | F-666 (partial) | Hyperelastic materials on structural elements by default | done: a5 made, no open questions | — |

## Key numbers and limits

- Small-strain clause of F-425: `4.7e-6` (neo-Hookean) and `4.9e-6` (Mooney-Rivlin) against `1e-6`, a known miss ([[roadmap-structural-solver]], ADR 0541).
- Inflated sphere: Full Hex8 `|p_FE - p_c| / p_c = 15.6` (limit `>= 0.1`), mean-dilatation Hex8 `4.1e-3` (limit `1e-2`) at inner stretch `1.10` (ADR 0557).
- Twin limits of the hyperelastic structural elements: `1e-10`; measured `0` on the cyclic, rotating, cms and mpc paths by default (ADR 0746).
- Beck's column: derived flutter load `20.0509536`; solid route within `1.87e-3` (limit 1 percent) (ADR 0580); Beam2 within `8.87e-4` (limit `2e-3`) and within `9.7e-7` of the Timoshenko continuum (ADR 0589); pairs backward stable to `2.5e-12` (ADR 0580).
- Arc-length ring under follower internal pressure: reproduces the load-controlled path of ADR 0480 within `1e-6` (the acceptance of F-572, ADR 0560).

## Open questions

- Case-file keys `continuation = { ..., contribution_load = true }` and `newton = { unsymmetric = true }` (ADR 0560 Q1), flutter keys (ADR 0580 Q3), follower tip-force keys (ADR 0589 Q3): recommended together, need a schema-golden change.
- `FollowerPressure` in the residual reference (ADR 0560 Q2): recommendation leave, would be opt-in.
- The pencil solver's convergence test on `eta` (ADR 0587 Q2) and a 2-norm estimate (Q1): recommendation keep as built.
- Checks of the paths opened by default but never run with a hyperelastic structural element (roadmap gap of F-666).

## Related

- [[hyperelastic-materials]], [[arc-length-continuation]], [[rcfd-fem]], [[structural-solver]], [[roadmap-structural-solver]]
- [[adr-thread-structural-materials-nonlinear]], [[adr-thread-structural-modal-buckling-cyclic]], [[adr-thread-structural-incompatible-hex8-rule-l]]
- [[bit-identical]], [[known-miss]]
