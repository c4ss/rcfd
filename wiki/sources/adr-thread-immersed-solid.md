---
type: source
tags: [adr-thread, immersed-boundary, forcing, dfg-cylinder, piso, lock-in]
sources: [doc-reference-data-f219-immersed-solid, doc-reference-data-f337-immersed-moment, doc-reference-data-f346-immersed-unsteady-force, doc-reference-data-f354-dfg-2d2-extrema, doc-reference-data-f355-immersed-piso]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: immersed (non-conforming) solid with modified forcing, fixed and moving

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations, diagnoses with result addenda), 17 ADRs, 2026-11-07 to 2026-11-17.

F-219 (ADR 0218) put a solid inside a Cartesian mesh by a modified forcing, checked on the DFG cylinder benchmarks ([[dfg-cylinder-benchmark]]). Three lines follow: the force and moment (F-337, F-346, F-354), the second-order forcing in transient PISO and its single-corrector stop (F-355, F-466, F-363, F-483, F-482, F-494, F-501, F-515), and moving immersed shapes against body-fitted moving-mesh runs (F-308, F-707, F-731, F-750, F-756). Everything is library or test code; no case key exists. See [[immersed-solid]].

## Current state

- **The forcing is first order** (F-219 partial): circular Couette observed order `1.023` against `1.5`, a known miss. The acceptance numbers are met: DFG 2D-1 `C_D` `-0.0215 %`, `C_L` `+0.669 %` on M3, 2D-2 `St` `-0.382 %` on T (ADR 0218).
- **Force and moment:** the moment of `force_moment` was first order (arm at the surface crossing); a second-order method beside it (F-337, done). For a body at rest the reaction of the forcing already is the unsteady force (F-346 partial: the 2D-2 lift on T `+2.58` percent against 1.5). ADR 0268 located that lift error to the D/40 mesh at the cylinder; on the D/80 meshes max `C_D` and max `C_L` are within `0.43` percent with either forcing (F-354 partial: on the M family with `build` the lift error does not decrease from D/40 to D/80, `0.161` to `0.283`).
- **Second-order forcing in PISO:** the unbounded `build_second_order` stops at step 6 of 2D-2 (Courant `1.1094e7`; the linear extension has a weight above 5 and "multiplies the velocity of the first fluid cell relative to the wall by up to 38 on M2"); a bounded constructor `build_second_order_bounded` runs (F-355 partial). Its stop on T at step 2951 was a breakdown of the AMG-preconditioned pressure CG (`r^T z < 0`), remedied by the opt-in `PimpleSettings::pressure_ic_fallback` (F-466, done; "The default configuration ... still stops on T at step 2951").
- **One PISO corrector:** refused with a located message for flux faces with weight above 1 (F-363); the stop came from the start field with flow through the body (F-501, done); the opt-in start projection `PimpleSolver::project_immersed_start` lifts the refusal (F-515, done). The case key for it is F-517 (todo).
- **Moving immersed shapes** (`immersed_motion.rs`, `MovingImmersed`, opt-in): in-line oscillation (F-308) and transverse oscillation at Re 20 (F-707) met every heavy check but are partial by the amendment rule (recommended done, the lead's call). Lock-in at Re 100: F-731 missed the lift amplitude (`0.0551` against `0.0478`); F-750 and F-756 found no qualifying forcing point in their scans, so no comparison was made. F-765 (frequency allowance bounded) is todo.
- **Maintainer, 2026-11-18** (ADR 0254 answers): `build_second_order` building the bounded form (F-364) is "Held ... it waits until the fan path needs it. Not to be issued."

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0218](../../docs/adr/0218-f219-registration-immersed-solid-with-modified-forcing.md) | 2026-11-07 | registration (hub) | F-219 | Immersed solid with modified forcing, DFG 2D-1 / 2D-2 | partial; acceptance met; V1 order `1.023` against `1.5` (known miss) | |
| [0234](../../docs/adr/0234-f309-registration-immersed-face-flux-at-the-surface-crossing.md) | 2026-11-12 | registration | F-309 | Second-order forcing: the fluid-solid face flux at the surface crossing (`U_f = HbyA_c + (t_f/theta_e) (u_G - HbyA_c)`, opt-in constructor) | done in the library; force and moment stay first order (Q5); added by the p3-physics job | |
| [0239](../../docs/adr/0239-f337-registration-immersed-force-and-moment-order.md) | 2026-11-12 | registration (diagnosis) | F-337 | Second-order force and moment | done in the library; torque at N 48 `4.12171` to `4.18431` if made the default (Q1) | 0249 |
| [0249](../../docs/adr/0249-f346-registration-immersed-force-in-unsteady-flow.md) | 2026-11-12 | registration | F-346 | Force and moment in unsteady flow | partial; 2D-2 lift on T missed (`+2.58` percent); M2 `+0.26`, `-0.16` percent | 0268 |
| [0254](../../docs/adr/0254-f355-registration-immersed-second-order-forcing-in-transient-piso.md) | 2026-11-12 | diagnosis | F-355 | Second-order forcing, the stop in transient PISO at Re 100 | partial; cause the weight above 5 at an impulsive start; bounded form built; held default 2026-11-18 | 0363, 0374 |
| [0268](../../docs/adr/0268-f354-registration-dfg-2d2-force-extrema-mesh-and-time-step-convergence.md) | 2026-11-12 | measurement | F-354 | DFG 2D-2 force extrema, mesh and time-step convergence | partial; D/80 within `0.43` percent; R1 (T, bounded) stopped at step 2951 | 0363 |
| [0363](../../docs/adr/0363-f466-registration-bounded-forcing-stop-on-t.md) | 2026-11-12 | diagnosis | F-466 | The stop of 2D-2 on T with the bounded forcing | done; AMG-CG breakdown; opt-in `pressure_ic_fallback` reaches `t = 8`; A2-T-2 met | |
| [0374](../../docs/adr/0374-f363-registration-immersed-flux-with-one-piso-corrector.md) | 2026-11-12 | diagnosis | F-363 | Second-order flux with a single PISO corrector | partial; refusal built; stop depends on `dt` (runs at `5e-4`), not explained | 0388, 0392 |
| [0388](../../docs/adr/0388-f483-registration-pressure-lag-estimate-at-an-immersed-body.md) | 2026-11-13 | diagnosis | F-483 | Pressure-lag estimate at an immersed body | partial; an eigenvalue of the frozen map (1.175, 1.248) at the first BDF2 step; does not predict the stop | |
| [0392](../../docs/adr/0392-f482-registration-immersed-step-linearisation.md) | 2026-11-13 | diagnosis | F-482 | Linearisation of the whole step | partial; period-two mode `-1.103 +- 0.346i` not dominant; pressure modes up to `1.93` | 0405 |
| [0405](../../docs/adr/0405-f494-registration-immersed-multistep-linearisation.md) | 2026-11-13 | diagnosis | F-494 | Growth over several steps | partial; bounded `1.881` per step, first-order `1.311`, separated by size only | 0421 |
| [0421](../../docs/adr/0421-f501-registration-immersed-early-state-start.md) | 2026-11-13 | diagnosis | F-501 | Early-state pressure modes from a start without flow through the cylinder | done; from the projected start the bounded form runs 200 steps | 0425 |
| [0425](../../docs/adr/0425-f515-registration-immersed-start-projection.md) | 2026-11-14 | registration | F-515 | Immersed start projection in the library | done; refusal of ADR 0374 lifted for a projected start | |
| [0631](../../docs/adr/0631-f308-registration-moving-immersed-shapes.md) | 2026-11-16 | registration | F-308 | Moving immersed shapes, in-line oscillating cylinder against body-fitted | partial by the amendment rule (every check met after amendment 2) | 0648 |
| [0648](../../docs/adr/0648-f707-registration-a-transversely-oscillating-cylinder-in-a-stream.md) | 2026-11-16 | registration | F-707 | Transversely oscillating cylinder in a stream (Re 20) | partial by the amendment rule; A1 to A5, R1 met with margins of 4 to 15; N1 misses every measure | 0657 |
| [0657](../../docs/adr/0657-f731-registration-moving-immersed-shapes-at-lock-in.md) | 2026-11-16 | registration | F-731 | Lock-in at Re 100 (A/D 0.5, f D/U 0.14) | partial; lift amplitude `0.0551` against `0.0478` (15 percent, limit 8.3) | 0671 |
| [0671](../../docs/adr/0671-f750-registration-moving-immersed-shapes-at-lock-in-where-the-lift-does-not-cancel.md) | 2026-11-16 | scan | F-750 | Lock-in where the lift does not cancel | partial; no candidate qualified, main record not run | 0677 |
| [0677](../../docs/adr/0677-f756-registration-moving-immersed-shapes-at-lock-in-near-the-natural-frequency-at-the-smaller-amplitude.md) | 2026-11-17 | scan | F-756 | Lock-in near the natural frequency at A/D 0.25 | partial; no candidate qualified (first harmonic changes 49 to 73 per cent across the bracket) | F-765 (todo) |

## Key numbers and limits

- ADR 0218: 2D-1 reference `C_D` 5.57953523384, `C_L` 0.010618948146; 2D-2 `St` 0.30188; acceptance limit 5 percent each.
- ADR 0254: bounded form on M2: max `C_D` 3.20930 (`-0.33` percent), max `C_L` 0.97593 (`-1.01` percent), `St` 0.30112 (`-0.25` percent).
- ADR 0268 Q6 recommends quoting the D/80 values as the validation (T2 with the bounded form `+0.136`, `+0.175`, `-0.042` percent) and that "M2's agreement is a cancellation and should not be quoted".
- ADR 0363: with `pressure_ic_fallback`, T reaches `t = 8` with max `C_D` `+0.216` and max `C_L` `+1.687` percent; A2-T-2 met (`0.216` to `0.136`, `1.687` to `0.175`).
- ADR 0374: with one corrector the stop steps are 126 (weights up to 2), 46 (theta_min 0.1), 34 (bounded).
- ADR 0421: from the projected start the ten-step growth is `1.012` (first-order) and `1.057` (bounded) per step.
- ADR 0657 (roadmap gap of F-731): complex harmonic 20 percent (limit 12.3), first-harmonic change across the bracket 43 percent (limit 25), negative-control mean drag only 2.2 percent (needs over 4.0).

## Open questions

- ADR 0631 Q1, ADR 0648 Q1, ADR 0657 Q1, ADR 0671 Q1, ADR 0677 Q1: a case-file key for moving immersed bodies (needs the F-307 schema golden).
- ADR 0631 Q3, ADR 0648 Q3: done or partial under the amendment rule (recommended done, the lead's call).
- ADR 0677 Q4: replace the bracket conditioning by a bounded frequency allowance (changes the acceptance, the maintainer's; it became F-765).
- ADR 0249 Q1 to Q3; ADR 0268 Q5 to Q7; ADR 0374 Q1 to Q4; ADR 0388 Q1, Q2; ADR 0392 Q2: as listed in the roadmap gaps.
- ADR 0363 Q4 to Q6: the AMG root cause, the repeat as default, a guard inside `cg` (changes iterates, needs the hash check).
- ADR 0425 Q2: a case-file key for the start projection (F-517, todo; schema golden).
- ADR 0234 Q1 to Q6 (F-309): the face flux as part of `modified` (recommendation (a) keep opt-in until Q5; on M2 the drag would move from `-0.17` to `-0.31` percent), a case key `immersed_solids` (schema golden), the refusal as a panic, the continuity measure, the first-order force and moment, `pick_correctors` without D1 to D3. ([[adr-thread-amg-coarse-lu]] holds the root cause of the AMG breakdown of ADR 0363.)

## Related

- [[immersed-solid]], [[dfg-cylinder-benchmark]], [[cylinder-re100]], [[piso]], [[dynamic-mesh]]
- [[adr-thread-amg-coarse-lu]], [[adr-thread-bdf2-step-consistent-face-flux]]
- [[adr-thread-overset]]
- [[roadmap-moving-meshes-rotors-and-coupling]]
- [[doc-reference-data-f219-immersed-solid]], [[doc-reference-data-f354-dfg-2d2-extrema]], [[doc-reference-data-f355-immersed-piso]]
