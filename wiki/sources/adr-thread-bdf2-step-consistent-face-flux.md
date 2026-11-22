---
type: source
tags: [adr-thread, numerics, bdf2, rhie-chow, time-order, fractional-step, overset]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the step-size-dependent remainder of BDF2 and the step-consistent face flux

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (diagnoses with opt-ins, one decision record of the maintainer), 4 ADRs, 2026-11-12.

The BDF2 step of the overset path missed its second-order ratio (F-338, ADR 0241: D2 `2.46` against 4), and the remainder was located in the library BDF2 step even at rest. F-344 (ADR 0245) named the term: the Rhie-Chow face flux depends on the step size. It built an opt-in step-consistent face coefficient for internal faces (`PimpleSettings::rhie_chow_step_consistent`), which the maintainer merged as an opt-in in person despite ADR 0076's earlier removal of a face-flux variant (F-G). F-443 (ADR 0352) carried it to the remaining transient paths and a case key; F-454 (ADR 0356) built the analogue for the fractional step; F-442 (ADR 0359) traced the remainder that stays on a moving overset mesh to two events of the coupling. See [[piso]], [[adr-thread-overset]].

## Current state

- **F-443 done; F-344, F-454, F-442 partial** ([[roadmap-numerics-and-solvers]]).
- **Opt-in, not default.** Decision record of 2026-11-12 (ADR 0245): "The maintainer decided in person on 2026-11-12 to merge F-344 as an opt-in (`PimpleSettings::rhie_chow_step_consistent`), limited to internal faces, not as the default. ADR 0076 stands as the record of F-G's removal and is not edited." C by default is open (ADR 0245 Q5, ADR 0352 Q1): "it would change every transient result".
- **Coverage (ADR 0352):** C "is now defined on every path the case tree's transient incompressible runs take except lumped components", including the consistent correctors and `step_ale_with_walls`; the key needed no hash-table or golden edit. Case key: `[solver.rhie_chow] step_consistent` ([[doc-case-schema-rhie-chow]]).
- **Fractional step (ADR 0356):** a step-consistent face flux for the projection, the same to `1.5e-13` by an opt-in, defaults bit for bit; partial only because mutation MF5 is "equivalent by construction". No case key (Q2); the Chorin form is not changed.
- **Moving overset mesh (ADR 0359):** two events carry the remainder, a donor stencil that changes with the pose (event 1) and cells changing kind next to the hole (event 2). The pose-continuous stencil F-468 (ADR 0396, in [[adr-thread-overset]]) removes event 1; event 2 remains (ratio `2.14`) and is accepted "until a case needs the BDF2 order on a moving overset mesh" (Q2 recommendation (b)).
- Still on the default face flux: `step_sliding`, the fractional step (without its opt-in) and coupled faces (F-344 gap).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0245](../../docs/adr/0245-f344-registration-bdf2-step-remainder.md) | 2026-11-12 | diagnosis + opt-in (three amendments, decision record) | F-344 | Which term makes the library BDF2 step's remainder step-size dependent | partial; H7 `8.41e-10` against `1.7e-6` (ratio `5.02`); A1 met; A2 missed `2.454` against 4 on TG_2 | 0352, 0359 |
| [0352](../../docs/adr/0352-f443-registration-step-consistent-face-coefficient-remaining-paths.md) | 2026-11-12 | registration | F-443 | The step-consistent face coefficient on the remaining paths and a key | done; all checks met, all eight mutations caught | 0356 |
| [0356](../../docs/adr/0356-f454-registration-fractional-step-step-consistent-face-flux.md) | 2026-11-12 | registration | F-454 | A step-consistent face flux for the fractional step | partial; every acceptance clause met, MF5 equivalent | |
| [0359](../../docs/adr/0359-f442-registration-overset-bdf2-motion-remainder.md) | 2026-11-12 | diagnosis | F-442 | The remainder made by the motion through the overset coupling | partial; H4 `1.16e-6` against `6.2e-6`, H5 `n / f = 8.79` against 5; D2 `1.845` with the most continuous stencil | 0396 (F-468) |

## Key numbers and limits

- ADR 0245 decision record: the default face flux's step dependence is "`5.13e-6` at a ratio of `2.30` on the mesh at rest, `3.7e-4` in the steady cavity".
- ADR 0245 Q1: the remainder of the coupled meshes in motion is `2.06e-5` on TG_2 (ratio `2.45`), `5.2e-5` on level 1 moving against `1.3e-7` at rest, below the spatial error of TG_2 (`3.2e-4`) by a factor 15. H8: the remainder is spatial with order `1.8` to `1.9` at fixed `nu dt / h^2`.
- ADR 0352 Q2: the fractional step's own step dependence is `8.69e-2` in the steady cavity between `dt = 0.04` and `0.01`, "large against the PISO step's `3.7e-4`" (became F-454).
- ADR 0359 Q1: a pose-continuous stencil would take the component cells from `2.06e-5` to `1.2e-6`; Q2: the kind changes give `9.9e-6`, ratio `1.85`, "below the spatial error by a factor 29 at level 2".
- F-344 gap: R2b on SECT-M is `0.578` against the default `0.538` (the seam floor of ADR 0076).

## Open questions

- ADR 0245 Q2: keeping `rhie_chow_step_consistent` is "in conflict with the letter of ADR 0076's removal, so it is flagged for the maintainer" (later answered by the decision record: merged as opt-in). Q3 open per the F-344 gap.
- ADR 0352 Q1 / ADR 0245 Q5, ADR 0356 Q3: C or F as the default (every transient or fractional-step result and hash changes).
- ADR 0352 Q3 to Q5: `step_sliding` with C (not now), lumped components (keep the located error), the pressure-lag estimate reads the default face coefficient.
- ADR 0356 Q2: a case key for the fractional step (recommendation (b), a new optional key, when a case needs it). Q4: outlets and periodic faces keep `dte R`.
- ADR 0359 Q2: the kind changes, accepted until a case needs them.

## Related

- [[piso]], [[doc-case-schema-rhie-chow]], [[adr-thread-overset]], [[adr-thread-piso-stability-consistent-correctors]], [[overset-mesh]], [[bit-identical]]
- [[roadmap-numerics-and-solvers]]
