---
type: source
tags: [adr-thread, structural, fem, modal, buckling, cyclic-symmetry, mpc]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: structural solver, modes, buckling, the transformation path and cyclic symmetry

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 12 ADRs, 2026-11-12 to 2026-11-14. Later buckling-eigensolver and shell/beam cyclic ADRs (F-628, F-642, F-650, F-672, F-680) are in the other half of the category.

Modal analysis and linear buckling of [[rcfd-fem]] beyond the basic tier: the convergence test of the subspace eigensolver, the geometric (initial-stress) stiffness, pre-stressed modes and buckling, everything on the [[mpc-transformation]] `x = T q + g` (linear constraints, rigid sets, elastic supports, F-393), the refusal of mechanisms, and [[cyclic-symmetry]]: one sector of a rotor for statics and modes by harmonic index.

## Current state

- **Cyclic symmetry** works for statics and modes by harmonic index (F-394, ADR 0386, done: eight of eight mutations caught), with harmonic loads and rigid sets at a non-zero index (F-489, ADR 0395, done), cylindrical material axes (F-544, ADR 0462, done) and pre-stressed modes with a cyclic constraint and per-point materials (F-547, ADR 0465, done).
- **On the transformation:** modal analysis (F-486, ADR 0384, partial by the A1 zero-mode ratio 2.07e-6 against 1e-6, which F-488 re-measured at 2.1e-13 sigma), zero modes and the count of free rigid motions (F-488, ADR 0399, done), pre-stressed modes and buckling (F-493, ADR 0398, partial: a mechanism not refused), and the zero-shift pivot-ratio rule that refuses a mechanism in buckling on the transformation (F-495, ADR 0401, done) and on the prescribed path (F-496, ADR 0402, done).
- **Buckling:** F-395 (ADR 0390) is **partial**: the second factor misses by 2.0135e-2 against 2e-2 (the solid's K_sigma carries axial stress on rotating sections); the knife-edge column against the solid's closed form (F-492, ADR 0394) is done.
- **Eigensolver:** scale-aware convergence of the subspace iteration (F-457, ADR 0354) done, with a caveat on C2.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0354](../../docs/adr/0354-f457-registration-scale-aware-eigensolver-convergence.md) | 2026-11-12 | registration | F-457 | Scale-aware convergence test of the subspace eigensolver | done; MS1 to MS7 caught, caveat on C2 | |
| [0384](../../docs/adr/0384-f486-registration-modal-analysis-on-the-transformation.md) | 2026-11-13 | registration | F-486 | Modal analysis on the transformation (constraints, rigid sets, supports) | partial; A1 zero-mode ratio 2.07e-6 against 1e-6; `si_mpc::n4` needs the maintainer | 0399 |
| [0386](../../docs/adr/0386-f394-registration-cyclic-symmetry-sector-statics-and-harmonic-modes.md) | 2026-11-13 | registration (hub) | F-394 | One sector of a rotor for statics and for modes by harmonic index | done; frequencies within 3.96e-14 of the full model | |
| [0390](../../docs/adr/0390-f395-registration-geometric-stiffness-prestressed-modes-and-buckling.md) | 2026-11-13 | registration | F-395 | Geometric stiffness, pre-stressed modes, linear buckling | partial; A9 2.0135e-2 against 2e-2 (known miss) | 0394 |
| [0394](../../docs/adr/0394-f492-registration-knife-edge-buckling-closed-form.md) | 2026-11-13 | registration | F-492 | Knife-edge column factors against the solid's closed form | done | |
| [0395](../../docs/adr/0395-f489-registration-cyclic-harmonic-loads-and-rigid-sets.md) | 2026-11-13 | registration | F-489 | Harmonic loads and rigid sets at a non-zero harmonic index | done (H3 via corrected H3b, set-up defect) | |
| [0398](../../docs/adr/0398-f493-registration-prestressed-modes-and-buckling-on-the-transformation.md) | 2026-11-13 | registration | F-493 | Pre-stressed modes and buckling on the transformation | partial; C13 mechanism row returns a rounding factor instead of a refusal | 0401 |
| [0399](../../docs/adr/0399-f488-registration-zero-modes-on-the-transformation.md) | 2026-11-13 | registration | F-488 | Zero modes on the transformation (measure against the shift, free rigid motions count) | done; seven mutations caught | |
| [0401](../../docs/adr/0401-f495-registration-buckling-mechanism-refusal.md) | 2026-11-13 | registration | F-495 | Refuse a mechanism in buckling on the transformation (zero-shift pivot ratio) | done; rounding ratio 2.5e-11, 400 times below 1e-8 | |
| [0402](../../docs/adr/0402-f496-registration-buckling-prescribed-mechanism.md) | 2026-11-13 | registration | F-496 | Refuse a non-rigid mechanism in buckling on the prescribed path (pivot ratio on K_FF) | done | |
| [0462](../../docs/adr/0462-f544-registration-cyclic-symmetry-with-cylindrical-material-axes.md) | 2026-11-14 | registration | F-544 | Cyclic symmetry with cylindrical material axes | done; (a4) with a 45-fold margin | |
| [0465](../../docs/adr/0465-f547-registration-prestressed-modes-cyclic-per-point-materials.md) | 2026-11-14 | registration | F-547 | Pre-stressed modes with a cyclic constraint and per-point materials | done; a behaviour change flagged for the maintainer | |

## Key numbers and limits

- F-394 (ADR 0386): sector against full model `max |f_s/f_f - 1|` 3.96e-14 (limit 1e-8); thin-ring n = 2 at k = 2 4.40e-4 and n = 3 at k = 3 1.81e-3 (limit 0.02); spinning-disc rim `u_r` 8.89e-5 (limit 0.01); matching tolerance `MATCH_TOLERANCE 1e-8` of the largest radius.
- F-395 gap: "The second buckling factor is 2.0135e-2 from four times the first against the limit 2e-2 ... a follow-up settled this against a derived closed form (1.7e-4) and at L/h = 40 (0.52 percent), but not against the acceptance as written" ([[roadmap-structural-solver]]).
- F-486 gap: "the three zero modes of a bar held only by axial springs are at 2.07e-6 of the first flexible frequency against the registered 1e-6; F-488 measured them at 2.1e-13 sigma (limit 1e-9 sigma)".
- F-495: "BM's rounding ratio is 2.5e-11, 400 times below `1e-8`; the margin may shrink on larger" models (ADR 0401 Q1).

## Open questions

- ADR 0390 Q1: re-word clause a9 (F-395) to the derived form, "the second factor within 1 percent of the root of the" derived equation (maintainer).
- ADR 0386 Q1 was taken on the standing instruction of 2026-11-11 (option (a): `modal_analysis` of a cyclic model gives `k = 0`, documented); Q2 became F-489.
- F-486: "The maintainer decides done" (roadmap gap).

## Related

- [[cyclic-symmetry]], [[mpc-transformation]], [[rcfd-fem]], [[structural-solver]], [[roadmap-structural-solver]]
- [[adr-thread-structural-rotordynamics]] (rotating modes of a cyclic sector, Campbell tables)
- [[known-miss]]
