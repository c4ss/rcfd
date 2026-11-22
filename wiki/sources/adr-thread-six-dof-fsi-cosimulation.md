---
type: source
tags: [adr-thread, six-dof, fsi, added-mass, cosim, coupling, wall-pressure]
sources: [doc-cosimulation-protocol, doc-case-schema]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: six-DOF bodies in the flow, strong coupling and the co-simulation participant (flow side)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 14 ADRs, 2026-11-07 to 2026-11-13.

The flow side of fluid-structure coupling: a rigid body on a moving mesh validated against closed forms (F-244), strong (implicit) coupling with added-mass stabilisation (F-251) and its case key (F-350, F-437), the wall pressure of the body force (F-351, F-358) and of the force reports (F-440), then the co-simulation participant interface (F-139), its quasi-Newton update (F-361), interface mapping (F-362), its transport over the solver service (F-360), participants from a case (F-444, F-481), and the one-way heat-rate load export (F-237). The structural side (a dynamic solid as participant, the flow-coupled bar F-401) is [[adr-thread-structural-fsi-coupling]]. See [[strong-fsi-coupling]] and [[cosimulation-protocol]].

## Current state

- **Six-DOF validation** (F-244, done): falling block `U(10 tau)/U_t` `1.00545` (limit `abs(. - 1) <= 0.02`), time-step halving `3.4e-6` (ADR 0211).
- **Loose coupling is unstable above an added-mass ratio of 2**; the strong step `rcfd_fv::fsi::strong_step` (F-251, done) is stable at a ratio of 4 ("The D7 loop is stable up to a ratio of 2 ... in the flow it is stable at 1.5 and diverges at 3", ADR 0250 Q1). It is selectable from a case by the optional table `[dynamic_mesh.six_dof.coupling] scheme = "strong"` (F-350, ADR 0266, partial only because mutation MU5 passed its named test; the binding force-floor test of F-437, ADR 0349, catches it and Q1 there recommends F-350 done).
- **Wall pressure of the body force:** the 1.25 percent added-mass deficit is "the wall-pressure extrapolation of `patch_force_moment`" (F-351, ADR 0252); the key `wall_pressure` (`extrapolated` / `prescribed_gradient`) selects it in the driver (F-358, ADR 0269), and the opt-in table `[dynamic_mesh.force_reports] wall_pressure` does the same for force reports (F-440, ADR 0342). ADR 0269 lists for the default decision: "No registered hash moves." The default was flipped later by ADR 0346 (outside this thread): `docs/case-schema.md` gives `prescribed_gradient` as "default since ADR 0346" for both keys.
- **Co-simulation:** in-process interface `rcfd_fv::cosim` with a mock structure (F-139, done), least-squares quasi-Newton (F-361, done; Aitken stays the default, Q1), mapping builders (F-362, roadmap done; ADR 0263 proposed partial because E1 missed on the registered code, Q2 recommends done), transport over the JSON-RPC service as protocol version 1.1, identifier `rcfd-service/1` kept, calls `rcfd-cosim/1` (F-360, done), a fluid participant from a case `[cosim]` hosted by `rcfd serve --cosim` (F-444, partial: the schema property and help line await approval, ADR 0380 Q1), and the fixed-against-sliding patch check (F-481, done).
- **One-way load export:** heat rate on shell decks and the tube round trip (F-237, partial): H1 `2.2311e-4` (limit `1e-3`), H7 force `1.552e-4`; the repository's own card `RCFDQ`, since "no vendor heat-flux card layout is specified in the repository" (F-347, todo).
- **Not delivered:** a real structural solver through the protocol from a case, the participant's own name, more than one participant per serve (ADR 0380); Newmark with the strong step and rest patches (refused, ADR 0266 Q3, Q4).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0211](../../docs/adr/0211-f244-registration-flow-coupled-six-dof-validation.md) | 2026-11-07 | registration | F-244 | Flow-coupled six-DOF validation (falling block, spring-mounted piston) | done; A1 `1.00545`, A3 continuity `1.89e-11` (limit `1e-10`); added-mass ratio `0.1778` | |
| [0250](../../docs/adr/0250-f251-registration-strong-fluid-body-coupling.md) | 2026-11-12 | registration (hub) | F-251 | Strong coupling with added-mass stabilisation (`strong_step`, opt-in) | done; library function only; Q4 the `1.25` percent added-mass deficit | 0266, 0252 |
| [0252](../../docs/adr/0252-f351-registration-added-mass-deficit-wall-pressure.md) | 2026-11-12 | diagnosis | F-351 | The 1.25 percent added-mass deficit | done (by the amended rule); cause the wall-pressure extrapolation; opt-in `patch_force_moment_wall_gradient` | 0269 |
| [0266](../../docs/adr/0266-f350-registration-strong-body-flow-coupling-case-key.md) | 2026-11-12 | registration | F-350 | Strong coupling from the case tree | partial; every acceptance clause met; MU5 not caught by E1 | 0349 |
| [0269](../../docs/adr/0269-f358-registration-wall-pressure-of-the-body-force-case-key.md) | 2026-11-12 | registration | F-358 | Wall pressure of the body force as a case key, callers listed, default left to the maintainer | done (by the amended rule); default flip listed (ratio 4: `1.005081` to `1.000042`) | default: ADR 0346 |
| [0342](../../docs/adr/0342-f440-registration-moving-wall-force-reports.md) | 2026-11-12 | registration | F-440 | Force reports on moving walls with the kept fixed-flux gradient | done; default `boundary_value` left to the maintainer | default: ADR 0346 |
| [0349](../../docs/adr/0349-f437-registration-strong-coupling-force-floor.md) | 2026-11-12 | registration | F-437 | A strong-coupling case whose force floor decides the test | done; 180 of 180 iterations decided by the floor | |
| [0256](../../docs/adr/0256-f139-registration-cosimulation-participant.md) | 2026-11-12 | registration (hub) | F-139 | Co-simulation participant interface (in-process), mock structure, protocol page | done; no transport, no real structure | 0345 |
| [0261](../../docs/adr/0261-f361-registration-quasi-newton-coupling.md) | 2026-11-12 | registration | F-361 | Least-squares (interface quasi-Newton) update | done; Aitken stays the default | |
| [0263](../../docs/adr/0263-f362-registration-interface-mapping.md) | 2026-11-12 | registration | F-362 | Interface mapping by projection and radial basis, unmapped-point policy | partial by the rule (E1 missed: absolute pivot test); E1b met after amendment 1; roadmap done | |
| [0345](../../docs/adr/0345-f360-registration-cosimulation-over-the-service.md) | 2026-11-12 | registration | F-360 | The protocol over the solver service | done; protocol version 1.1; four acceptance records ignored in the gate | 0380 |
| [0380](../../docs/adr/0380-f444-registration-cosim-participants-from-the-case.md) | 2026-11-13 | registration | F-444 | Participants from the case, hosted by `rcfd serve` | partial; served run equals in-process bit for bit over 650 steps; MU5 not caught | 0381 |
| [0381](../../docs/adr/0381-f481-registration-fixed-against-sliding-patches-of-the-cosim-smoothing.md) | 2026-11-13 | registration | F-481 | A check that sees fixed against sliding patches of the smoothing | done; 168 points differ, fixed patch 0 bits | |
| [0244](../../docs/adr/0244-f237-registration-heat-rate-load-export.md) | 2026-11-12 | registration | F-237 | Heat-rate load export on shell decks, tube round trip | every registered check met; F-237 stays partial (CGNS round trip, case-tree wiring, card layouts) | |

## Key numbers and limits

- ADR 0250 Q6: the safe start `2M/(2M + m_a)` "belongs to RK4 with a constant force"; Newmark has another constant.
- ADR 0252 Q1: the default is "low by a quarter of the wall-cell size over the column length: 1.25 percent of the added mass on 20 cells, `+0.5` percent of frequency at a mass ratio of 4, `+0.1` percent at 0.2".
- ADR 0269 listing: spring-mounted piston at ratio 0.2, `omega_m/omega` `1.001307` (extrapolated) against `1.000264` (prescribed gradient); at ratio 4 strong `1.005081` against `1.000042`.
- ADR 0269 Q4: on the sliding step "no caller evaluates a force there; its default evaluation misses `0.41 g dn` on a rotor wall".
- ADR 0256 Q2: Aitken needs 8.8 iterations a step on the model problem (the quasi-Newton acceptance asks at most 7).
- ADR 0263 Q3: the brute-force search and dense radial-basis rows limit the mapping "to a few thousand nodes".
- ADR 0345 Q2: the ignored records would add "about 20 s more on a free machine, two to five minutes on a loaded one".

## Open questions

- ADR 0349 Q1: F-350 done with F-437 (recommended).
- ADR 0266 Q3, Q4: Newmark with the strong step; rest patches with the strong step.
- ADR 0261 Q1: least-squares as the default of `CouplingSettings` (recommendation keep Aitken).
- ADR 0263 Q1, Q2: elements in the `Structure` trait; whether amendment 1 closes E1.
- ADR 0380 Q1 to Q4: approve the `cosim` schema property and the `--cosim` help line; the participant's own name; MU5; a structure from a case.
- ADR 0244 questions 1 to 4: vendor heat-flux card (F-347), default mapping (conservative recommended), case-tree wiring, the `nastran_deck` conformance case.
- ADR 0252 Q3: an audit of other walls with the same mismatch (rest walls, rotating walls of the sliding step).

## The default flip of the two wall-pressure keys (ADR 0346; added by job p3-records-b)

[ADR 0346](../../docs/adr/0346-defaults-registration-wall-pressure-flip.md) (registered 2026-11-12, F-358, F-440) applies what the maintainer approved "in person, on 2026-11-12 (commit 9872a218 records it in the roadmap notes of F-358 and F-440)": ADR 0269 Q1 (b), `wall_pressure` of `[dynamic_mesh.six_dof.coupling]` defaults to `"prescribed_gradient"` instead of `"extrapolated"` in the loose and the strong scheme; and ADR 0342 Q1 (b), `wall_pressure` of `[dynamic_mesh.force_reports]` defaults to `"prescribed_gradient"` instead of `"boundary_value"`. "The library defaults (`fsi::strong_step`, `sixdof::patch_force_moment`) are not changed". The corrected wall pressure is ADR 0252's, unchanged; the flip lives in the schema default (T1), not a third value.

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0346](../../docs/adr/0346-defaults-registration-wall-pressure-flip.md) | 2026-11-12 | default change (registration) | F-358, F-440 | Flip both wall-pressure defaults to `prescribed_gradient` | **done** by its rule (H, K, E, D0, X1, Y, P1 to P6, R met; every mutation caught, MU6 only through ADR 0266 E1); no registered hash moved | |

- Measured moves (proposed F-358 note): "SP 1.001307 -> 1.000264, ADR 0266's strong piston 1.005081 -> 1.000042 (turning 0.92469), T4 x 0.506503 at 92 steps, ADR 0266 L stop 34 -> 35"; F-440: "the column's fp deficit 2.5004597e-2 -> 4.597e-6", except with layering or the compressible solver, "which keep the stored values silently (only a written table is refused there)".
- An explicit old value gives the old bits ("digests equal to the old tree's"); a written coupling or force-report table without the key now writes `wall_pressure = "prescribed_gradient"` and has another settings hash (ADR 0266's strong table T0 "with" hash now `9afbea20cf9d3338`).
- Open: Q1, a written table without the key in a refused context is now refused (recommendation (a) keep); Q2, the library defaults (recommendation (a) leave "until a caller outside the case driver needs it").

## Related

- [[strong-fsi-coupling]], [[cosimulation-protocol]], [[dynamic-mesh]], [[structural-solver]]
- [[adr-thread-structural-fsi-coupling]]
- [[roadmap-moving-meshes-rotors-and-coupling]], [[roadmap-structural-solver]]
- [[doc-cosimulation-protocol]], [[doc-case-schema]]
- Earlier history (added by p3-records-a): six-DOF rigid-body dynamics first came in wave 20B, [ADR 0033](../../docs/adr/0033-wave20b-dynamic-mesh.md) (F-229 partial), as "a loose, explicit coupling of the body to the flow"; "the flow-coupled spring-mounted-body frequency check named in the F-229 acceptance (2 percent) is not delivered" there. See [[adr-thread-layering-remeshing-ale]].
