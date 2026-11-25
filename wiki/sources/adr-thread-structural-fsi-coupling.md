---
type: source
tags: [adr-thread, structural, fem, fsi, coupling, cosim]
sources: [doc-cosimulation-protocol]
origin: docs/adr/ (ADR list below)
origin_commit: 82985b47
created: 2026-11-22
updated: 2026-11-25
status: draft
---

# ADR thread: structural solver, coupling with the flow (failing participants, dynamic solid, one-way loads, flow-coupled bar, aerodynamic damping)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 10 ADRs, 2026-11-12 to 2026-11-14.

The structural side of the [[cosimulation-protocol]]: coupled steps whose structure or fluid fails in a later iteration (the coupler's restore), a dynamic solid of [[rcfd-fem]] as a co-simulation participant on interface weights built from the two surfaces, its coupled frequency on non-matching interfaces, the bilinear quad projection, the elastic bar coupled two-way with the incompressible flow solver (F-401), one-way loads of the F-237 deck applied to the solid surface (F-384, F-523), and aerodynamic damping and added mass of a mode by prescribed wall motion (F-421).

## Current state

- **Two-way coupling with the flow exists as a library check**, not as a case: F-401 (ADR 0434) couples an elastic bar with the PISO flow solver (the piston on an elastic bar); every acceptance clause met, but the companion VS missed (the viscous splitting of the PISO step, proportional to $\nu$), so **partial**; ADR 0434 Q1 to Q3 open. A `[solid]` table in the flow case (F-404) and a public coupled benchmark (F-434) are todo ([[roadmap-structural-solver]]).
- **Dynamic solid participant** (F-400, ADR 0410, partial): every clause met except c7, the coupled frequency change between the matching and the 3:2 map (6.3e-3 and 2.4e-3 against 1e-3; non-consistent weights in the transposed projection). F-502 (ADR 0412, done) predicts that change from the static compliance of the mapped weights, within 2.4e-3 of the predictions against the limit 0.1. F-504 (ADR 0414, done) adds an opt-in bilinear projection on quad interface elements.
- **Failing participants:** a failing static solve in iteration 2 or 3 (F-463, ADR 0361, done); a failing fluid (F-465, ADR 0364, partial: the start-of-iteration restore mutations caught only by a set-up panic); closed by F-469 (ADR 0444, done) from a start state that uses no restore. F-465 is done by the lead's reading of 2026-11-25 (82985b47): F-469 (ADR 0444) holds the check that was missing.
- **One-way loads:** F-384 (ADR 0436, partial: N2 defective set-up, MU4 not caught; Tet, Tet10, Hex20 surfaces refused); deck pressure on non-parallelogram faces (F-523, ADR 0441, done).
- **Aerodynamic damping** (F-421, ADR 0446, partial): the Womersley-5 real part missed in the registered run (8.55e-2 against 3e-2), W1, LF, P1 met only after amendments; Q1 to Q3 open.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0361](../../docs/adr/0361-f463-registration-failing-solve-in-a-later-iteration.md) | 2026-11-12 | registration | F-463 | Failing static solve in iteration 2 and 3 of a coupled step | done; M9, M11 survived as registered | |
| [0364](../../docs/adr/0364-f465-registration-failing-fluid-in-a-later-iteration.md) | 2026-11-12 | registration | F-465 | Failing fluid in iteration 2 and 3 of a coupled step | partial; M16 to M19, M25 caught only by the set-up panic | 0444 |
| [0410](../../docs/adr/0410-f400-registration-dynamic-solid-participant.md) | 2026-11-13 | registration (hub) | F-400 | Dynamic solid as a co-simulation participant, weights from both surfaces | partial; c7 6.3e-3 / 2.4e-3 against 1e-3 | 0412 |
| [0412](../../docs/adr/0412-f502-registration-nonmatching-coupled-frequency.md) | 2026-11-13 | registration | F-502 | Coupled frequency on non-matching interfaces against face refinement and map | done | |
| [0414](../../docs/adr/0414-f504-registration-bilinear-quad-projection.md) | 2026-11-13 | registration | F-504 | Opt-in bilinear projection on quad interface elements | done | |
| [0434](../../docs/adr/0434-f401-registration-piston-on-an-elastic-bar.md) | 2026-11-14 | registration (hub) | F-401 | Two-way coupling of an elastic bar with the incompressible flow solver | partial; VS missed (PISO viscous splitting), H1 after amendment 1 | |
| [0436](../../docs/adr/0436-f384-registration-one-way-deck-loads.md) | 2026-11-14 | registration | F-384 | Loads of the F-237 deck on the solid surface | partial; N2 after amendment, MU4 not caught | 0441 |
| [0441](../../docs/adr/0441-f523-registration-deck-nonparallelogram-faces.md) | 2026-11-14 | registration | F-523 | Deck pressure loads on non-parallelogram faces | done | |
| [0444](../../docs/adr/0444-f469-registration-loose-start-for-failing-fluid.md) | 2026-11-14 | registration | F-469 | Failing-participant checks from a start state with no restore | done; M16 to M19, M25 caught | |
| [0446](../../docs/adr/0446-f421-registration-aerodynamic-damping-energy-method.md) | 2026-11-14 | registration | F-421 | Aerodynamic damping and added mass of a mode (energy method) | partial; W5 8.55e-2 against 3e-2 | |

## Key numbers and limits

- F-400 gap: "The coupled frequency change between the matching and the 3:2 map is 6.3e-3 (ratio 4) and 2.4e-3 (ratio 0.5) against the limit 1e-3 ... A follow-up measures it by face refinement (below 1e-3 from 8 x 8 elements with the projection)" ([[roadmap-structural-solver]]).
- F-401 gap: "the PISO step moves the tip by 8.3e-5 U at nu x 100, proportional to nu. The note says it is met at 1.0e-10 U with the opt-in component diagonal (F-531)" (see [[component-diagonal]]).
- From the protocol page: Aitken and least-squares converge to `1e-6` in 3 iterations on the matching wall ([[doc-cosimulation-protocol]]).
- F-421 gap: "The order companion of the Womersley-1 imaginary part is 0.68 against 0.8"; Symmetry dissipation addressed by F-531, PISO stability by F-530.

## Open questions

- ADR 0410 Q1 to Q3, ADR 0434 Q1 to Q3, ADR 0446 Q1 to Q3 remain open (roadmap gaps).
- ADR 0414 Q1: expose the bilinear projection in the case file (the F-444 path): (a) a map-method choice in the cosimulation section, recommended as a follow-up (changes the schema golden).
- F-465: "A registered check that observes them is missing" (roadmap gap), although F-469 then caught M16 to M19 and M25.

## Related

- [[cosimulation-protocol]], [[rcfd-fem]], [[rcfd-fv]], [[piso]], [[structural-solver]], [[roadmap-structural-solver]]
- [[roadmap-moving-meshes-rotors-and-coupling]], [[dynamic-mesh]]
