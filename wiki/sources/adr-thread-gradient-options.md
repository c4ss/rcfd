---
type: source
tags: [adr-thread, numerics, gradient, non-orthogonal, limiter, cht, electric]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the gradient options of the diffusion term, through the case tree

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda and one decision record), 4 ADRs, 2026-11-05 to 2026-11-07.

F-208 (ADR 0016 D2) built a library `GradientOptions` for the explicit non-orthogonal (secondary) part of $-\nabla\cdot(\gamma \nabla\phi)$: a face correction, a secondary limiter (Barth-Jespersen or Venkatakrishnan), and a boundary rule. It was "not claimed through the tree". This thread wires it: first to the energy and scalar sweep (F-256), then to momentum, pressure, turbulence and the electric potential (F-291), and makes the reports that read those fields consistent with the solved operator (the conjugate `heat_flow`, F-289; the electric current and Joule heat, F-298). All four items are done; every option is opt-in and bit-identical at its default. See [[doc-case-schema-gradient-options]].

## Current state

- **All four items done** (F-256, F-291 on [[roadmap-numerics-and-solvers]]; F-289, F-298 on [[roadmap-heat-materials-and-chemistry]]).
- **Keys:** four flat keys `solver.gradient_face_correction`, `solver.gradient_secondary_limiter`, `solver.gradient_limiter_k` (default `5.0`), `solver.gradient_boundary` (ADR 0190 K1; Q1 and Q2 "Not asked; proceeding on (a)"), and the optional table `[solver.gradient_equations]` with four booleans `momentum`, `pressure`, `turbulence`, `electric_potential`, each default `false` and `hash_omit_default` (ADR 0204 K1). A `true` selector makes that equation take the one option set of the case; energy and scalar diffusion always take it.
- **Reports follow the solver:** `conductive_face_flows_opt` in `rcfd-fv/src/cht.rs` (ADR 0202 D1) and `face_currents_opt`, `joule_heat_opt` in `rcfd-fv/src/electric.rs` (ADR 0221 D1). With the options, the Joule heat equals the boundary power up to the potential's residual.
- **Fixed-pressure flux correction:** under the options only (ADR 0221 D3); the default's missing boundary term "costs a converged run 2.0e-3 of the inflow" (measurement M). Making it the default (Q2) and the default Joule heat on skewed meshes (Q1) stay on recommendation (a), leave and document.
- **No accuracy benefit is claimed**: on the manufactured problem the options keep second order but "do not reduce the error" ([[doc-case-schema-gradient-options]]).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0190](../../docs/adr/0190-f256-registration-case-tree-wiring-of-the-gradient-options.md) | 2026-11-05 | registration + decision record 2026-11-06 | F-256 | The four `solver.gradient_*` keys on the energy and scalar sweep | done; Q3 answered (b) as F-289, Q4 answered (b) as F-291 | 0202, 0204 |
| [0202](../../docs/adr/0202-f289-registration-conjugate-heat-flow-report-with-the-gradient-options.md) | 2026-11-06 | registration | F-289 | The conjugate `heat_flow` report with the options, by a heat balance | done | |
| [0204](../../docs/adr/0204-f291-registration-gradient-options-on-turbulence-momentum-pressure-and-the-electric-potential.md) | 2026-11-06 | registration (two amendments) | F-291 | The options on turbulence, momentum, pressure, electric potential | done; Q1 to Q8 proceed on their reversible choices | 0221 |
| [0221](../../docs/adr/0221-f298-registration-electric-current-and-joule-heat-with-the-gradient-options-and-the-fixed-pressure-flux-correction.md) | 2026-11-07 | registration (two amendments) | F-298 | Electric current and Joule heat with the options; the fixed-pressure flux correction | done; every check met, M matters so D3 built | |

## Key numbers and limits

- ADR 0190 Q2: `gradient_limiter_k = 5.0`, "ADR 0016's C2 value"; "no other value was ever checked".
- ADR 0221 M: the default's missing boundary term on fixed-pressure faces costs `2.0e-3` of the inflow on a 30-degree wedge ([[doc-case-schema-gradient-options]]).
- ADR 0221 amendment 2: `Some(GradientOptions::default())` is `None` (the tree never passes the former).
- ADR 0202: the tree runs of the check C stop on `tolerance_u` after 374 to 376 iterations; gate 2938 passed, 2 failed (outside the item), 822 ignored.

## Open questions

- ADR 0202 Q1: the non-conjugate `EnergySolver::heat_flow_out` has no secondary flux on fixed-value faces; recommendation (a) leave it, (b) as an item only if a case needs it.
- ADR 0221 Q1: the default Joule heat on a skewed mesh (recommendation (a) leave and document, (c) an opt-in key only if a case needs it).
- ADR 0221 Q2: the fixed-pressure flux correction as the default (recommendation (b) as its own item "if M matters"; changes every converged result of a skewed fixed-pressure case with correctors).
- ADR 0190 Q1, Q2 and ADR 0204 Q1: key names and places, "reversible before any case uses them".

## Related

- [[doc-case-schema-gradient-options]], [[doc-case-schema-linear-upwind-limiter]], [[mesh-quality]], [[simple]], [[bit-identical]]
- [[rcfd-fv]], [[roadmap-numerics-and-solvers]], [[roadmap-heat-materials-and-chemistry]]
